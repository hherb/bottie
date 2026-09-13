//! Assistant-owned generated-image metadata and content-addressed PNG storage.

mod retry;
mod types;

use std::{
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};

use super::{
    ConversationStore, DEFAULT_PROFILE_ID, MessageState, StorageError, StoredMessage,
    attachment_preview::AttachmentPreview, load_conversation_from_connection, memory_chunks,
    now_ms,
};

pub(crate) use types::{
    GeneratedAssetExecution, GeneratedAssetStatus, GeneratedImageProvenance,
    GeneratedImageRequestOptions, PreparedGeneratedImage, StartedGeneratedImage,
    StoredGeneratedAsset, normalize_generated_png,
};

const GENERATED_ASSET_DIRECTORY_NAME: &str = "generated-assets";
const GENERATED_BLOB_DIRECTORY_NAME: &str = "blobs";
const GENERATED_TEMPORARY_DIRECTORY_NAME: &str = "temporary";
const GENERATED_MEDIA_TYPE: &str = "image/png";
const MIN_OUTPUT_COUNT: u8 = 1;
const MAX_OUTPUT_COUNT: u8 = 6;

impl ConversationStore {
    /// Loads one completed generated-image preview by opaque asset identity.
    pub(crate) fn load_generated_asset_preview(
        &self,
        asset_id: &str,
    ) -> Result<Option<AttachmentPreview>, StorageError> {
        let sha256 = self
            .open()?
            .query_row(
                "SELECT sha256 FROM generated_assets WHERE id = ?1 AND status = 'completed'",
                [asset_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        sha256
            .map(|sha256| {
                let path = self.generated_asset_blob_path(&sha256)?;
                Self::encode_preview_file(
                    &path,
                    super::image_normalization::NormalizedImageFormat::Png,
                )
            })
            .transpose()
    }

    /// Inserts one assistant message and its ordered pending generated-image outputs atomically.
    pub(crate) fn start_generated_image_message(
        &self,
        conversation_id: &str,
        request_message_id: &str,
        expected_prompt: &str,
        output_count: u8,
        provenance: &GeneratedImageProvenance,
        options: &GeneratedImageRequestOptions,
    ) -> Result<StartedGeneratedImage, StorageError> {
        validate_output_count(output_count)?;
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let branch_id = selected_branch_without_active_generation(&transaction, conversation_id)?;
        let prompt = transaction
            .query_row(
                "SELECT message_blocks.text_content FROM messages
                 JOIN message_blocks ON message_blocks.message_id = messages.id
                    AND message_blocks.ordinal = 0 AND message_blocks.block_type = 'text'
                 WHERE messages.id = ?1 AND messages.conversation_id = ?2
                   AND messages.branch_id = ?3 AND messages.role = 'user' AND messages.state = 'final'
                   AND NOT EXISTS (
                       SELECT 1 FROM messages AS later
                       WHERE later.branch_id = messages.branch_id AND later.sequence > messages.sequence
                   )",
                params![request_message_id, conversation_id, branch_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| StorageError::invalid("That image prompt is no longer the selected request."))?;
        if prompt != expected_prompt {
            return Err(StorageError::invalid(
                "The image prompt did not match the durable selected request.",
            ));
        }
        let message_id = insert_generated_image_message(
            &transaction,
            conversation_id,
            &branch_id,
            request_message_id,
            output_count,
            provenance,
            options,
        )?;
        let message = load_generated_message(&transaction, conversation_id, &message_id)?;
        transaction.commit()?;
        Ok(StartedGeneratedImage {
            message,
            output_count,
            prompt,
            options: *options,
            provenance: provenance.clone(),
        })
    }

    /// Commits an exact set of validated PNGs and finalizes their assistant message.
    pub(crate) fn complete_generated_image_message(
        &self,
        message_id: &str,
        images: &[PreparedGeneratedImage],
    ) -> Result<StoredMessage, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let conversation_id = pending_generated_message_conversation(&transaction, message_id)?;
        let expected_count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM generated_assets WHERE message_id = ?1 AND status = 'pending'",
            [message_id],
            |row| row.get(0),
        )?;
        if i64::try_from(images.len()).ok() != Some(expected_count) || images.is_empty() {
            return Err(StorageError::invalid(
                "The generated image count did not match the accepted request.",
            ));
        }
        let mut newly_moved = Vec::new();
        let update_result = (|| -> Result<(), StorageError> {
            for (ordinal, image) in images.iter().enumerate() {
                verify_prepared_image(image)?;
                let destination = self.generated_asset_blob_path(&image.sha256)?;
                if destination.exists() {
                    verify_content_file(&destination, image.byte_size, &image.sha256)?;
                    fs::remove_file(&image.temporary_path)
                        .map_err(|_| StorageError::generated_image())?;
                } else {
                    fs::create_dir_all(destination.parent().ok_or_else(StorageError::internal)?)
                        .map_err(|_| StorageError::generated_image())?;
                    fs::rename(&image.temporary_path, &destination)
                        .map_err(|_| StorageError::generated_image())?;
                    newly_moved.push(destination);
                }
                let changed = transaction.execute(
                    "UPDATE generated_assets
                     SET status = 'completed', sha256 = ?1, media_type = ?2, width = ?3, height = ?4,
                         byte_size = ?5, updated_at_ms = ?6
                     WHERE message_id = ?7 AND ordinal = ?8 AND status = 'pending'",
                    params![
                        image.sha256,
                        GENERATED_MEDIA_TYPE,
                        image.width,
                        image.height,
                        i64::try_from(image.byte_size)
                            .map_err(|_| StorageError::generated_image())?,
                        now_ms()?,
                        message_id,
                        i64::try_from(ordinal).map_err(|_| StorageError::generated_image())?,
                    ],
                )?;
                if changed != 1 {
                    return Err(StorageError::generated_image());
                }
            }
            Ok(())
        })();
        if let Err(error) = update_result {
            cleanup_new_files(&newly_moved);
            return Err(error);
        }
        let label = if images.len() == 1 {
            "Generated image."
        } else {
            "Generated images."
        };
        let message = match (|| {
            finish_generated_message(&transaction, message_id, MessageState::Final, label)?;
            load_generated_message(&transaction, &conversation_id, message_id)
        })() {
            Ok(message) => message,
            Err(error) => {
                cleanup_new_files(&newly_moved);
                return Err(error);
            }
        };
        if let Err(error) = transaction.commit() {
            cleanup_new_files(&newly_moved);
            return Err(error.into());
        }
        Ok(message)
    }

    /// Finalizes a pending generated-image message without retaining partial bytes.
    pub(crate) fn fail_generated_image_message(
        &self,
        message_id: &str,
        status: GeneratedAssetStatus,
        error_code: Option<&str>,
    ) -> Result<StoredMessage, StorageError> {
        let (message_state, label, stored_error) = match status {
            GeneratedAssetStatus::Cancelled => {
                (MessageState::Cancelled, "Image generation cancelled.", None)
            }
            GeneratedAssetStatus::Failed => (
                MessageState::Failed,
                "Image generation failed.",
                Some(error_code.unwrap_or("generation_failed")),
            ),
            _ => {
                return Err(StorageError::invalid(
                    "Only failed or cancelled image generation can use this action.",
                ));
            }
        };
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let conversation_id = pending_generated_message_conversation(&transaction, message_id)?;
        transaction.execute(
            "UPDATE generated_assets SET status = ?1, error_code = ?2, updated_at_ms = ?3
             WHERE message_id = ?4 AND status = 'pending'",
            params![status.as_str(), stored_error, now_ms()?, message_id],
        )?;
        finish_generated_message(&transaction, message_id, message_state, label)?;
        let message = load_generated_message(&transaction, &conversation_id, message_id)?;
        transaction.commit()?;
        Ok(message)
    }

    /// Returns the native temporary directory used before validated PNG commit.
    pub(crate) fn generated_asset_temporary_directory(&self) -> PathBuf {
        self.generated_asset_root()
            .join(GENERATED_TEMPORARY_DIRECTORY_NAME)
    }

    /// Resolves one content identity to its application-private generated PNG path.
    pub(crate) fn generated_asset_blob_path(&self, sha256: &str) -> Result<PathBuf, StorageError> {
        if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(StorageError::generated_image());
        }
        Ok(self
            .generated_asset_root()
            .join(GENERATED_BLOB_DIRECTORY_NAME)
            .join(&sha256[..2])
            .join(format!("{sha256}.png")))
    }

