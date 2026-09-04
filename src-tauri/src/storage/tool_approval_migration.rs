//! Schema migration for append-only approval decisions on native tool calls.

/// Adds one immutable approve or deny decision before an approval-required tool result.
pub(super) const MIGRATION_22: &str = r#"
CREATE TABLE tool_approvals (
    id TEXT PRIMARY KEY,
    tool_invocation_id TEXT NOT NULL UNIQUE REFERENCES tool_invocations(id) ON DELETE CASCADE,
    decision TEXT NOT NULL CHECK (decision IN ('approved', 'denied')),
    created_at_ms INTEGER NOT NULL
) STRICT;
"#;
