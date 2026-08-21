//! Ordered SQLite migrations for durable conversation storage.

/// Initial local-profile, conversation, branch, message, and content-block schema.
pub(super) const MIGRATION_1: &str = r#"
CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at_ms INTEGER NOT NULL
) STRICT;
CREATE TABLE profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
) STRICT;
CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    profile_id TEXT NOT NULL REFERENCES profiles(id),
    title TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    archived_at_ms INTEGER,
    deleted_at_ms INTEGER
) STRICT;
CREATE TABLE branches (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
) STRICT;
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    branch_id TEXT NOT NULL REFERENCES branches(id) ON DELETE CASCADE,
    parent_message_id TEXT REFERENCES messages(id),
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    state TEXT NOT NULL CHECK (state IN ('partial', 'final', 'cancelled', 'failed')),
    provider_id TEXT,
    model_id TEXT,
    created_at_ms INTEGER NOT NULL
) STRICT;
CREATE TABLE message_blocks (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    block_type TEXT NOT NULL CHECK (block_type IN ('text', 'reasoning')),
    text_content TEXT NOT NULL,
    UNIQUE (message_id, ordinal)
) STRICT;
CREATE INDEX conversations_profile_updated_idx
    ON conversations(profile_id, updated_at_ms DESC)
    WHERE deleted_at_ms IS NULL;
CREATE INDEX messages_branch_created_idx ON messages(branch_id, created_at_ms, id);
"#;

/// Adds a branch-local append order independent of wall-clock resolution.
pub(super) const MIGRATION_2: &str = r#"
ALTER TABLE messages ADD COLUMN sequence INTEGER NOT NULL DEFAULT 0;
WITH ordered AS (
    SELECT id, ROW_NUMBER() OVER (PARTITION BY branch_id ORDER BY created_at_ms, id) - 1 AS value
    FROM messages
)
UPDATE messages SET sequence = (SELECT value FROM ordered WHERE ordered.id = messages.id);
DROP INDEX messages_branch_created_idx;
CREATE UNIQUE INDEX messages_branch_sequence_idx ON messages(branch_id, sequence);
"#;

