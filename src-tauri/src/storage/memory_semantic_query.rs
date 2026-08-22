//! Native-only bounded semantic KNN retrieval over Bottie's current sqlite-vec generation.

#![allow(dead_code)]

use rusqlite::params;

use super::{
    ConversationStore, DEFAULT_PROFILE_ID, StorageError,
    memory_chunks::CHUNKING_VERSION,
    memory_filters::{
        MAX_MEMORY_RESULTS, MemorySearchFilters, MemorySourceKind, normalized_memory_query,
    },
    memory_semantic::{
        EMBEDDING_DIMENSIONS, EMBEDDING_MODEL_VARIANT, EMBEDDING_VERSION, INDEX_GENERATION,
        SemanticEmbedder,
    },
};

/// Stable retrieval-query prefix recommended by the EmbeddingGemma contract.
pub(super) const QUERY_INPUT_PREFIX: &str = "task: search result | query: ";

/// Shared memory filter contract used by focused semantic callers and tests.
pub(crate) type MemorySemanticFilters = MemorySearchFilters;

/// One native semantic chunk match with bounded text and opaque source provenance.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MemorySemanticHit {
    /// Indexed source category.
    pub(crate) source_kind: MemorySourceKind,
    /// Opaque native message or attachment identity.
    pub(crate) source_id: String,
    /// Zero-based order within the complete source.
    pub(crate) ordinal: usize,
    /// Inclusive Unicode-scalar offset in the complete source.
    pub(crate) start_character: usize,
    /// Exclusive Unicode-scalar offset in the complete source.
    pub(crate) end_character: usize,
    /// Exact bounded deterministic chunk text.
    pub(crate) excerpt: String,
    /// sqlite-vec cosine distance, where a lower value is a stronger match.
    pub(crate) distance: f64,
    /// Durable source creation time used by date filters.
    pub(crate) created_at_ms: i64,
}

impl ConversationStore {
    /// Embeds one bounded query and searches the current vector generation under native policy.
    pub(crate) fn search_memory_semantically(
        &self,
        query: &str,
        embedder: &mut impl SemanticEmbedder,
        filters: MemorySearchFilters,
    ) -> Result<Vec<MemorySemanticHit>, StorageError> {
        let query = normalized_semantic_query(query)?;
        filters.validate()?;
        if query.is_empty() || filters.limit == 0 {
            return Ok(Vec::new());
        }
        let embeddings = embedder
            .embed(&[format!("{QUERY_INPUT_PREFIX}{query}")])
            .map_err(|_| StorageError::internal())?;
        let [embedding] = embeddings.as_slice() else {
            return Err(StorageError::internal());
        };
        validate_query_embedding(embedding)?;
        self.query_semantic_index(embedding, filters)
    }

