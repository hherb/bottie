//! Versioned deterministic native chunks derived from durable memory sources.

use rusqlite::{OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};

use super::{ConversationStore, DEFAULT_PROFILE_ID, StorageError};

/// Active deterministic chunking contract stored with every derived row.
pub(super) const CHUNKING_VERSION: i64 = 1;
/// Maximum Unicode scalar values retained in one chunk.
pub(super) const MAX_CHUNK_CHARACTERS: usize = 1_200;
/// Preferred earliest split point when a whitespace boundary is available.
pub(super) const MIN_CHUNK_SPLIT_CHARACTERS: usize = 900;
/// Approximate Unicode overlap retained between adjacent chunks.
pub(super) const CHUNK_OVERLAP_CHARACTERS: usize = 200;
/// Stable name for the active whitespace-aware Unicode algorithm.
#[cfg(test)]
pub(super) const CHUNKING_ALGORITHM: &str = "unicode-whitespace-v1";

/// Removes the derived catalog and its cleanup triggers for migration fixtures.
#[cfg(test)]
pub(super) const REMOVE_MEMORY_CHUNK_SCHEMA_FOR_TEST: &str = r#"
DROP TRIGGER IF EXISTS memory_chunks_message_blocks_after_insert;
DROP TRIGGER IF EXISTS memory_chunks_message_blocks_after_update;
DROP TRIGGER IF EXISTS memory_chunks_message_blocks_after_delete;
DROP TRIGGER IF EXISTS memory_chunks_messages_after_state_update;
DROP TRIGGER IF EXISTS memory_chunks_messages_after_delete;
DROP TRIGGER IF EXISTS memory_chunks_extractions_after_insert;
DROP TRIGGER IF EXISTS memory_chunks_extractions_after_update;
DROP TRIGGER IF EXISTS memory_chunks_extractions_after_delete;
DROP TABLE IF EXISTS memory_chunks;
DROP TABLE IF EXISTS memory_chunk_metadata;
"#;

/// Native durable source category for a derived memory chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MemoryChunkSourceKind {
    /// Final user or assistant answer text, excluding reasoning.
    Message,
    /// Ready extracted attachment text.
    Attachment,
}

impl MemoryChunkSourceKind {
    /// Returns the stable database representation.
    fn as_str(self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::Attachment => "attachment",
        }
    }
}

/// One deterministic text slice before or after native catalog persistence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MemoryChunk {
    /// Stable SHA-256 identity over the versioned source slice.
    pub(super) id: String,
    /// Active chunking contract version.
    pub(super) chunking_version: i64,
    /// Zero-based source-local order.
    pub(super) ordinal: usize,
    /// Inclusive Unicode-scalar offset in the complete source.
    pub(super) start_character: usize,
    /// Exclusive Unicode-scalar offset in the complete source.
    pub(super) end_character: usize,
    /// Exact source slice retained for later lexical and semantic consumers.
    pub(super) text: String,
}

/// Produces stable, Unicode-safe, whitespace-aware overlapping chunks.
pub(super) fn chunks_for_text(source_kind: &str, source_id: &str, text: &str) -> Vec<MemoryChunk> {
    let characters = text.chars().collect::<Vec<_>>();
    let mut chunks = Vec::new();
    let mut start = skip_whitespace(&characters, 0);
    while start < characters.len() {
        let window_end = (start + MAX_CHUNK_CHARACTERS).min(characters.len());
        let end = chunk_end(&characters, start, window_end);
        if end <= start {
            break;
        }
        let chunk_text = characters[start..end].iter().collect::<String>();
        chunks.push(MemoryChunk {
            id: chunk_id(
                source_kind,
                source_id,
                chunks.len(),
                start,
                end,
                &chunk_text,
            ),
            chunking_version: CHUNKING_VERSION,
            ordinal: chunks.len(),
            start_character: start,
            end_character: end,
            text: chunk_text,
        });
        if end == characters.len() {
            break;
        }
        start = next_chunk_start(&characters, start, end);
    }
    chunks
}