/// Adds provider-run provenance, terminal state, and append-only usage snapshots.
pub(super) const MIGRATION_3: &str = r#"
CREATE TABLE provider_runs (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    branch_id TEXT NOT NULL REFERENCES branches(id) ON DELETE CASCADE,
    request_message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL CHECK (length(trim(provider_id)) > 0),
    model_id TEXT NOT NULL CHECK (length(trim(model_id)) > 0),
    state TEXT NOT NULL CHECK (state IN ('running', 'completed', 'cancelled', 'failed')),
    reasoning_effort TEXT NOT NULL CHECK (reasoning_effort IN ('off', 'low')),
    temperature REAL,
    max_output_tokens INTEGER CHECK (max_output_tokens > 0),
    error_code TEXT,
    started_at_ms INTEGER NOT NULL,
    completed_at_ms INTEGER,
    CHECK (
        (state = 'running' AND completed_at_ms IS NULL)
        OR (state != 'running' AND completed_at_ms IS NOT NULL)
    ),
    CHECK (
        (state = 'failed' AND error_code IS NOT NULL)
        OR (state != 'failed' AND error_code IS NULL)
    )
) STRICT;
CREATE TABLE usage_records (
    id TEXT PRIMARY KEY,
    provider_run_id TEXT NOT NULL REFERENCES provider_runs(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    input_tokens INTEGER CHECK (input_tokens >= 0),
    output_tokens INTEGER CHECK (output_tokens >= 0),
    cost_usd REAL CHECK (cost_usd >= 0),
    recorded_at_ms INTEGER NOT NULL,
    CHECK (input_tokens IS NOT NULL OR output_tokens IS NOT NULL OR cost_usd IS NOT NULL),
    UNIQUE (provider_run_id, ordinal)
) STRICT;
ALTER TABLE messages ADD COLUMN provider_run_id TEXT REFERENCES provider_runs(id);
CREATE INDEX provider_runs_conversation_started_idx
    ON provider_runs(conversation_id, started_at_ms, id);
CREATE UNIQUE INDEX messages_provider_run_idx
    ON messages(provider_run_id) WHERE provider_run_id IS NOT NULL;
"#;

/// Adds profile-owned navigation state for exact conversation restoration.
pub(super) const MIGRATION_4: &str = r#"
ALTER TABLE profiles ADD COLUMN last_open_conversation_id TEXT REFERENCES conversations(id);
UPDATE profiles
SET last_open_conversation_id = (
    SELECT id FROM conversations
    WHERE conversations.profile_id = profiles.id
      AND archived_at_ms IS NULL
      AND deleted_at_ms IS NULL
    ORDER BY updated_at_ms DESC, id DESC
    LIMIT 1
);
"#;

/// Adds the selected branch used to reconstruct one visible conversation lineage.
pub(super) const MIGRATION_5: &str = r#"
ALTER TABLE conversations ADD COLUMN current_branch_id TEXT REFERENCES branches(id);
UPDATE conversations
SET current_branch_id = (
    SELECT id FROM branches
    WHERE branches.conversation_id = conversations.id
    ORDER BY created_at_ms, id
    LIMIT 1
);
"#;

/// Adds one mutable local quality rating per immutable assistant response.
pub(super) const MIGRATION_6: &str = r#"
CREATE TABLE response_ratings (
    message_id TEXT PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    rating TEXT NOT NULL CHECK (rating IN ('good', 'poor')),
    updated_at_ms INTEGER NOT NULL
) STRICT;
"#;

/// Adds immutable provider tool calls and their optional append-only results.
pub(super) const MIGRATION_7: &str = r#"
CREATE TABLE tool_invocations (
    id TEXT PRIMARY KEY,
    provider_run_id TEXT NOT NULL REFERENCES provider_runs(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    provider_call_id TEXT NOT NULL CHECK (length(trim(provider_call_id)) > 0),
    tool_name TEXT NOT NULL CHECK (length(trim(tool_name)) > 0),
    arguments_json TEXT NOT NULL CHECK (json_valid(arguments_json)),
    created_at_ms INTEGER NOT NULL,
    UNIQUE (provider_run_id, ordinal),
    UNIQUE (provider_run_id, provider_call_id)
) STRICT;
CREATE TABLE tool_results (
    id TEXT PRIMARY KEY,
    tool_invocation_id TEXT NOT NULL UNIQUE REFERENCES tool_invocations(id) ON DELETE CASCADE,
    output_json TEXT NOT NULL CHECK (json_valid(output_json)),
    is_error INTEGER NOT NULL CHECK (is_error IN (0, 1)),
    created_at_ms INTEGER NOT NULL
) STRICT;
CREATE INDEX tool_invocations_run_created_idx
    ON tool_invocations(provider_run_id, ordinal);
"#;

/// Adds application-owned content-addressed attachment metadata.
pub(super) const MIGRATION_8: &str = r#"
CREATE TABLE attachments (
    id TEXT PRIMARY KEY,
    sha256 TEXT NOT NULL UNIQUE
        CHECK (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) > 0),
    mime_type TEXT NOT NULL CHECK (length(trim(mime_type)) > 0),
    byte_size INTEGER NOT NULL CHECK (byte_size > 0),
    created_at_ms INTEGER NOT NULL
) STRICT;
CREATE INDEX attachments_created_idx ON attachments(created_at_ms, id);
"#;

/// Associates retained attachment metadata with ordered durable user messages.
pub(super) const MIGRATION_9: &str = r#"
CREATE TABLE message_attachments (
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    attachment_id TEXT NOT NULL REFERENCES attachments(id) ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    attached_at_ms INTEGER NOT NULL,
    PRIMARY KEY (message_id, attachment_id),
    UNIQUE (message_id, ordinal)
) STRICT;
CREATE INDEX message_attachments_attachment_idx
    ON message_attachments(attachment_id, message_id);
"#;

/// Adds durable, path-free text extraction state and native-only extracted content.
pub(super) const MIGRATION_10: &str = r#"
CREATE TABLE attachment_extractions (
    attachment_id TEXT PRIMARY KEY REFERENCES attachments(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK (state IN ('pending', 'ready', 'unsupported', 'failed')),
    format TEXT CHECK (format IN ('plain_text', 'markdown')),
    text_content TEXT,
    character_count INTEGER CHECK (character_count >= 0),
    error_code TEXT,
    updated_at_ms INTEGER NOT NULL,
    CHECK (
        (state = 'pending' AND format IS NULL AND text_content IS NULL
            AND character_count IS NULL AND error_code IS NULL)
        OR (state = 'ready' AND format IS NOT NULL AND text_content IS NOT NULL
            AND character_count IS NOT NULL AND error_code IS NULL)
        OR (state = 'unsupported' AND format IS NULL AND text_content IS NULL
            AND character_count IS NULL AND error_code IS NULL)
        OR (state = 'failed' AND format IS NULL AND text_content IS NULL
            AND character_count IS NULL AND error_code IS NOT NULL)
    )
) STRICT;
INSERT INTO attachment_extractions (attachment_id, state, updated_at_ms)
SELECT id, 'pending', created_at_ms FROM attachments;
"#;

/// Extends native extraction state with page-aware PDF text while preserving completed text rows.
pub(super) const MIGRATION_11: &str = r#"
CREATE TABLE attachment_extractions_v11 (
    attachment_id TEXT PRIMARY KEY REFERENCES attachments(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK (state IN ('pending', 'ready', 'unsupported', 'failed')),
    format TEXT CHECK (format IN ('plain_text', 'markdown', 'pdf')),
    text_content TEXT,
    character_count INTEGER CHECK (character_count >= 0),
    page_count INTEGER CHECK (page_count > 0),
    error_code TEXT,
    updated_at_ms INTEGER NOT NULL,
    CHECK (
        (state = 'pending' AND format IS NULL AND text_content IS NULL
            AND character_count IS NULL AND page_count IS NULL AND error_code IS NULL)
        OR (state = 'ready' AND format IN ('plain_text', 'markdown') AND text_content IS NOT NULL
            AND character_count IS NOT NULL AND page_count IS NULL AND error_code IS NULL)
        OR (state = 'ready' AND format = 'pdf' AND text_content IS NOT NULL
            AND character_count IS NOT NULL AND page_count IS NOT NULL AND error_code IS NULL)
        OR (state = 'unsupported' AND format IS NULL AND text_content IS NULL
            AND character_count IS NULL AND page_count IS NULL AND error_code IS NULL)
        OR (state = 'failed' AND format IS NULL AND text_content IS NULL
            AND character_count IS NULL AND page_count IS NULL AND error_code IS NOT NULL)
    )
) STRICT;
INSERT INTO attachment_extractions_v11
    (attachment_id, state, format, text_content, character_count, page_count, error_code, updated_at_ms)
SELECT attachment_extractions.attachment_id,
       CASE WHEN attachments.mime_type = 'application/pdf' THEN 'pending' ELSE attachment_extractions.state END,
       CASE WHEN attachments.mime_type = 'application/pdf' THEN NULL ELSE attachment_extractions.format END,
       CASE WHEN attachments.mime_type = 'application/pdf' THEN NULL ELSE attachment_extractions.text_content END,
       CASE WHEN attachments.mime_type = 'application/pdf' THEN NULL ELSE attachment_extractions.character_count END,
       NULL,
       CASE WHEN attachments.mime_type = 'application/pdf' THEN NULL ELSE attachment_extractions.error_code END,
       attachment_extractions.updated_at_ms
FROM attachment_extractions
JOIN attachments ON attachments.id = attachment_extractions.attachment_id;
DROP TABLE attachment_extractions;
ALTER TABLE attachment_extractions_v11 RENAME TO attachment_extractions;
"#;

/// Extends native extraction state with bounded DOCX text while preserving completed rows.
pub(super) const MIGRATION_12: &str = r#"
CREATE TABLE attachment_extractions_v12 (
    attachment_id TEXT PRIMARY KEY REFERENCES attachments(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK (state IN ('pending', 'ready', 'unsupported', 'failed')),
    format TEXT CHECK (format IN ('plain_text', 'markdown', 'pdf', 'docx')),
    text_content TEXT,
    character_count INTEGER CHECK (character_count >= 0),
    page_count INTEGER CHECK (page_count > 0),
    error_code TEXT,
    updated_at_ms INTEGER NOT NULL,
    CHECK (
        (state = 'pending' AND format IS NULL AND text_content IS NULL
            AND character_count IS NULL AND page_count IS NULL AND error_code IS NULL)
        OR (state = 'ready' AND format IN ('plain_text', 'markdown', 'docx')
            AND text_content IS NOT NULL AND character_count IS NOT NULL
            AND page_count IS NULL AND error_code IS NULL)
        OR (state = 'ready' AND format = 'pdf' AND text_content IS NOT NULL
            AND character_count IS NOT NULL AND page_count IS NOT NULL AND error_code IS NULL)
        OR (state = 'unsupported' AND format IS NULL AND text_content IS NULL
            AND character_count IS NULL AND page_count IS NULL AND error_code IS NULL)
        OR (state = 'failed' AND format IS NULL AND text_content IS NULL
            AND character_count IS NULL AND page_count IS NULL AND error_code IS NOT NULL)
    )
) STRICT;
INSERT INTO attachment_extractions_v12
    (attachment_id, state, format, text_content, character_count, page_count, error_code, updated_at_ms)
SELECT attachment_extractions.attachment_id,
       CASE WHEN attachments.mime_type IN (
           'application/zip',
           'application/vnd.openxmlformats-officedocument.wordprocessingml.document'
       ) THEN 'pending' ELSE attachment_extractions.state END,
       CASE WHEN attachments.mime_type IN (
           'application/zip',
           'application/vnd.openxmlformats-officedocument.wordprocessingml.document'
       ) THEN NULL ELSE attachment_extractions.format END,
       CASE WHEN attachments.mime_type IN (
           'application/zip',
           'application/vnd.openxmlformats-officedocument.wordprocessingml.document'
       ) THEN NULL ELSE attachment_extractions.text_content END,
       CASE WHEN attachments.mime_type IN (
           'application/zip',
           'application/vnd.openxmlformats-officedocument.wordprocessingml.document'
       ) THEN NULL ELSE attachment_extractions.character_count END,
       CASE WHEN attachments.mime_type IN (
           'application/zip',
           'application/vnd.openxmlformats-officedocument.wordprocessingml.document'
       ) THEN NULL ELSE attachment_extractions.page_count END,
       CASE WHEN attachments.mime_type IN (
           'application/zip',
           'application/vnd.openxmlformats-officedocument.wordprocessingml.document'
       ) THEN NULL ELSE attachment_extractions.error_code END,
       attachment_extractions.updated_at_ms
FROM attachment_extractions
JOIN attachments ON attachments.id = attachment_extractions.attachment_id;
DROP TABLE attachment_extractions;
ALTER TABLE attachment_extractions_v12 RENAME TO attachment_extractions;
"#;

/// Adds durable native-only JPEG and PNG normalization state and derivative identities.
pub(super) const MIGRATION_13: &str = r#"
CREATE TABLE attachment_image_normalizations (
    attachment_id TEXT PRIMARY KEY REFERENCES attachments(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK (state IN ('pending', 'ready', 'unsupported', 'failed')),
    format TEXT CHECK (format IN ('jpeg', 'png')),
    width INTEGER CHECK (width > 0),
    height INTEGER CHECK (height > 0),
    byte_size INTEGER CHECK (byte_size > 0),
    normalized_sha256 TEXT
        CHECK (length(normalized_sha256) = 64 AND normalized_sha256 NOT GLOB '*[^0-9a-f]*'),
    error_code TEXT,
    updated_at_ms INTEGER NOT NULL,
    CHECK (
        (state = 'pending' AND format IS NULL AND width IS NULL AND height IS NULL
            AND byte_size IS NULL AND normalized_sha256 IS NULL AND error_code IS NULL)
        OR (state = 'ready' AND format IS NOT NULL AND width IS NOT NULL AND height IS NOT NULL
            AND byte_size IS NOT NULL AND normalized_sha256 IS NOT NULL AND error_code IS NULL)
        OR (state = 'unsupported' AND format IS NULL AND width IS NULL AND height IS NULL
            AND byte_size IS NULL AND normalized_sha256 IS NULL AND error_code IS NULL)
        OR (state = 'failed' AND format IS NULL AND width IS NULL AND height IS NULL
            AND byte_size IS NULL AND normalized_sha256 IS NULL AND error_code IS NOT NULL)
    )
) STRICT;
INSERT INTO attachment_image_normalizations (attachment_id, state, updated_at_ms)
SELECT id,
       CASE WHEN mime_type IN ('image/jpeg', 'image/png') THEN 'pending' ELSE 'unsupported' END,
       created_at_ms
FROM attachments;
"#;