    /// Returns the native generated-image storage root beside the SQLite store.
    fn generated_asset_root(&self) -> PathBuf {
        self.path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(GENERATED_ASSET_DIRECTORY_NAME)
    }
}

/// Inserts one pending assistant image message and its exact request record in an existing transaction.
pub(super) fn insert_generated_image_message(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    branch_id: &str,
    request_message_id: &str,
    output_count: u8,
    provenance: &GeneratedImageProvenance,
    options: &GeneratedImageRequestOptions,
) -> Result<String, StorageError> {
    validate_output_count(output_count)?;
    let parent_message_id: String = transaction.query_row(
        "SELECT id FROM messages WHERE branch_id = ?1 ORDER BY sequence DESC LIMIT 1",
        [branch_id],
        |row| row.get(0),
    )?;
    let sequence: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(sequence), -1) + 1 FROM messages WHERE branch_id = ?1",
        [branch_id],
        |row| row.get(0),
    )?;
    let message_id = uuid::Uuid::new_v4().to_string();
    let created_at_ms = now_ms()?;
    transaction.execute(
        "INSERT INTO messages
         (id, conversation_id, branch_id, parent_message_id, role, state, provider_id, model_id,
          created_at_ms, sequence, provider_run_id)
         VALUES (?1, ?2, ?3, ?4, 'assistant', 'partial', ?5, ?6, ?7, ?8, NULL)",
        params![
            message_id,
            conversation_id,
            branch_id,
            parent_message_id,
            provenance.provider_id,
            provenance.model_id,
            created_at_ms,
            sequence,
        ],
    )?;
    transaction.execute(
        "INSERT INTO message_blocks (id, message_id, ordinal, block_type, text_content)
         VALUES (?1, ?2, 0, 'text', 'Generating image…')",
        params![uuid::Uuid::new_v4().to_string(), message_id],
    )?;
    transaction.execute(
        "INSERT INTO generated_image_requests
         (message_id, request_message_id, width, height, prompt_extend)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            message_id,
            request_message_id,
            options.width,
            options.height,
            options.prompt_extend,
        ],
    )?;
    for ordinal in 0..output_count {
        transaction.execute(
            "INSERT INTO generated_assets
             (id, message_id, ordinal, status, provider_id, model_id, execution, seed,
              created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, 'pending', ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                uuid::Uuid::new_v4().to_string(),
                message_id,
                ordinal,
                provenance.provider_id,
                provenance.model_id,
                provenance.execution.as_str(),
                provenance.seed,
                created_at_ms,
            ],
        )?;
    }
    transaction.execute(
        "UPDATE conversations SET updated_at_ms = ?1, archived_at_ms = NULL WHERE id = ?2",
        params![created_at_ms, conversation_id],
    )?;
    Ok(message_id)
}