/// Backfills every eligible durable source inside the schema migration transaction.
pub(super) fn backfill_memory_chunks(transaction: &Transaction<'_>) -> Result<(), StorageError> {
    let message_ids = source_ids(
        transaction,
        "SELECT id FROM messages WHERE state = 'final' ORDER BY id",
    )?;
    for message_id in message_ids {
        refresh_message_chunks_before_exclusion_preferences(transaction, &message_id)?;
    }
    let attachment_ids = source_ids(
        transaction,
        "SELECT attachment_id FROM attachment_extractions WHERE state = 'ready' ORDER BY attachment_id",
    )?;
    for attachment_id in attachment_ids {
        refresh_attachment_chunks(transaction, &attachment_id)?;
    }
    Ok(())
}

/// Atomically replaces the derived chunks for one message when it is eligible.
pub(super) fn refresh_message_chunks(
    transaction: &Transaction<'_>,
    message_id: &str,
) -> Result<(), StorageError> {
    refresh_message_chunks_with_policy(transaction, message_id, true)
}

/// Rebuilds one message during schema-17 backfill, before schema-19 preferences exist.
fn refresh_message_chunks_before_exclusion_preferences(
    transaction: &Transaction<'_>,
    message_id: &str,
) -> Result<(), StorageError> {
    refresh_message_chunks_with_policy(transaction, message_id, false)
}

