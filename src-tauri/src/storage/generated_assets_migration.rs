//! Durable assistant-owned generated-image asset schema.

/// Adds path-free generated-image metadata owned by assistant messages.
pub(super) const MIGRATION_23: &str = r#"
CREATE TABLE generated_assets (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    status TEXT NOT NULL CHECK (status IN ('pending', 'completed', 'cancelled', 'failed')),
    sha256 TEXT
        CHECK (sha256 IS NULL OR (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*')),
    media_type TEXT CHECK (media_type IS NULL OR media_type = 'image/png'),
    width INTEGER CHECK (width IS NULL OR width > 0),
    height INTEGER CHECK (height IS NULL OR height > 0),
    byte_size INTEGER CHECK (byte_size IS NULL OR byte_size > 0),
    provider_id TEXT NOT NULL CHECK (length(trim(provider_id)) > 0),
    model_id TEXT NOT NULL CHECK (length(trim(model_id)) > 0),
    execution TEXT NOT NULL CHECK (execution IN ('cloud', 'local')),
    seed INTEGER,
    error_code TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    UNIQUE (message_id, ordinal),
    CHECK (
        (status = 'completed' AND sha256 IS NOT NULL AND media_type = 'image/png'
            AND width IS NOT NULL AND height IS NOT NULL AND byte_size IS NOT NULL
            AND error_code IS NULL)
        OR (status = 'failed' AND sha256 IS NULL AND media_type IS NULL
            AND width IS NULL AND height IS NULL AND byte_size IS NULL
            AND error_code IS NOT NULL)
        OR (status IN ('pending', 'cancelled') AND sha256 IS NULL AND media_type IS NULL
            AND width IS NULL AND height IS NULL AND byte_size IS NULL
            AND error_code IS NULL)
    )
) STRICT;
CREATE INDEX generated_assets_message_idx ON generated_assets(message_id, ordinal);
CREATE INDEX generated_assets_content_idx
    ON generated_assets(sha256) WHERE sha256 IS NOT NULL;
"#;
