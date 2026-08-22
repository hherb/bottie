//! SQLite schema for durable per-conversation memory exclusion.

/// Adds one reversible local memory preference without changing retained source content.
pub(super) const MIGRATION_19: &str = r#"
CREATE TABLE conversation_memory_preferences (
    conversation_id TEXT PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
    excluded INTEGER NOT NULL CHECK (excluded IN (0, 1)),
    updated_at_ms INTEGER NOT NULL
) STRICT;
"#;
