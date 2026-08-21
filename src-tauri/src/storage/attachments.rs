//! Content-addressed application-private attachment ingestion.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;

use super::{
    AttachmentExtractionFormat, AttachmentExtractionState, ConversationStore, DEFAULT_PROFILE_ID,
    StorageError, StoredAttachment, StoredAttachmentExtraction, StoredImageNormalization,
    StoredMessage, StoredRole,
    attachment_policy::{PreparedAttachment, prepare_blob, validate_source},
    load_conversation_from_connection, now_ms,
};

pub(crate) use super::attachment_policy::safe_display_name;
#[cfg(test)]
pub(super) use super::attachment_policy::{MAX_ATTACHMENT_BYTES, detect_mime_type};

const ATTACHMENT_DIRECTORY_NAME: &str = "attachments";
const BLOB_DIRECTORY_NAME: &str = "blobs";
const TEMPORARY_DIRECTORY_NAME: &str = "temporary";
/// Maximum files accepted by one native picker interaction.
pub(crate) const MAX_ATTACHMENT_SELECTION_COUNT: usize = 8;

/// Safe attachment metadata returned without source or application-private paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IngestedAttachment {
    /// Opaque native identity for later attachment association.
    pub(crate) id: String,
    /// Sanitized leaf name safe for inert interface display.
    pub(crate) display_name: String,
    /// MIME type inferred from content rather than browser or extension claims.
    pub(crate) mime_type: String,
    /// Exact byte count retained in application-private storage.
    pub(crate) byte_size: u64,
    /// Lowercase SHA-256 content identity.
    pub(crate) sha256: String,
    /// Native-only extraction status; extracted content is deliberately omitted.
    pub(crate) extraction: StoredAttachmentExtraction,
    /// Native-only image normalization status; derivative bytes and paths are omitted.
    pub(crate) normalization: StoredImageNormalization,
    /// Whether this selection reused an already retained content blob.
    pub(crate) duplicate: bool,
}

impl ConversationStore {
    /// Streams one local file into the content-addressed attachment store.
    pub(crate) fn ingest_attachment(
        &self,
        source_path: &Path,
    ) -> Result<IngestedAttachment, StorageError> {
        validate_source(source_path)?;
        let display_name = source_path
            .file_name()
            .map(|name| safe_display_name(&name.to_string_lossy()))
            .unwrap_or_else(|| "attachment".into());
        let temporary_directory = self.attachment_root().join(TEMPORARY_DIRECTORY_NAME);
        fs::create_dir_all(&temporary_directory)?;
        let temporary_path = temporary_directory.join(format!("{}.part", uuid::Uuid::new_v4()));
        let prepared = prepare_blob(source_path, &temporary_path, &display_name);
        if prepared.is_err() {
            let _ = fs::remove_file(&temporary_path);
        }
        let prepared = prepared?;
        let result = self.commit_blob(&temporary_path, prepared);
        let _ = fs::remove_file(&temporary_path);
        result
    }

