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

/// Adds the durable user-prompt link and accepted options needed for exact retry.
pub(super) const MIGRATION_24: &str = r#"
CREATE TABLE generated_image_requests (
    message_id TEXT PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    request_message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    width INTEGER NOT NULL CHECK (width > 0),
    height INTEGER NOT NULL CHECK (height > 0),
    prompt_extend INTEGER NOT NULL CHECK (prompt_extend IN (0, 1))
) STRICT;
CREATE INDEX generated_image_requests_prompt_idx
    ON generated_image_requests(request_message_id);
"#;

/// Adds exact ordered source snapshots for every assistant-owned edited output.
pub(super) const MIGRATION_25: &str = r#"
CREATE TABLE generated_asset_sources (
    generated_asset_id TEXT NOT NULL REFERENCES generated_assets(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 2),
    source_type TEXT NOT NULL CHECK (source_type IN ('attachment', 'generated_asset')),
    attachment_id TEXT REFERENCES attachments(id),
    source_generated_asset_id TEXT REFERENCES generated_assets(id),
    sha256 TEXT NOT NULL
        CHECK (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
    media_type TEXT NOT NULL CHECK (media_type IN ('image/jpeg', 'image/png')),
    width INTEGER NOT NULL CHECK (width > 0),
    height INTEGER NOT NULL CHECK (height > 0),
    byte_size INTEGER NOT NULL CHECK (byte_size > 0),
    PRIMARY KEY (generated_asset_id, ordinal),
    CHECK (
        (source_type = 'attachment' AND attachment_id IS NOT NULL
            AND source_generated_asset_id IS NULL)
        OR (source_type = 'generated_asset' AND attachment_id IS NULL
            AND source_generated_asset_id IS NOT NULL
            AND source_generated_asset_id <> generated_asset_id)
    )
) STRICT;
CREATE UNIQUE INDEX generated_asset_attachment_source_idx
    ON generated_asset_sources(generated_asset_id, attachment_id)
    WHERE source_type = 'attachment';
CREATE UNIQUE INDEX generated_asset_generated_source_idx
    ON generated_asset_sources(generated_asset_id, source_generated_asset_id)
    WHERE source_type = 'generated_asset';
CREATE UNIQUE INDEX generated_asset_source_content_idx
    ON generated_asset_sources(generated_asset_id, sha256);
CREATE INDEX generated_asset_sources_attachment_idx
    ON generated_asset_sources(attachment_id) WHERE attachment_id IS NOT NULL;
CREATE INDEX generated_asset_sources_generated_idx
    ON generated_asset_sources(source_generated_asset_id) WHERE source_generated_asset_id IS NOT NULL;
"#;
