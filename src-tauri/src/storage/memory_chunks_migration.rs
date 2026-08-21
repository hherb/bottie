//! SQLite schema for the versioned native deterministic memory-chunk catalog.

/// Adds chunking metadata, derived source slices, and stale-row cleanup triggers.
pub(super) const MIGRATION_17: &str = r#"
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

CREATE TABLE memory_chunk_metadata (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    chunking_version INTEGER NOT NULL CHECK (chunking_version > 0),
    algorithm TEXT NOT NULL CHECK (length(trim(algorithm)) > 0),
    max_characters INTEGER NOT NULL CHECK (max_characters > 0),
    min_split_characters INTEGER NOT NULL CHECK (
        min_split_characters > 0 AND min_split_characters <= max_characters
    ),
    overlap_characters INTEGER NOT NULL CHECK (
        overlap_characters >= 0 AND overlap_characters < min_split_characters
    )
) STRICT;
INSERT INTO memory_chunk_metadata
    (singleton, chunking_version, algorithm, max_characters, min_split_characters, overlap_characters)
VALUES (1, 1, 'unicode-whitespace-v1', 1200, 900, 200);

CREATE TABLE memory_chunks (
    id TEXT PRIMARY KEY
        CHECK (length(id) = 64 AND id NOT GLOB '*[^0-9a-f]*'),
    source_kind TEXT NOT NULL CHECK (source_kind IN ('message', 'attachment')),
    source_id TEXT NOT NULL CHECK (length(trim(source_id)) > 0),
    profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    chunking_version INTEGER NOT NULL CHECK (chunking_version > 0),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    start_character INTEGER NOT NULL CHECK (start_character >= 0),
    end_character INTEGER NOT NULL CHECK (end_character > start_character),
    text_content TEXT NOT NULL CHECK (
        length(text_content) > 0
        AND length(text_content) = end_character - start_character
    ),
    content_sha256 TEXT NOT NULL
        CHECK (length(content_sha256) = 64 AND content_sha256 NOT GLOB '*[^0-9a-f]*'),
    source_created_at_ms INTEGER NOT NULL,
    UNIQUE (source_kind, source_id, chunking_version, ordinal)
) STRICT;
CREATE INDEX memory_chunks_profile_source_created_idx
    ON memory_chunks(profile_id, source_kind, source_created_at_ms, source_id, ordinal);

CREATE TRIGGER memory_chunks_message_blocks_after_insert
AFTER INSERT ON message_blocks
WHEN NEW.block_type = 'text'
BEGIN
    DELETE FROM memory_chunks WHERE source_kind = 'message' AND source_id = NEW.message_id;
END;

CREATE TRIGGER memory_chunks_message_blocks_after_update
AFTER UPDATE OF message_id, ordinal, block_type, text_content ON message_blocks
BEGIN
    DELETE FROM memory_chunks
    WHERE source_kind = 'message' AND source_id IN (OLD.message_id, NEW.message_id);
END;

CREATE TRIGGER memory_chunks_message_blocks_after_delete
AFTER DELETE ON message_blocks
WHEN OLD.block_type = 'text'
BEGIN
    DELETE FROM memory_chunks WHERE source_kind = 'message' AND source_id = OLD.message_id;
END;

CREATE TRIGGER memory_chunks_messages_after_state_update
AFTER UPDATE OF state ON messages
BEGIN
    DELETE FROM memory_chunks WHERE source_kind = 'message' AND source_id = NEW.id;
END;

CREATE TRIGGER memory_chunks_messages_after_delete
AFTER DELETE ON messages
BEGIN
    DELETE FROM memory_chunks WHERE source_kind = 'message' AND source_id = OLD.id;
END;

CREATE TRIGGER memory_chunks_extractions_after_insert
AFTER INSERT ON attachment_extractions
BEGIN
    DELETE FROM memory_chunks
    WHERE source_kind = 'attachment' AND source_id = NEW.attachment_id;
END;

CREATE TRIGGER memory_chunks_extractions_after_update
AFTER UPDATE OF attachment_id, state, text_content ON attachment_extractions
BEGIN
    DELETE FROM memory_chunks
    WHERE source_kind = 'attachment'
      AND source_id IN (OLD.attachment_id, NEW.attachment_id);
END;

CREATE TRIGGER memory_chunks_extractions_after_delete
AFTER DELETE ON attachment_extractions
BEGIN
    DELETE FROM memory_chunks
    WHERE source_kind = 'attachment' AND source_id = OLD.attachment_id;
END;
"#;