    /// Removes one visible user-message association while retaining catalog metadata and bytes.
    pub(crate) fn remove_message_attachment(
        &self,
        conversation_id: &str,
        message_id: &str,
        attachment_id: &str,
    ) -> Result<StoredMessage, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        ensure_no_active_run(&transaction, conversation_id)?;
        let branch_id = selected_branch_id(&transaction, conversation_id)?;
        if !visible_user_message_has_attachment(
            &transaction,
            &branch_id,
            message_id,
            attachment_id,
        )? {
            return Err(StorageError::not_found(
                "That message attachment is unavailable.",
            ));
        }
        transaction.execute(
            "DELETE FROM message_attachments WHERE message_id = ?1 AND attachment_id = ?2",
            params![message_id, attachment_id],
        )?;
        let conversation = load_conversation_from_connection(&transaction, conversation_id)?;
        let message = conversation
            .messages
            .into_iter()
            .find(|message| message.id == message_id)
            .ok_or_else(StorageError::internal)?;
        transaction.commit()?;
        Ok(message)
    }

    /// Commits prepared bytes and metadata while preventing duplicate content rows.
    fn commit_blob(
        &self,
        temporary_path: &Path,
        prepared: PreparedAttachment,
    ) -> Result<IngestedAttachment, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(existing) = find_attachment(&transaction, &prepared.sha256)? {
            return Ok(IngestedAttachment {
                duplicate: true,
                ..existing
            });
        }

        let blob_path = self.attachment_blob_path(&prepared.sha256);
        let blob_directory = blob_path.parent().ok_or_else(StorageError::internal)?;
        fs::create_dir_all(blob_directory)?;
        if blob_path.exists() {
            fs::remove_file(&blob_path)?;
        }
        fs::rename(temporary_path, &blob_path)?;
        let normalization = StoredImageNormalization::pending_or_unsupported(&prepared.mime_type);
        let attachment = IngestedAttachment {
            id: uuid::Uuid::new_v4().to_string(),
            display_name: prepared.display_name,
            mime_type: prepared.mime_type,
            byte_size: prepared.byte_size,
            sha256: prepared.sha256,
            extraction: StoredAttachmentExtraction {
                state: AttachmentExtractionState::Pending,
                format: None,
                character_count: None,
                page_count: None,
                error_code: None,
            },
            normalization,
            duplicate: false,
        };
        let inserted = transaction.execute(
            "INSERT INTO attachments (id, sha256, display_name, mime_type, byte_size, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                attachment.id,
                attachment.sha256,
                attachment.display_name,
                attachment.mime_type,
                attachment.byte_size as i64,
                now_ms()?
            ],
        );
        if inserted.is_err() {
            let _ = fs::remove_file(&blob_path);
        }
        inserted?;
        transaction.execute(
            "INSERT INTO attachment_extractions (attachment_id, state, updated_at_ms)
             VALUES (?1, 'pending', ?2)",
            params![attachment.id, now_ms()?],
        )?;
        transaction.execute(
            "INSERT INTO attachment_image_normalizations (attachment_id, state, updated_at_ms)
             VALUES (?1, ?2, ?3)",
            params![
                attachment.id,
                attachment.normalization.state.as_str(),
                now_ms()?
            ],
        )?;
        if let Err(error) = transaction.commit() {
            let _ = fs::remove_file(&blob_path);
            return Err(error.into());
        }
        Ok(attachment)
    }

    /// Returns the application-private attachment directory beside the SQLite store.
    pub(super) fn attachment_root(&self) -> PathBuf {
        self.path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(ATTACHMENT_DIRECTORY_NAME)
    }

    /// Resolves a content hash to its sharded application-private blob location.
    pub(super) fn attachment_blob_path(&self, sha256: &str) -> PathBuf {
        self.attachment_root()
            .join(BLOB_DIRECTORY_NAME)
            .join(&sha256[..2])
            .join(sha256)
    }

    /// Counts retained attachment metadata for storage contract tests.
    #[cfg(test)]
    pub(super) fn attachment_count(&self) -> Result<i64, StorageError> {
        self.open()?
            .query_row("SELECT COUNT(*) FROM attachments", [], |row| row.get(0))
            .map_err(Into::into)
    }

    /// Loads path-free attachment metadata for storage contract tests.
    #[cfg(test)]
    pub(super) fn stored_attachment_for_test(
        &self,
        attachment_id: &str,
    ) -> Result<Option<StoredAttachment>, StorageError> {
        stored_attachment(&self.open()?, attachment_id)
    }
}

