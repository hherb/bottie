//! Resumable native semantic indexing over deterministic memory chunks.

use std::sync::OnceLock;

use rusqlite::{TransactionBehavior, params};
use serde::Serialize;
use sqlite_vec::sqlite3_vec_init;

use super::{
    ConversationStore, DEFAULT_PROFILE_ID, StorageError, memory_chunks::CHUNKING_VERSION, now_ms,
};

/// Durable embedding-input contract version.
pub(super) const EMBEDDING_VERSION: i64 = 1;
/// Built-in FastEmbed model repository.
pub(super) const EMBEDDING_MODEL_CODE: &str = "onnx-community/embeddinggemma-300m-ONNX";
/// Built-in quantized FastEmbed model variant.
pub(super) const EMBEDDING_MODEL_VARIANT: &str = "EmbeddingGemma300MQ4";
/// Runtime family/version retained with the durable index contract.
pub(super) const EMBEDDING_RUNTIME_VERSION: &str = "fastembed-6";
/// Fixed output dimensions for EmbeddingGemma.
pub(super) const EMBEDDING_DIMENSIONS: usize = 768;
/// Initial vector-index generation.
pub(super) const INDEX_GENERATION: i64 = 1;
/// Stable document prefix contract recommended for EmbeddingGemma corpus entries.
pub(super) const DOCUMENT_INPUT_PREFIX: &str = "title: none | text: ";
/// Default bounded number of chunks embedded before one durable commit.
pub(crate) const DEFAULT_SEMANTIC_BATCH_SIZE: usize = 8;
const MAX_SEMANTIC_BATCH_SIZE: usize = 32;
const INPUT_CONTRACT: &str = "embeddinggemma-document-v1";

/// Removes the derived semantic schema and triggers for migration fixtures.
#[cfg(test)]
pub(super) const REMOVE_MEMORY_SEMANTIC_SCHEMA_FOR_TEST: &str = r#"
DROP TRIGGER IF EXISTS memory_embedding_records_after_delete;
DROP TRIGGER IF EXISTS memory_chunks_semantic_after_insert;
DROP TRIGGER IF EXISTS memory_chunks_semantic_after_delete;
DROP TABLE IF EXISTS memory_embedding_records;
DROP TABLE IF EXISTS memory_vector_index;
DROP TABLE IF EXISTS memory_semantic_metadata;
"#;

static SQLITE_VEC_REGISTRATION: OnceLock<i32> = OnceLock::new();

/// Registers statically linked sqlite-vec for every future SQLite connection.
pub(super) fn register_sqlite_vec() -> Result<(), StorageError> {
    let result = *SQLITE_VEC_REGISTRATION.get_or_init(|| unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite3_vec_init as *const (),
        )))
    });
    if result == rusqlite::ffi::SQLITE_OK {
        Ok(())
    } else {
        Err(StorageError::internal())
    }
}

/// Rejects a store whose durable semantic contract does not match this application build.
pub(super) fn validate_semantic_contract(
    connection: &rusqlite::Connection,
) -> Result<(), StorageError> {
    let metadata: (i64, String, String, String, i64, i64, i64, String) = connection.query_row(
        "SELECT embedding_version, model_code, model_variant, runtime_version,
                dimensions, chunking_version, index_generation, input_contract
         FROM memory_semantic_metadata WHERE singleton = 1",
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        },
    )?;
    let expected = (
        EMBEDDING_VERSION,
        EMBEDDING_MODEL_CODE.to_owned(),
        EMBEDDING_MODEL_VARIANT.to_owned(),
        EMBEDDING_RUNTIME_VERSION.to_owned(),
        EMBEDDING_DIMENSIONS as i64,
        CHUNKING_VERSION,
        INDEX_GENERATION,
        INPUT_CONTRACT.to_owned(),
    );
    if metadata == expected {
        Ok(())
    } else {
        Err(StorageError::internal())
    }
}

/// Minimal embedding boundary used by the production FastEmbed adapter and deterministic tests.
pub(crate) trait SemanticEmbedder {
    /// Produces one embedding for every supplied versioned document input.
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String>;
}

/// Durable native-only lifecycle of the active semantic-index generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SemanticIndexState {
    /// Eligible chunks are waiting for model acquisition or embedding.
    Pending,
    /// The application-owned model cache is being checked or populated.
    LoadingModel,
    /// Bounded chunk batches are being embedded and committed.
    Indexing,
    /// Every current eligible chunk has an active embedding.
    Ready,
    /// Model acquisition or embedding failed and can be retried after another wake.
    Failed,
}