/// Applies the durable output-count bound shared by first attempts and retry.
pub(super) fn validate_output_count(output_count: u8) -> Result<(), StorageError> {
    if !(MIN_OUTPUT_COUNT..=MAX_OUTPUT_COUNT).contains(&output_count) {
        return Err(StorageError::invalid(
            "Generate between 1 and 6 images at a time.",
        ));
    }
    Ok(())
}

/// Loads ordered path-free generated assets for one message.
pub(super) fn load_message_generated_assets(
    connection: &Connection,
    message_id: &str,
) -> Result<Vec<StoredGeneratedAsset>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT id, ordinal, status, media_type, width, height, byte_size, provider_id, model_id,
                execution, seed, error_code, created_at_ms, sha256
         FROM generated_assets WHERE message_id = ?1 ORDER BY ordinal",
    )?;
    let rows = statement.query_map([message_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, u8>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<u32>>(4)?,
            row.get::<_, Option<u32>>(5)?,
            row.get::<_, Option<i64>>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, String>(8)?,
            row.get::<_, String>(9)?,
            row.get::<_, Option<i64>>(10)?,
            row.get::<_, Option<String>>(11)?,
            row.get::<_, i64>(12)?,
            row.get::<_, Option<String>>(13)?,
        ))
    })?;
    rows.map(|row| {
        let row = row?;
        Ok(StoredGeneratedAsset {
            id: row.0,
            ordinal: row.1,
            status: GeneratedAssetStatus::from_database(&row.2)?,
            media_type: row.3,
            width: row.4,
            height: row.5,
            byte_size: row
                .6
                .map(u64::try_from)
                .transpose()
                .map_err(|_| StorageError::internal())?,
            provider_id: row.7,
            model_id: row.8,
            execution: GeneratedAssetExecution::from_database(&row.9)?,
            seed: row.10,
            error_code: row.11,
            created_at_ms: row.12,
            sha256: row.13,
        })
    })
    .collect()
}

