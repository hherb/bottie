//! SQLite schema for the resumable native sqlite-vec semantic index.

/// Adds versioned semantic-index metadata, durable chunk mappings, and cosine vectors.
pub(super) const MIGRATION_18: &str = r#"
DROP TRIGGER IF EXISTS memory_embedding_records_after_delete;
DROP TRIGGER IF EXISTS memory_chunks_semantic_after_insert;
DROP TRIGGER IF EXISTS memory_chunks_semantic_after_delete;
DROP TABLE IF EXISTS memory_embedding_records;
DROP TABLE IF EXISTS memory_vector_index;
DROP TABLE IF EXISTS memory_semantic_metadata;

CREATE TABLE memory_semantic_metadata (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    embedding_version INTEGER NOT NULL CHECK (embedding_version > 0),
    model_code TEXT NOT NULL CHECK (length(trim(model_code)) > 0),
    model_variant TEXT NOT NULL CHECK (length(trim(model_variant)) > 0),
    runtime_version TEXT NOT NULL CHECK (length(trim(runtime_version)) > 0),
    dimensions INTEGER NOT NULL CHECK (dimensions > 0),
    chunking_version INTEGER NOT NULL CHECK (chunking_version > 0),
    index_generation INTEGER NOT NULL CHECK (index_generation > 0),
    input_contract TEXT NOT NULL CHECK (length(trim(input_contract)) > 0),
    state TEXT NOT NULL CHECK (state IN ('pending', 'loading_model', 'indexing', 'ready', 'failed')),
    completed_chunks INTEGER NOT NULL CHECK (completed_chunks >= 0),
    total_chunks INTEGER NOT NULL CHECK (total_chunks >= completed_chunks),
    error_code TEXT,
    updated_at_ms INTEGER NOT NULL
) STRICT;
INSERT INTO memory_semantic_metadata
    (singleton, embedding_version, model_code, model_variant, runtime_version, dimensions,
     chunking_version, index_generation, input_contract, state,
     completed_chunks, total_chunks, error_code, updated_at_ms)
SELECT 1, 1, 'onnx-community/embeddinggemma-300m-ONNX', 'EmbeddingGemma300MQ4',
       'fastembed-6', 768, 1, 1, 'embeddinggemma-document-v1',
       CASE WHEN COUNT(*) = 0 THEN 'ready' ELSE 'pending' END,
       0, COUNT(*), NULL, CAST(strftime('%s', 'now') AS INTEGER) * 1000
FROM memory_chunks;

CREATE TABLE memory_embedding_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chunk_id TEXT NOT NULL UNIQUE REFERENCES memory_chunks(id) ON DELETE CASCADE,
    content_sha256 TEXT NOT NULL
        CHECK (length(content_sha256) = 64 AND content_sha256 NOT GLOB '*[^0-9a-f]*'),
    embedding_version INTEGER NOT NULL CHECK (embedding_version > 0),
    model_variant TEXT NOT NULL CHECK (length(trim(model_variant)) > 0),
    dimensions INTEGER NOT NULL CHECK (dimensions > 0),
    chunking_version INTEGER NOT NULL CHECK (chunking_version > 0),
    index_generation INTEGER NOT NULL CHECK (index_generation > 0),
    embedded_at_ms INTEGER NOT NULL
) STRICT;

CREATE VIRTUAL TABLE memory_vector_index USING vec0(
    embedding float[768] distance_metric=cosine
);

CREATE TRIGGER memory_embedding_records_after_delete
AFTER DELETE ON memory_embedding_records
BEGIN
    DELETE FROM memory_vector_index WHERE rowid = OLD.id;
END;

CREATE TRIGGER memory_chunks_semantic_after_insert
AFTER INSERT ON memory_chunks
BEGIN
    UPDATE memory_semantic_metadata
    SET state = 'pending',
        total_chunks = (SELECT COUNT(*) FROM memory_chunks),
        error_code = NULL,
        updated_at_ms = CAST(strftime('%s', 'now') AS INTEGER) * 1000
    WHERE singleton = 1;
END;

CREATE TRIGGER memory_chunks_semantic_after_delete
AFTER DELETE ON memory_chunks
BEGIN
    UPDATE memory_semantic_metadata
    SET completed_chunks = (SELECT COUNT(*) FROM memory_embedding_records),
        total_chunks = (SELECT COUNT(*) FROM memory_chunks),
        state = CASE
            WHEN (SELECT COUNT(*) FROM memory_embedding_records)
               = (SELECT COUNT(*) FROM memory_chunks) THEN 'ready'
            ELSE 'pending'
        END,
        error_code = NULL,
        updated_at_ms = CAST(strftime('%s', 'now') AS INTEGER) * 1000
    WHERE singleton = 1;
END;
"#;
