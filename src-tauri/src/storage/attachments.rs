//! Content-addressed application-private attachment ingestion.

use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    ConversationStore, DEFAULT_PROFILE_ID, StorageError, StoredAttachment, StoredMessage,
    StoredRole, load_conversation_from_connection, now_ms,
};

const ATTACHMENT_DIRECTORY_NAME: &str = "attachments";
const BLOB_DIRECTORY_NAME: &str = "blobs";
const TEMPORARY_DIRECTORY_NAME: &str = "temporary";
const COPY_BUFFER_BYTES: usize = 64 * 1024;
const MIME_SNIFF_BYTES: usize = 8 * 1024;
const MAX_DISPLAY_NAME_CHARACTERS: usize = 120;
const BYTES_PER_MEBIBYTE: u64 = 1024 * 1024;
const MAX_ATTACHMENT_MEBIBYTES: u64 = 25;

/// Maximum bytes accepted for one selected attachment.
pub(super) const MAX_ATTACHMENT_BYTES: u64 = MAX_ATTACHMENT_MEBIBYTES * BYTES_PER_MEBIBYTE;
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
        let attachment = IngestedAttachment {
            id: uuid::Uuid::new_v4().to_string(),
            display_name: prepared.display_name,
            mime_type: prepared.mime_type,
            byte_size: prepared.byte_size,
            sha256: prepared.sha256,
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
        if let Err(error) = transaction.commit() {
            let _ = fs::remove_file(&blob_path);
            return Err(error.into());
        }
        Ok(attachment)
    }

    /// Returns the application-private attachment directory beside the SQLite store.
    fn attachment_root(&self) -> PathBuf {
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
                attachments.byte_size, attachments.sha256
         FROM message_attachments
         JOIN attachments ON attachments.id = message_attachments.attachment_id
         WHERE message_attachments.message_id = ?1
         ORDER BY message_attachments.ordinal",
    )?;
    statement
        .query_map([message_id], stored_attachment_from_row)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

/// Loads one retained attachment identity without exposing its blob path.
fn stored_attachment(
    connection: &Connection,
    attachment_id: &str,
) -> Result<Option<StoredAttachment>, StorageError> {
    connection
        .query_row(
            "SELECT id, display_name, mime_type, byte_size, sha256 FROM attachments WHERE id = ?1",
            [attachment_id],
            stored_attachment_from_row,
        )
        .optional()
        .map_err(Into::into)
}

/// Decodes trusted attachment metadata shared by direct and message-scoped queries.
fn stored_attachment_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredAttachment> {
    Ok(StoredAttachment {
        id: row.get(0)?,
        display_name: row.get(1)?,
        mime_type: row.get(2)?,
        byte_size: row.get::<_, i64>(3)? as u64,
        sha256: row.get(4)?,
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

/// Prepared content and safe metadata awaiting one database transaction.
struct PreparedAttachment {
    display_name: String,
    mime_type: String,
    byte_size: u64,
    sha256: String,
}

/// Rejects non-files, empty files, and files over the native policy ceiling.
fn validate_source(source_path: &Path) -> Result<(), StorageError> {
    let metadata = fs::metadata(source_path).map_err(|_| StorageError::attachment_read())?;
    if !metadata.is_file() {
        return Err(StorageError::invalid("Choose a regular file to attach."));
    }
    if metadata.len() == 0 {
        return Err(StorageError::invalid("Empty files cannot be attached."));
    }
    if metadata.len() > MAX_ATTACHMENT_BYTES {
        return Err(attachment_too_large());
    }
    Ok(())
}

/// Copies and hashes one source through a bounded streaming buffer.
fn prepare_blob(
    source_path: &Path,
    temporary_path: &Path,
    display_name: &str,
) -> Result<PreparedAttachment, StorageError> {
    let mut source = File::open(source_path).map_err(|_| StorageError::attachment_read())?;
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary_path)?;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    let mut sniffed = Vec::with_capacity(MIME_SNIFF_BYTES);
    let mut hasher = Sha256::new();
    let mut byte_size = 0_u64;
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|_| StorageError::attachment_read())?;
        if read == 0 {
            break;
        }
        byte_size = byte_size
            .checked_add(read as u64)
            .ok_or_else(attachment_too_large)?;
        if byte_size > MAX_ATTACHMENT_BYTES {
            return Err(attachment_too_large());
        }
        let sniff_remaining = MIME_SNIFF_BYTES.saturating_sub(sniffed.len());
        sniffed.extend_from_slice(&buffer[..read.min(sniff_remaining)]);
        hasher.update(&buffer[..read]);
        destination.write_all(&buffer[..read])?;
    }
    destination.sync_all()?;
    if byte_size == 0 {
        return Err(StorageError::invalid("Empty files cannot be attached."));
    }
    Ok(PreparedAttachment {
        display_name: display_name.into(),
        mime_type: detect_mime_type(&sniffed, display_name).into(),
        byte_size,
        sha256: format!("{:x}", hasher.finalize()),
    })
}

/// Loads retained metadata for one content hash.
fn find_attachment(
    connection: &rusqlite::Connection,
    sha256: &str,
) -> Result<Option<IngestedAttachment>, StorageError> {
    connection
        .query_row(
            "SELECT id, display_name, mime_type, byte_size, sha256
             FROM attachments WHERE sha256 = ?1",
            [sha256],
            |row| {
                Ok(IngestedAttachment {
                    id: row.get(0)?,
                    display_name: row.get(1)?,
                    mime_type: row.get(2)?,
                    byte_size: row.get::<_, i64>(3)? as u64,
                    sha256: row.get(4)?,
                    duplicate: false,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

/// Infers MIME from content signatures, then falls back to inert text or binary.
pub(super) fn detect_mime_type(bytes: &[u8], _display_name: &str) -> &'static str {
    if let Some(kind) = infer::get(bytes) {
        return kind.mime_type();
    }
    if !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok() {
        return "text/plain";
    }
    "application/octet-stream"
}

/// Removes path separators, controls, bidi overrides, excess whitespace, and unsafe length.
pub(crate) fn safe_display_name(value: &str) -> String {
    let filtered: String = value
        .chars()
        .filter(|character| {
            !character.is_control()
                && !matches!(character, '/' | '\\')
                && !matches!(*character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .collect();
    let normalized = filtered.split_whitespace().collect::<Vec<_>>().join(" ");
    let bounded: String = normalized
        .trim_matches(|character: char| character == '.' || character.is_whitespace())
        .chars()
        .take(MAX_DISPLAY_NAME_CHARACTERS)
        .collect();
    if bounded.is_empty() {
        "attachment".into()
    } else {
        bounded
    }
}

/// Creates the stable rejection used by both metadata and streaming size checks.
fn attachment_too_large() -> StorageError {
    StorageError::invalid(format!(
        "Attachments must be {MAX_ATTACHMENT_MEBIBYTES} MiB or smaller."
    ))
}