impl SemanticIndexState {
    /// Parses migration-constrained durable state.
    fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "pending" => Ok(Self::Pending),
            "loading_model" => Ok(Self::LoadingModel),
            "indexing" => Ok(Self::Indexing),
            "ready" => Ok(Self::Ready),
            "failed" => Ok(Self::Failed),
            _ => Err(StorageError::internal()),
        }
    }
}

/// Path-free progress for diagnostics and native scheduling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticIndexProgress {
    /// Current durable lifecycle state.
    pub(crate) state: SemanticIndexState,
    /// Chunks durably represented in the active vector generation.
    pub(crate) completed_chunks: usize,
    /// Current eligible deterministic chunks.
    pub(crate) total_chunks: usize,
    /// Stable path-free failure category, when indexing last failed.
    pub(crate) error_code: Option<String>,
}

/// One immutable chunk snapshot selected before model work begins.
struct PendingChunk {
    id: String,
    content_sha256: String,
    chunking_version: i64,
    text: String,
}

impl ConversationStore {
    /// Records that the application-owned model cache is being checked or populated.
    pub(crate) fn mark_semantic_model_loading(&self) -> Result<(), StorageError> {
        self.update_semantic_state("loading_model", None)
    }

    /// Records a path-free model/runtime failure without deleting resumable vectors.
    pub(crate) fn mark_semantic_failed(&self, error_code: &str) -> Result<(), StorageError> {
        self.update_semantic_state("failed", Some(error_code))
    }

