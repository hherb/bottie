//! Selected-branch open, export, and deletion actions for completed generated images.

use std::{fs, path::PathBuf};

use rusqlite::{OptionalExtension, params};

use super::{
    ConversationStore, DEFAULT_PROFILE_ID, StorageError, StoredConversation,
    generated_assets::verify_content_file, load_conversation_from_connection, memory_chunks,
};

/// Native-only generated PNG resolved from an opaque selected-branch asset identity.
pub(crate) struct GeneratedAssetFile {
    /// Application-private source path that never crosses IPC.
    pub(crate) path: PathBuf,
    /// Safe suggested export leaf filename.
    pub(crate) file_name: String,
}

impl ConversationStore {
    /// Resolves and revalidates one completed selected-branch PNG for a native action.
    pub(crate) fn generated_asset_file(
        &self,
        asset_id: &str,
    ) -> Result<GeneratedAssetFile, StorageError> {
        let record = self
            .open()?
            .query_row(
                "SELECT generated_assets.sha256, generated_assets.byte_size,
                        generated_assets.ordinal
                 FROM generated_assets
                 JOIN messages ON messages.id = generated_assets.message_id
                 JOIN conversations ON conversations.id = messages.conversation_id
                 WHERE generated_assets.id = ?1 AND generated_assets.status = 'completed'
                   AND messages.branch_id = conversations.current_branch_id
                   AND conversations.profile_id = ?2 AND conversations.deleted_at_ms IS NULL",
                params![asset_id, DEFAULT_PROFILE_ID],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, u8>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| {
                StorageError::not_found("That generated image is no longer available.")
            })?;
        let byte_size = u64::try_from(record.1).map_err(|_| StorageError::internal())?;
        let path = self.generated_asset_blob_path(&record.0)?;
        verify_content_file(&path, byte_size, &record.0)?;
        Ok(GeneratedAssetFile {
            path,
            file_name: format!("bottie-generated-image-{}.png", u16::from(record.2) + 1),
        })
    }

    /// Deletes one completed selected-branch output and its final unreferenced blob atomically.
    pub(crate) fn delete_generated_asset(
        &self,
        asset_id: &str,
    ) -> Result<StoredConversation, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let record = transaction
            .query_row(
                "SELECT messages.conversation_id, messages.id, messages.parent_message_id,
                        generated_assets.sha256, generated_assets.byte_size
                 FROM generated_assets
                 JOIN messages ON messages.id = generated_assets.message_id
                 JOIN conversations ON conversations.id = messages.conversation_id
                 WHERE generated_assets.id = ?1 AND generated_assets.status = 'completed'
                   AND messages.branch_id = conversations.current_branch_id
                   AND conversations.profile_id = ?2 AND conversations.deleted_at_ms IS NULL",
                params![asset_id, DEFAULT_PROFILE_ID],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| {
                StorageError::not_found("That generated image is no longer available.")
            })?;
        let byte_size = u64::try_from(record.4).map_err(|_| StorageError::internal())?;
        let blob_path = self.generated_asset_blob_path(&record.3)?;
        verify_content_file(&blob_path, byte_size, &record.3)?;
        transaction.execute("DELETE FROM generated_assets WHERE id = ?1", [asset_id])?;
        let remaining: u8 = transaction.query_row(
            "SELECT COUNT(*) FROM generated_assets WHERE message_id = ?1",
            [&record.1],
            |row| row.get(0),
        )?;
        if remaining == 0 {
            transaction.execute(
                "UPDATE messages SET parent_message_id = ?1 WHERE parent_message_id = ?2",
                params![record.2, record.1],
            )?;
            transaction.execute("DELETE FROM messages WHERE id = ?1", [&record.1])?;
        } else {
            let label = if remaining == 1 {
                "Generated image."
            } else {
                "Generated images."
            };
            transaction.execute(
                "UPDATE message_blocks SET text_content = ?1
                 WHERE message_id = ?2 AND ordinal = 0 AND block_type = 'text'",
                params![label, record.1],
            )?;
            memory_chunks::refresh_message_chunks(&transaction, &record.1)?;
        }
        let referenced: bool = transaction.query_row(
            "SELECT EXISTS (SELECT 1 FROM generated_assets WHERE sha256 = ?1)",
            [&record.3],
            |row| row.get(0),
        )?;
        let conversation = load_conversation_from_connection(&transaction, &record.0)?;
        let tombstone = if referenced {
            None
        } else {
            let directory = self.generated_asset_temporary_directory();
            fs::create_dir_all(&directory).map_err(|_| StorageError::generated_image())?;
            let tombstone = directory.join(format!("{}.deleted", uuid::Uuid::new_v4()));
            fs::rename(&blob_path, &tombstone).map_err(|_| StorageError::generated_image())?;
            Some(tombstone)
        };
        if let Err(error) = transaction.commit() {
            if let Some(tombstone) = &tombstone {
                let _ = fs::rename(tombstone, &blob_path);
            }
            return Err(error.into());
        }
        if let Some(tombstone) = tombstone {
            let _ = fs::remove_file(tombstone);
        }
        Ok(conversation)
    }
}