/// Selects an active conversation branch with no text or image generation already pending.
pub(super) fn selected_branch_without_active_generation(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<String, StorageError> {
    let branch_id = transaction
        .query_row(
            "SELECT current_branch_id FROM conversations
             WHERE id = ?1 AND profile_id = ?2 AND deleted_at_ms IS NULL",
            params![conversation_id, DEFAULT_PROFILE_ID],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| StorageError::not_found("That conversation no longer exists."))?;
    let active: bool = transaction.query_row(
        "SELECT EXISTS (
             SELECT 1 FROM provider_runs WHERE conversation_id = ?1 AND state = 'running'
             UNION ALL
             SELECT 1 FROM generated_assets JOIN messages ON messages.id = generated_assets.message_id
             WHERE messages.conversation_id = ?1 AND generated_assets.status = 'pending'
         )",
        [conversation_id],
        |row| row.get(0),
    )?;
    if active {
        return Err(StorageError::invalid(
            "Wait for the active response to finish before generating an image.",
        ));
    }
    Ok(branch_id)
}

/// Requires one selected-lineage assistant image message whose outputs are all pending.
fn pending_generated_message_conversation(
    transaction: &Transaction<'_>,
    message_id: &str,
) -> Result<String, StorageError> {
    transaction
        .query_row(
            "SELECT messages.conversation_id FROM messages
             WHERE messages.id = ?1 AND messages.role = 'assistant' AND messages.state = 'partial'
               AND EXISTS (
                   SELECT 1 FROM generated_assets
                   WHERE generated_assets.message_id = messages.id
                     AND generated_assets.status = 'pending'
               )",
            [message_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| StorageError::invalid("That image generation is no longer active."))
}

/// Updates the assistant message and its text block after one terminal image outcome.
fn finish_generated_message(
    transaction: &Transaction<'_>,
    message_id: &str,
    state: MessageState,
    label: &str,
) -> Result<(), StorageError> {
    let changed = transaction.execute(
        "UPDATE messages SET state = ?1 WHERE id = ?2 AND role = 'assistant' AND state = 'partial'",
        params![state.as_str(), message_id],
    )?;
    if changed != 1 {
        return Err(StorageError::generated_image());
    }
    transaction.execute(
        "UPDATE message_blocks SET text_content = ?1
         WHERE message_id = ?2 AND ordinal = 0 AND block_type = 'text'",
        params![label, message_id],
    )?;
    memory_chunks::refresh_message_chunks(transaction, message_id)?;
    Ok(())
}

/// Reloads one newly appended image message through the ordinary conversation contract.
pub(super) fn load_generated_message(
    connection: &Connection,
    conversation_id: &str,
    message_id: &str,
) -> Result<StoredMessage, StorageError> {
    load_conversation_from_connection(connection, conversation_id)?
        .messages
        .into_iter()
        .find(|message| message.id == message_id)
        .ok_or_else(StorageError::internal)
}

/// Rechecks exact staged bytes before trusting caller-supplied file metadata.
fn verify_prepared_image(image: &PreparedGeneratedImage) -> Result<(), StorageError> {
    verify_content_file(&image.temporary_path, image.byte_size, &image.sha256)
}

/// Verifies exact size and SHA-256 identity without exposing a native path in errors.
pub(super) fn verify_content_file(
    path: &Path,
    byte_size: u64,
    sha256: &str,
) -> Result<(), StorageError> {
    let bytes = fs::read(path).map_err(|_| StorageError::generated_image())?;
    if bytes.len() as u64 != byte_size || format!("{:x}", Sha256::digest(bytes)) != sha256 {
        return Err(StorageError::generated_image());
    }
    Ok(())
}

/// Removes only files created by the current failed commit attempt.
fn cleanup_new_files(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}
