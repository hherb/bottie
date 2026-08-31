//! Optional durable retention of one Rust-owned microphone capture.

use std::{fs, io::Write};

use rusqlite::params;
use sha2::{Digest, Sha256};

use super::{
    ConversationStore, DEFAULT_PROFILE_ID, StorageError, StoredMessage,
    attachment_policy::{MAX_ATTACHMENT_BYTES, PreparedAttachment},
    attachments::{
        IngestedAttachment, MAX_ATTACHMENT_SELECTION_COUNT, TEMPORARY_DIRECTORY_NAME,
        stored_attachment,
    },
    load_conversation_from_connection, now_ms,
};

const RETAINED_AUDIO_DISPLAY_NAME: &str = "voice-recording.wav";
const RETAINED_AUDIO_MIME_TYPE: &str = "audio/wav";

impl ConversationStore {
    /// Retains one Rust-owned WAV capture without routing bytes through the WebView or a picker.
    pub(crate) fn ingest_native_audio(
        &self,
        bytes: &[u8],
    ) -> Result<IngestedAttachment, StorageError> {
        if bytes.is_empty() || bytes.len() as u64 > MAX_ATTACHMENT_BYTES {
            return Err(StorageError::invalid(
                "The captured recording is unavailable for local retention.",
            ));
        }
        let temporary_directory = self.attachment_root().join(TEMPORARY_DIRECTORY_NAME);
        fs::create_dir_all(&temporary_directory)?;
        let temporary_path = temporary_directory.join(format!("{}.part", uuid::Uuid::new_v4()));
        let mut temporary = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)?;
        temporary.write_all(bytes)?;
        temporary.sync_all()?;
        drop(temporary);
        let prepared = PreparedAttachment {
            display_name: RETAINED_AUDIO_DISPLAY_NAME.into(),
            mime_type: RETAINED_AUDIO_MIME_TYPE.into(),
            byte_size: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        };
        let result = self.commit_blob(&temporary_path, prepared);
        let _ = fs::remove_file(&temporary_path);
        result
    }

    /// Associates one retained native recording with the exact latest user request.
    pub(crate) fn associate_attachment_with_request(
        &self,
        conversation_id: &str,
        message_id: &str,
        attachment_id: &str,
    ) -> Result<StoredMessage, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let is_latest_request: bool = transaction.query_row(
            "SELECT EXISTS (
                 SELECT 1 FROM messages
                 JOIN conversations ON conversations.id = messages.conversation_id
                 WHERE messages.id = ?1 AND messages.conversation_id = ?2
                   AND messages.role = 'user' AND messages.state = 'final'
                   AND conversations.profile_id = ?3 AND conversations.deleted_at_ms IS NULL
                   AND messages.id = (
                     SELECT id FROM messages
                     WHERE branch_id = conversations.current_branch_id AND role = 'user'
                     ORDER BY sequence DESC LIMIT 1
                   )
             )",
            params![message_id, conversation_id, DEFAULT_PROFILE_ID],
            |row| row.get(0),
        )?;
        if !is_latest_request {
            return Err(StorageError::invalid(
                "The retained recording no longer matches the selected request.",
            ));
        }
        stored_attachment(&transaction, attachment_id)?
            .ok_or_else(|| StorageError::invalid("The retained recording is unavailable."))?;
        let existing: bool = transaction.query_row(
            "SELECT EXISTS (
                 SELECT 1 FROM message_attachments
                 WHERE message_id = ?1 AND attachment_id = ?2
             )",
            params![message_id, attachment_id],
            |row| row.get(0),
        )?;
        if !existing {
            let ordinal: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM message_attachments WHERE message_id = ?1",
                [message_id],
                |row| row.get(0),
            )?;
            if ordinal >= MAX_ATTACHMENT_SELECTION_COUNT as i64 {
                return Err(StorageError::invalid(format!(
                    "Attach at most {MAX_ATTACHMENT_SELECTION_COUNT} files to one message."
                )));
            }
            transaction.execute(
                "INSERT INTO message_attachments (message_id, attachment_id, ordinal, attached_at_ms)
                 VALUES (?1, ?2, ?3, ?4)",
                params![message_id, attachment_id, ordinal, now_ms()?],
            )?;
        }
        let conversation = load_conversation_from_connection(&transaction, conversation_id)?;
        let message = conversation
            .messages
            .into_iter()
            .find(|message| message.id == message_id)
            .ok_or_else(StorageError::internal)?;
        transaction.commit()?;
        Ok(message)
    }
}