/// Validates and inserts ordered attachment associations for one newly appended message.
pub(super) fn associate_message_attachments(
    transaction: &Transaction<'_>,
    message_id: &str,
    role: StoredRole,
    attachment_ids: &[String],
) -> Result<Vec<StoredAttachment>, StorageError> {
    if attachment_ids.is_empty() {
        return Ok(Vec::new());
    }
    if role != StoredRole::User {
        return Err(StorageError::invalid(
            "Attachments can be associated only with user messages.",
        ));
    }
    if attachment_ids.len() > MAX_ATTACHMENT_SELECTION_COUNT {
        return Err(StorageError::invalid(format!(
            "Attach at most {MAX_ATTACHMENT_SELECTION_COUNT} files to one message."
        )));
    }
    let unique_ids = attachment_ids.iter().collect::<HashSet<_>>();
    if unique_ids.len() != attachment_ids.len() {
        return Err(StorageError::invalid(
            "The same attachment cannot be associated twice.",
        ));
    }
    let attached_at_ms = now_ms()?;
    let mut attachments = Vec::with_capacity(attachment_ids.len());
    for (ordinal, attachment_id) in attachment_ids.iter().enumerate() {
        let attachment = stored_attachment(transaction, attachment_id)?.ok_or_else(|| {
            StorageError::invalid("One or more selected attachments are unavailable.")
        })?;
        transaction.execute(
            "INSERT INTO message_attachments (message_id, attachment_id, ordinal, attached_at_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![message_id, attachment_id, ordinal as i64, attached_at_ms],
        )?;
        attachments.push(attachment);
    }
    Ok(attachments)
}

/// Reconstructs ordered path-free metadata for one durable message.
pub(super) fn load_message_attachments(
    connection: &Connection,
    message_id: &str,
) -> Result<Vec<StoredAttachment>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT attachments.id, attachments.display_name, attachments.mime_type,
                attachments.byte_size, attachments.sha256, attachment_extractions.state,
                attachment_extractions.format, attachment_extractions.character_count,
                attachment_extractions.page_count, attachment_extractions.error_code,
                attachment_image_normalizations.state, attachment_image_normalizations.format,
                attachment_image_normalizations.width, attachment_image_normalizations.height,
                attachment_image_normalizations.byte_size, attachment_image_normalizations.error_code
         FROM message_attachments
         JOIN attachments ON attachments.id = message_attachments.attachment_id
         JOIN attachment_extractions ON attachment_extractions.attachment_id = attachments.id
         JOIN attachment_image_normalizations
           ON attachment_image_normalizations.attachment_id = attachments.id
         WHERE message_attachments.message_id = ?1
         ORDER BY message_attachments.ordinal",
    )?;
    statement
        .query_map([message_id], stored_attachment_from_row)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

/// Loads one retained attachment identity without exposing its blob path.
pub(super) fn stored_attachment(
    connection: &Connection,
    attachment_id: &str,
) -> Result<Option<StoredAttachment>, StorageError> {
    connection
        .query_row(
            "SELECT attachments.id, attachments.display_name, attachments.mime_type,
                    attachments.byte_size, attachments.sha256, attachment_extractions.state,
                    attachment_extractions.format, attachment_extractions.character_count,
                    attachment_extractions.page_count, attachment_extractions.error_code,
                    attachment_image_normalizations.state, attachment_image_normalizations.format,
                    attachment_image_normalizations.width, attachment_image_normalizations.height,
                    attachment_image_normalizations.byte_size, attachment_image_normalizations.error_code
             FROM attachments
             JOIN attachment_extractions ON attachment_extractions.attachment_id = attachments.id
             JOIN attachment_image_normalizations
               ON attachment_image_normalizations.attachment_id = attachments.id
             WHERE attachments.id = ?1",
            [attachment_id],
            stored_attachment_from_row,
        )
        .optional()
        .map_err(Into::into)
}

/// Decodes trusted attachment metadata shared by direct and message-scoped queries.
fn stored_attachment_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredAttachment> {
    let state = AttachmentExtractionState::from_database(&row.get::<_, String>(5)?)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let format = row
        .get::<_, Option<String>>(6)?
        .as_deref()
        .map(AttachmentExtractionFormat::from_database)
        .transpose()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(StoredAttachment {
        id: row.get(0)?,
        display_name: row.get(1)?,
        mime_type: row.get(2)?,
        byte_size: row.get::<_, i64>(3)? as u64,
        sha256: row.get(4)?,
        extraction: StoredAttachmentExtraction {
            state,
            format,
            character_count: row.get::<_, Option<i64>>(7)?.map(|count| count as u64),
            page_count: row.get::<_, Option<i64>>(8)?.map(|count| count as u64),
            error_code: row.get(9)?,
        },
        normalization: StoredImageNormalization::from_row(row, 10)?,
    })
}