    /// Executes exact sqlite-vec KNN after dynamic lifecycle and association prefiltering.
    fn query_semantic_index(
        &self,
        embedding: &[f32],
        filters: MemorySearchFilters,
    ) -> Result<Vec<MemorySemanticHit>, StorageError> {
        let connection = self.open()?;
        let source_kind = filters.source_kind.map(MemorySourceKind::as_str);
        let limit = filters.limit.min(MAX_MEMORY_RESULTS) as i64;
        let mut statement = connection.prepare(
            "SELECT memory_chunks.source_kind, memory_chunks.source_id,
                    memory_chunks.ordinal, memory_chunks.start_character,
                    memory_chunks.end_character, memory_chunks.text_content,
                    memory_vector_index.distance, memory_chunks.source_created_at_ms
             FROM memory_vector_index
             JOIN memory_embedding_records
               ON memory_embedding_records.id = memory_vector_index.rowid
             JOIN memory_chunks ON memory_chunks.id = memory_embedding_records.chunk_id
             WHERE memory_vector_index.embedding MATCH ?1
               AND memory_vector_index.k = ?2
               AND memory_vector_index.rowid IN (
                   SELECT eligible_records.id
                   FROM memory_embedding_records AS eligible_records
                   JOIN memory_chunks AS eligible_chunks
                     ON eligible_chunks.id = eligible_records.chunk_id
                   WHERE eligible_chunks.profile_id = ?3
                     AND eligible_records.embedding_version = ?4
                     AND eligible_records.model_variant = ?5
                     AND eligible_records.dimensions = ?6
                     AND eligible_records.chunking_version = ?7
                     AND eligible_records.index_generation = ?8
                     AND (?9 IS NULL OR eligible_chunks.source_kind = ?9)
                     AND (?10 IS NULL OR eligible_chunks.source_created_at_ms >= ?10)
                     AND (?11 IS NULL OR eligible_chunks.source_created_at_ms <= ?11)
                     AND (
                         (eligible_chunks.source_kind = 'message' AND EXISTS (
                             SELECT 1 FROM messages
                             JOIN conversations ON conversations.id = messages.conversation_id
                             WHERE messages.id = eligible_chunks.source_id
                               AND conversations.profile_id = ?3
                               AND conversations.deleted_at_ms IS NULL
                               AND NOT EXISTS (
                                   SELECT 1 FROM conversation_memory_preferences
                                   WHERE conversation_id = conversations.id AND excluded = 1
                               )
                               AND (?12 IS NULL OR conversations.id = ?12)
                         ))
                         OR
                         (eligible_chunks.source_kind = 'attachment' AND (
                             EXISTS (
                                 SELECT 1 FROM conversation_attachments
                                 JOIN conversations
                                   ON conversations.id = conversation_attachments.conversation_id
                                 WHERE conversation_attachments.attachment_id = eligible_chunks.source_id
                                   AND conversations.profile_id = ?3
                                   AND conversations.deleted_at_ms IS NULL
                                   AND NOT EXISTS (
                                       SELECT 1 FROM conversation_memory_preferences
                                       WHERE conversation_id = conversations.id AND excluded = 1
                                   )
                                   AND (?12 IS NULL OR conversations.id = ?12)
                             )
                             OR EXISTS (
                                 SELECT 1 FROM message_attachments
                                 JOIN messages ON messages.id = message_attachments.message_id
                                 JOIN conversations ON conversations.id = messages.conversation_id
                                 WHERE message_attachments.attachment_id = eligible_chunks.source_id
                                   AND conversations.profile_id = ?3
                                   AND conversations.deleted_at_ms IS NULL
                                   AND NOT EXISTS (
                                       SELECT 1 FROM conversation_memory_preferences
                                       WHERE conversation_id = conversations.id AND excluded = 1
                                   )
                                   AND (?12 IS NULL OR conversations.id = ?12)
                             )
                         ))
                     )
               )
             ORDER BY memory_vector_index.distance",
        )?;
        let query = embedding_bytes(embedding);
        let rows = statement.query_map(
            params![
                query,
                limit,
                DEFAULT_PROFILE_ID,
                EMBEDDING_VERSION,
                EMBEDDING_MODEL_VARIANT,
                EMBEDDING_DIMENSIONS as i64,
                CHUNKING_VERSION,
                INDEX_GENERATION,
                source_kind,
                filters.created_after_ms,
                filters.created_before_ms,
                filters.conversation_id,
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, f64>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            },
        )?;
        rows.map(|row| {
            let (source_kind, source_id, ordinal, start, end, excerpt, distance, created_at_ms) =
                row?;
            Ok(MemorySemanticHit {
                source_kind: MemorySourceKind::from_database(&source_kind)?,
                source_id,
                ordinal: usize::try_from(ordinal).map_err(|_| StorageError::internal())?,
                start_character: usize::try_from(start).map_err(|_| StorageError::internal())?,
                end_character: usize::try_from(end).map_err(|_| StorageError::internal())?,
                excerpt,
                distance,
                created_at_ms,
            })
        })
        .collect()
    }
}

/// Normalizes whitespace while preserving literal query text for native embedding.
fn normalized_semantic_query(value: &str) -> Result<String, StorageError> {
    normalized_memory_query(value)
}

/// Rejects vectors that cannot match the compiled sqlite-vec contract.
fn validate_query_embedding(embedding: &[f32]) -> Result<(), StorageError> {
    if embedding.len() != EMBEDDING_DIMENSIONS || embedding.iter().any(|value| !value.is_finite()) {
        return Err(StorageError::internal());
    }
    Ok(())
}

/// Encodes native-endian float32 values in sqlite-vec's compact BLOB representation.
fn embedding_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect()
}