    /// Returns durable path-free semantic progress for the native worker.
    pub(crate) fn semantic_index_progress(&self) -> Result<SemanticIndexProgress, StorageError> {
        let connection = self.open()?;
        connection
            .query_row(
                "SELECT state, completed_chunks, total_chunks, error_code
                 FROM memory_semantic_metadata WHERE singleton = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .map_err(StorageError::from)
            .and_then(|(state, completed, total, error_code)| {
                Ok(SemanticIndexProgress {
                    state: SemanticIndexState::from_database(&state)?,
                    completed_chunks: usize::try_from(completed)
                        .map_err(|_| StorageError::internal())?,
                    total_chunks: usize::try_from(total).map_err(|_| StorageError::internal())?,
                    error_code,
                })
            })
    }

    /// Embeds and atomically commits one bounded batch, returning no item when fully current.
    pub(crate) fn process_next_semantic_batch(
        &self,
        embedder: &mut impl SemanticEmbedder,
        batch_size: usize,
    ) -> Result<Option<SemanticIndexProgress>, StorageError> {
        if !(1..=MAX_SEMANTIC_BATCH_SIZE).contains(&batch_size) {
            return Err(StorageError::internal());
        }
        let pending = self.pending_semantic_chunks(batch_size)?;
        if pending.is_empty() {
            self.update_semantic_progress("ready", None)?;
            return Ok(None);
        }
        self.update_semantic_state("indexing", None)?;
        let inputs = pending
            .iter()
            .map(|chunk| format!("{DOCUMENT_INPUT_PREFIX}{}", chunk.text))
            .collect::<Vec<_>>();
        let embeddings = match embedder.embed(&inputs) {
            Ok(embeddings) if embeddings.len() == pending.len() => embeddings,
            Ok(_) => {
                self.mark_semantic_failed("embedding_count")?;
                return Err(StorageError::internal());
            }
            Err(_) => {
                self.mark_semantic_failed("embedding_runtime")?;
                return Err(StorageError::internal());
            }
        };
        if embeddings
            .iter()
            .any(|embedding| embedding.len() != EMBEDDING_DIMENSIONS)
        {
            self.mark_semantic_failed("embedding_dimensions")?;
            return Err(StorageError::internal());
        }
        if embeddings
            .iter()
            .flat_map(|embedding| embedding.iter())
            .any(|value| !value.is_finite())
        {
            self.mark_semantic_failed("embedding_values")?;
            return Err(StorageError::internal());
        }
        self.commit_semantic_batch(&pending, &embeddings)?;
        Ok(Some(self.semantic_index_progress()?))
    }

    /// Loads path-free progress through the focused storage test boundary.
    #[cfg(test)]
    pub(super) fn semantic_index_status_for_test(
        &self,
    ) -> Result<SemanticIndexProgress, StorageError> {
        self.semantic_index_progress()
    }

    /// Selects stable pending source snapshots without holding a write lock during inference.
    fn pending_semantic_chunks(&self, limit: usize) -> Result<Vec<PendingChunk>, StorageError> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT memory_chunks.id, memory_chunks.content_sha256,
                    memory_chunks.chunking_version, memory_chunks.text_content
             FROM memory_chunks
             WHERE memory_chunks.profile_id = ?1
               AND NOT EXISTS (
                   SELECT 1 FROM memory_embedding_records
                   WHERE memory_embedding_records.chunk_id = memory_chunks.id
                     AND memory_embedding_records.content_sha256 = memory_chunks.content_sha256
                     AND memory_embedding_records.embedding_version = ?2
                     AND memory_embedding_records.model_variant = ?3
                     AND memory_embedding_records.dimensions = ?4
                     AND memory_embedding_records.chunking_version = memory_chunks.chunking_version
                     AND memory_embedding_records.index_generation = ?5
               )
             ORDER BY memory_chunks.source_created_at_ms,
                      memory_chunks.source_kind, memory_chunks.source_id, memory_chunks.ordinal
             LIMIT ?6",
        )?;
        statement
            .query_map(
                params![
                    DEFAULT_PROFILE_ID,
                    EMBEDDING_VERSION,
                    EMBEDDING_MODEL_VARIANT,
                    EMBEDDING_DIMENSIONS as i64,
                    INDEX_GENERATION,
                    limit as i64,
                ],
                |row| {
                    Ok(PendingChunk {
                        id: row.get(0)?,
                        content_sha256: row.get(1)?,
                        chunking_version: row.get(2)?,
                        text: row.get(3)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Commits only chunks whose identity/content still match the pre-inference snapshot.
    fn commit_semantic_batch(
        &self,
        pending: &[PendingChunk],
        embeddings: &[Vec<f32>],
    ) -> Result<(), StorageError> {
        let mut connection = self.open()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for (chunk, embedding) in pending.iter().zip(embeddings) {
            let inserted = transaction.execute(
                "INSERT INTO memory_embedding_records
                 (chunk_id, content_sha256, embedding_version, model_variant, dimensions,
                  chunking_version, index_generation, embedded_at_ms)
                 SELECT id, content_sha256, ?3, ?4, ?5, chunking_version, ?6, ?7
                 FROM memory_chunks
                 WHERE id = ?1 AND content_sha256 = ?2 AND chunking_version = ?8
                 ON CONFLICT(chunk_id) DO NOTHING",
                params![
                    chunk.id,
                    chunk.content_sha256,
                    EMBEDDING_VERSION,
                    EMBEDDING_MODEL_VARIANT,
                    EMBEDDING_DIMENSIONS as i64,
                    INDEX_GENERATION,
                    now_ms()?,
                    chunk.chunking_version,
                ],
            )?;
            if inserted == 1 {
                transaction.execute(
                    "INSERT INTO memory_vector_index(rowid, embedding) VALUES (?1, ?2)",
                    params![transaction.last_insert_rowid(), embedding_bytes(embedding)],
                )?;
            }
        }
        update_progress_in_transaction(&transaction, "indexing", None)?;
        transaction.commit()?;
        Ok(())
    }

    /// Updates only the durable phase/error while retaining committed progress counts.
    fn update_semantic_state(
        &self,
        state: &str,
        error_code: Option<&str>,
    ) -> Result<(), StorageError> {
        let connection = self.open()?;
        connection.execute(
            "UPDATE memory_semantic_metadata
             SET state = ?1, error_code = ?2, updated_at_ms = ?3
             WHERE singleton = 1",
            params![state, error_code, now_ms()?],
        )?;
        Ok(())
    }

    /// Recounts current source and vector rows while updating the durable phase.
    fn update_semantic_progress(
        &self,
        state: &str,
        error_code: Option<&str>,
    ) -> Result<(), StorageError> {
        let mut connection = self.open()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        update_progress_in_transaction(&transaction, state, error_code)?;
        transaction.commit()?;
        Ok(())
    }
}

/// Recounts active records inside the caller's atomic batch transaction.
fn update_progress_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    state: &str,
    error_code: Option<&str>,
) -> Result<(), StorageError> {
    transaction.execute(
        "UPDATE memory_semantic_metadata
         SET state = ?1,
             completed_chunks = (SELECT COUNT(*) FROM memory_embedding_records),
             total_chunks = (SELECT COUNT(*) FROM memory_chunks),
             error_code = ?2,
             updated_at_ms = ?3
         WHERE singleton = 1",
        params![state, error_code, now_ms()?],
    )?;
    Ok(())
}

/// Encodes native-endian float32 values in sqlite-vec's compact BLOB representation.
fn embedding_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect()
}

/// Returns active contract fields for assertions that schema SQL and Rust stay aligned.
#[cfg(test)]
fn active_contract() -> (&'static str, i64) {
    (INPUT_CONTRACT, CHUNKING_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_input_and_contract_are_stable() {
        assert_eq!(
            format!("{DOCUMENT_INPUT_PREFIX}memory text"),
            "title: none | text: memory text"
        );
        assert_eq!(active_contract(), ("embeddinggemma-document-v1", 1));
        assert_eq!(embedding_bytes(&[1.0, -2.0]).len(), 8);
    }
}
