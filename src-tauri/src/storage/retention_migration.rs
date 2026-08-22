//! SQLite schema for one built-in-profile Trash retention policy.

/// Adds an opt-in bounded retention period without changing existing Trash by default.
pub(super) const MIGRATION_20: &str = r#"
CREATE TABLE conversation_retention_policies (
    profile_id TEXT PRIMARY KEY REFERENCES profiles(id) ON DELETE CASCADE,
    period TEXT NOT NULL CHECK (period IN ('thirty_days', 'ninety_days', 'one_year')),
    updated_at_ms INTEGER NOT NULL
) STRICT;
"#;