/// Replaces one message's chunks under the schema version available to the caller.
fn refresh_message_chunks_with_policy(
    transaction: &Transaction<'_>,
    message_id: &str,
    enforce_exclusion: bool,
) -> Result<(), StorageError> {
    delete_source_chunks(transaction, MemoryChunkSourceKind::Message, message_id)?;
    let source = transaction
        .query_row(
            "SELECT conversations.profile_id, conversations.id, messages.created_at_ms,
                    (SELECT group_concat(ordered_blocks.text_content, '')
                     FROM (
                         SELECT text_content FROM message_blocks
                         WHERE message_id = messages.id AND block_type = 'text'
                         ORDER BY ordinal
                     ) AS ordered_blocks)
             FROM messages
             JOIN conversations ON conversations.id = messages.conversation_id
             WHERE messages.id = ?1 AND messages.state = 'final'
               AND EXISTS (
                   SELECT 1 FROM message_blocks
                   WHERE message_id = messages.id AND block_type = 'text'
                     AND length(trim(text_content)) > 0
               )",
            [message_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    if let Some((profile_id, conversation_id, created_at_ms, text)) = source {
        if enforce_exclusion && conversation_memory_is_excluded(transaction, &conversation_id)? {
            return Ok(());
        }
        insert_source_chunks(
            transaction,
            MemoryChunkSourceKind::Message,
            message_id,
            &profile_id,
            created_at_ms,
            &text,
        )?;
    }
    Ok(())
}

/// Reads the schema-19 preference only for current-runtime chunk refreshes.
fn conversation_memory_is_excluded(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<bool, StorageError> {
    transaction
        .query_row(
            "SELECT EXISTS (
                 SELECT 1 FROM conversation_memory_preferences
                 WHERE conversation_id = ?1 AND excluded = 1
             )",
            [conversation_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

/// Atomically replaces the derived chunks for one ready extracted document.
pub(super) fn refresh_attachment_chunks(
    transaction: &Transaction<'_>,
    attachment_id: &str,
) -> Result<(), StorageError> {
    delete_source_chunks(
        transaction,
        MemoryChunkSourceKind::Attachment,
        attachment_id,
    )?;
    let source = transaction
        .query_row(
            "SELECT attachments.created_at_ms, attachment_extractions.text_content
             FROM attachments
             JOIN attachment_extractions
               ON attachment_extractions.attachment_id = attachments.id
             WHERE attachments.id = ?1 AND attachment_extractions.state = 'ready'
               AND length(trim(attachment_extractions.text_content)) > 0",
            [attachment_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    if let Some((created_at_ms, text)) = source {
        insert_source_chunks(
            transaction,
            MemoryChunkSourceKind::Attachment,
            attachment_id,
            DEFAULT_PROFILE_ID,
            created_at_ms,
            &text,
        )?;
    }
    Ok(())
}

impl ConversationStore {
    /// Loads one source's native chunks for storage contract tests.
    #[cfg(test)]
    pub(super) fn memory_chunks_for_source_for_test(
        &self,
        source_kind: MemoryChunkSourceKind,
        source_id: &str,
    ) -> Result<Vec<MemoryChunk>, StorageError> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT id, chunking_version, ordinal, start_character, end_character, text_content
             FROM memory_chunks
             WHERE source_kind = ?1 AND source_id = ?2
             ORDER BY ordinal",
        )?;
        statement
            .query_map(params![source_kind.as_str(), source_id], |row| {
                Ok(MemoryChunk {
                    id: row.get(0)?,
                    chunking_version: row.get(1)?,
                    ordinal: row.get::<_, i64>(2)? as usize,
                    start_character: row.get::<_, i64>(3)? as usize,
                    end_character: row.get::<_, i64>(4)? as usize,
                    text: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

/// Collects bounded source identities before catalog inserts mutate the same transaction.
fn source_ids(transaction: &Transaction<'_>, sql: &str) -> Result<Vec<String>, StorageError> {
    let mut statement = transaction.prepare(sql)?;
    let rows = statement.query_map([], |row| row.get(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Inserts one complete replacement set using stable source-local order.
fn insert_source_chunks(
    transaction: &Transaction<'_>,
    source_kind: MemoryChunkSourceKind,
    source_id: &str,
    profile_id: &str,
    created_at_ms: i64,
    text: &str,
) -> Result<(), StorageError> {
    for chunk in chunks_for_text(source_kind.as_str(), source_id, text) {
        let content_sha256 = format!("{:x}", Sha256::digest(chunk.text.as_bytes()));
        transaction.execute(
            "INSERT INTO memory_chunks
             (id, source_kind, source_id, profile_id, chunking_version, ordinal,
              start_character, end_character, text_content, content_sha256, source_created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                chunk.id,
                source_kind.as_str(),
                source_id,
                profile_id,
                chunk.chunking_version,
                chunk.ordinal as i64,
                chunk.start_character as i64,
                chunk.end_character as i64,
                chunk.text,
                content_sha256,
                created_at_ms,
            ],
        )?;
    }
    Ok(())
}

/// Deletes one source's old derived rows before an eligible replacement is inserted.
fn delete_source_chunks(
    transaction: &Transaction<'_>,
    source_kind: MemoryChunkSourceKind,
    source_id: &str,
) -> Result<(), StorageError> {
    transaction.execute(
        "DELETE FROM memory_chunks WHERE source_kind = ?1 AND source_id = ?2",
        params![source_kind.as_str(), source_id],
    )?;
    Ok(())
}

/// Chooses a trailing whitespace boundary without producing an undersized non-final chunk.
fn chunk_end(characters: &[char], start: usize, window_end: usize) -> usize {
    if window_end == characters.len() {
        return trim_trailing_whitespace(characters, start, window_end);
    }
    let earliest_split = (start + MIN_CHUNK_SPLIT_CHARACTERS).min(window_end);
    let boundary = (earliest_split..window_end)
        .rev()
        .find(|index| characters[*index].is_whitespace())
        .unwrap_or(window_end);
    trim_trailing_whitespace(characters, start, boundary)
}

/// Finds a word-aligned overlapping start while guaranteeing forward progress.
fn next_chunk_start(characters: &[char], previous_start: usize, previous_end: usize) -> usize {
    let desired = previous_end
        .saturating_sub(CHUNK_OVERLAP_CHARACTERS)
        .max(previous_start + 1);
    let mut aligned = desired;
    while aligned > previous_start + 1 && !characters[aligned - 1].is_whitespace() {
        aligned -= 1;
    }
    let candidate = skip_whitespace(characters, aligned);
    if candidate >= previous_end {
        desired
    } else {
        candidate
    }
}

/// Skips Unicode whitespace at a candidate chunk start.
fn skip_whitespace(characters: &[char], mut index: usize) -> usize {
    while index < characters.len() && characters[index].is_whitespace() {
        index += 1;
    }
    index
}

/// Removes only boundary whitespace while retaining exact interior source bytes.
fn trim_trailing_whitespace(characters: &[char], start: usize, mut end: usize) -> usize {
    while end > start && characters[end - 1].is_whitespace() {
        end -= 1;
    }
    end
}

/// Hashes a canonical length-prefixed source-slice identity.
fn chunk_id(
    source_kind: &str,
    source_id: &str,
    ordinal: usize,
    start: usize,
    end: usize,
    text: &str,
) -> String {
    let mut hasher = Sha256::new();
    for field in [
        CHUNKING_VERSION.to_string(),
        source_kind.to_owned(),
        source_id.to_owned(),
        ordinal.to_string(),
        start.to_string(),
        end.to_string(),
        text.to_owned(),
    ] {
        hasher.update((field.len() as u64).to_be_bytes());
        hasher.update(field.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}
