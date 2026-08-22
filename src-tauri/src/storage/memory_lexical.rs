//! Native-only bounded BM25 search over Bottie's derived SQLite FTS5 memory index.

#![allow(dead_code)]

use rusqlite::params;

use super::{ConversationStore, DEFAULT_PROFILE_ID, StorageError};

const MAX_LEXICAL_QUERY_CHARACTERS: usize = 200;
const MAX_LEXICAL_RESULTS: usize = 50;
const DEFAULT_LEXICAL_RESULTS: usize = MAX_LEXICAL_RESULTS;
const SNIPPET_TOKEN_COUNT: usize = 24;

/// Removes the derived index and its source-table triggers for migration fixtures.
#[cfg(test)]
pub(super) const REMOVE_LEXICAL_SCHEMA_FOR_TEST: &str = r#"
DROP TRIGGER IF EXISTS memory_message_blocks_after_insert;
DROP TRIGGER IF EXISTS memory_message_blocks_after_update;
DROP TRIGGER IF EXISTS memory_message_blocks_after_delete;
DROP TRIGGER IF EXISTS memory_messages_after_state_update;
DROP TRIGGER IF EXISTS memory_messages_after_delete;
DROP TRIGGER IF EXISTS memory_extractions_after_insert;
DROP TRIGGER IF EXISTS memory_extractions_after_update;
DROP TRIGGER IF EXISTS memory_extractions_after_delete;
DROP TABLE IF EXISTS memory_lexical_index;
"#;

/// Native source category retained by the lexical index without crossing IPC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MemorySourceKind {
    /// One complete final user or assistant message answer, excluding reasoning.
    Message,
    /// One complete ready extracted attachment document.
    Attachment,
}

impl MemorySourceKind {
    /// Returns the stable derived-index representation.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::Attachment => "attachment",
        }
    }

    /// Parses a trusted source kind constrained by migration-owned triggers.
    pub(super) fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "message" => Ok(Self::Message),
            "attachment" => Ok(Self::Attachment),
            _ => Err(StorageError::internal()),
        }
    }
}

/// Native filters for lexical retrieval before any memory tool or WebView exposure exists.
#[derive(Clone, Debug)]
pub(crate) struct MemoryLexicalFilters {
    /// Optional source-category restriction.
    pub(crate) source_kind: Option<MemorySourceKind>,
    /// Optional conversation scope resolved through native durable associations.
    pub(crate) conversation_id: Option<String>,
    /// Optional inclusive source creation-time floor.
    pub(crate) created_after_ms: Option<i64>,
    /// Optional inclusive source creation-time ceiling.
    pub(crate) created_before_ms: Option<i64>,
    /// Requested result count, capped by native policy.
    pub(crate) limit: usize,
}

impl Default for MemoryLexicalFilters {
    fn default() -> Self {
        Self {
            source_kind: None,
            conversation_id: None,
            created_after_ms: None,
            created_before_ms: None,
            limit: DEFAULT_LEXICAL_RESULTS,
        }
    }
}

/// One native lexical match with opaque provenance and no content outside its bounded excerpt.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MemoryLexicalHit {
    /// Indexed source category.
    pub(crate) source_kind: MemorySourceKind,
    /// Opaque native message or attachment identity.
    pub(crate) source_id: String,
    /// Bounded FTS5 excerpt around matching terms.
    pub(crate) snippet: String,
    /// SQLite BM25 score, where a lower value is a stronger match.
    pub(crate) rank: f64,
    /// Durable creation time used by date filters and deterministic tie-breaking.
    pub(crate) created_at_ms: i64,
}