/// Resolves the selected branch of one editable local-profile conversation.
fn selected_branch_id(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<String, StorageError> {
    transaction
        .query_row(
            "SELECT current_branch_id FROM conversations
             WHERE id = ?1 AND profile_id = ?2 AND deleted_at_ms IS NULL",
            params![conversation_id, DEFAULT_PROFILE_ID],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| StorageError::not_found("That conversation no longer exists."))
}

/// Prevents context mutation while a response is still linked to the request.
fn ensure_no_active_run(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<(), StorageError> {
    let has_active_run: bool = transaction.query_row(
        "SELECT EXISTS (
             SELECT 1 FROM provider_runs WHERE conversation_id = ?1 AND state = 'running'
         )",
        [conversation_id],
        |row| row.get(0),
    )?;
    if has_active_run {
        return Err(StorageError::invalid(
            "Wait for the active response to finish before removing message context.",
        ));
    }
    Ok(())
}

/// Confirms that an association belongs to a visible selected-lineage user message.
fn visible_user_message_has_attachment(
    transaction: &Transaction<'_>,
    branch_id: &str,
    message_id: &str,
    attachment_id: &str,
) -> Result<bool, StorageError> {
    transaction
        .query_row(
            "WITH RECURSIVE lineage(id, parent_message_id) AS (
                 SELECT id, parent_message_id FROM messages
                 WHERE id = (
                     SELECT id FROM messages WHERE branch_id = ?1 ORDER BY sequence DESC LIMIT 1
                 )
                 UNION ALL
                 SELECT messages.id, messages.parent_message_id
                 FROM messages JOIN lineage ON messages.id = lineage.parent_message_id
             )
             SELECT EXISTS (
                 SELECT 1 FROM lineage
                 JOIN messages ON messages.id = lineage.id
                 JOIN message_attachments ON message_attachments.message_id = messages.id
                 WHERE messages.id = ?2 AND messages.role = 'user'
                   AND message_attachments.attachment_id = ?3
             )",
            params![branch_id, message_id, attachment_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

/// Loads retained metadata for one content hash.
fn find_attachment(
    connection: &rusqlite::Connection,
    sha256: &str,
) -> Result<Option<IngestedAttachment>, StorageError> {
    connection
        .query_row(
            "SELECT attachments.id, attachments.display_name, attachments.mime_type,
                    attachments.byte_size, attachments.sha256, attachment_extractions.state,
                    attachment_extractions.format, attachment_extractions.character_count,
                    attachment_extractions.page_count, attachment_extractions.error_code,
                    attachment_image_normalizations.state, attachment_image_normalizations.format,
                    attachment_image_normalizations.width, attachment_image_normalizations.height,
                    attachment_image_normalizations.byte_size, attachment_image_normalizations.error_code
             FROM attachments
             JOIN attachment_extractions ON attachment_extractions.attachment_id = attachments.id
             JOIN attachment_image_normalizations
               ON attachment_image_normalizations.attachment_id = attachments.id
             WHERE attachments.sha256 = ?1",
            [sha256],
            |row| {
                stored_attachment_from_row(row)
                    .map(|attachment| ingested_from_stored(attachment, false))
            },
        )
        .optional()
        .map_err(Into::into)
}

/// Adds the selection-specific duplicate flag to shared path-free metadata.
fn ingested_from_stored(attachment: StoredAttachment, duplicate: bool) -> IngestedAttachment {
    IngestedAttachment {
        id: attachment.id,
        display_name: attachment.display_name,
        mime_type: attachment.mime_type,
        byte_size: attachment.byte_size,
        sha256: attachment.sha256,
        extraction: attachment.extraction,
        normalization: attachment.normalization,
        duplicate,
    }
}
