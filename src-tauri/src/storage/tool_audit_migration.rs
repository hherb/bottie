//! Schema migration for durable provider-neutral tool execution audit metadata.

/// Adds immutable execution classification, outcome, and duration to existing tool records.
pub(super) const MIGRATION_21: &str = r#"
ALTER TABLE tool_invocations ADD COLUMN execution_policy TEXT NOT NULL DEFAULT 'legacy'
    CHECK (execution_policy IN ('legacy', 'safe', 'approval_required', 'unregistered'));
ALTER TABLE tool_results ADD COLUMN outcome_code TEXT NOT NULL DEFAULT 'legacy_error'
    CHECK (outcome_code IN (
        'success', 'unsupported_tool', 'invalid_arguments', 'approval_required',
        'unavailable', 'execution_failed', 'output_too_large', 'legacy_error'
    ));
UPDATE tool_results SET outcome_code = 'success' WHERE is_error = 0;
ALTER TABLE tool_results ADD COLUMN duration_ms INTEGER CHECK (duration_ms >= 0);
"#;