impl ConversationStore {
    /// Searches final message answers and associated ready documents through native SQLite FTS5.
    pub(crate) fn search_memory_lexically(
        &self,
        query: &str,
        filters: MemoryLexicalFilters,
    ) -> Result<Vec<MemoryLexicalHit>, StorageError> {
        let query = normalized_fts_query(query)?;
        if query.is_empty() || filters.limit == 0 {
            return Ok(Vec::new());
        }
        validate_filters(&filters)?;
        let source_kind = filters.source_kind.map(MemorySourceKind::as_str);
        let limit = filters.limit.min(MAX_LEXICAL_RESULTS) as i64;
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT source_kind, source_id,
                    snippet(memory_lexical_index, 4, '', '', '…', ?1),
                    bm25(memory_lexical_index), created_at_ms
             FROM memory_lexical_index
             WHERE memory_lexical_index MATCH ?2
               AND profile_id = ?3
               AND (?4 IS NULL OR source_kind = ?4)
               AND (?5 IS NULL OR created_at_ms >= ?5)
               AND (?6 IS NULL OR created_at_ms <= ?6)
               AND (
                   (source_kind = 'message' AND EXISTS (
                       SELECT 1 FROM messages
                       JOIN conversations ON conversations.id = messages.conversation_id
                       WHERE messages.id = memory_lexical_index.source_id
                         AND conversations.profile_id = ?3
                         AND conversations.deleted_at_ms IS NULL
                         AND (?7 IS NULL OR conversations.id = ?7)
                   ))
                   OR
                   (source_kind = 'attachment' AND (
                       EXISTS (
                           SELECT 1 FROM conversation_attachments
                           JOIN conversations
                             ON conversations.id = conversation_attachments.conversation_id
                           WHERE conversation_attachments.attachment_id = memory_lexical_index.source_id
                             AND conversations.profile_id = ?3
                             AND conversations.deleted_at_ms IS NULL
                             AND (?7 IS NULL OR conversations.id = ?7)
                       )
                       OR EXISTS (
                           SELECT 1 FROM message_attachments
                           JOIN messages ON messages.id = message_attachments.message_id
                           JOIN conversations ON conversations.id = messages.conversation_id
                           WHERE message_attachments.attachment_id = memory_lexical_index.source_id
                             AND conversations.profile_id = ?3
                             AND conversations.deleted_at_ms IS NULL
                             AND (?7 IS NULL OR conversations.id = ?7)
                       )
                   ))
               )
             ORDER BY bm25(memory_lexical_index), created_at_ms DESC, source_kind, source_id
             LIMIT ?8",
        )?;
        let rows = statement.query_map(
            params![
                SNIPPET_TOKEN_COUNT as i64,
                query,
                DEFAULT_PROFILE_ID,
                source_kind,
                filters.created_after_ms,
                filters.created_before_ms,
                filters.conversation_id,
                limit,
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, f64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )?;
        rows.map(|row| {
            let (source_kind, source_id, snippet, rank, created_at_ms) = row?;
            Ok(MemoryLexicalHit {
                source_kind: MemorySourceKind::from_database(&source_kind)?,
                source_id,
                snippet,
                rank,
                created_at_ms,
            })
        })
        .collect()
    }
}

/// Converts user text into an AND query containing only quoted tokenizer-safe terms.
fn normalized_fts_query(value: &str) -> Result<String, StorageError> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() > MAX_LEXICAL_QUERY_CHARACTERS {
        return Err(StorageError::invalid(format!(
            "Memory search is limited to {MAX_LEXICAL_QUERY_CHARACTERS} characters."
        )));
    }
    let mut terms = Vec::new();
    let mut term = String::new();
    for character in normalized.chars() {
        if character.is_alphanumeric() || character == '_' {
            term.push(character);
        } else if !term.is_empty() {
            terms.push(std::mem::take(&mut term));
        }
    }
    if !term.is_empty() {
        terms.push(term);
    }
    Ok(terms
        .into_iter()
        .map(|term| format!("\"{term}\""))
        .collect::<Vec<_>>()
        .join(" AND "))
}

/// Rejects contradictory or malformed native filter values before querying SQLite.
fn validate_filters(filters: &MemoryLexicalFilters) -> Result<(), StorageError> {
    if filters
        .conversation_id
        .as_deref()
        .is_some_and(|conversation_id| conversation_id.trim().is_empty())
    {
        return Err(StorageError::invalid(
            "A memory conversation filter cannot be empty.",
        ));
    }
    if filters
        .created_after_ms
        .zip(filters.created_before_ms)
        .is_some_and(|(after, before)| after > before)
    {
        return Err(StorageError::invalid(
            "The memory date filter has an invalid range.",
        ));
    }
    Ok(())
}
