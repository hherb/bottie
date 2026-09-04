//! Append-only decisions for native approval-required tool calls.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use super::{
    ConversationStore, StorageError, now_ms,
    tools::{
        MAX_PROVIDER_CALL_ID_CHARACTERS, ToolAuditOutcome, ToolAuditPolicy, normalized_identity,
        require_active_run,
    },
};

/// One explicit native decision appended before an approval-required call can execute.
#[derive(Clone, Debug)]
pub(crate) struct NewToolApproval {
    /// Native provider run that owns the matching call.
    pub(crate) provider_run_id: String,
    /// Provider-scoped call identity retained only for native correlation.
    pub(crate) provider_call_id: String,
    /// Exact user decision applied to the unchanged call.
    pub(crate) decision: ToolApprovalDecision,
}

/// Immutable native approval decision for one exact approval-required call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolApprovalDecision {
    /// The user authorized the unchanged call exactly once.
    Approved,
    /// The user refused the unchanged call.
    Denied,
}

impl ToolApprovalDecision {
    /// Returns the stable SQLite representation.
    fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Denied => "denied",
        }
    }

    /// Reconstructs a schema-constrained SQLite representation.
    pub(super) fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "approved" => Ok(Self::Approved),
            "denied" => Ok(Self::Denied),
            _ => Err(StorageError::internal()),
        }
    }
}

/// One reconstructed approval decision without its native or provider call identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredToolApproval {
    /// Exact approve or deny decision applied to the call.
    pub(crate) decision: ToolApprovalDecision,
    /// Native wall-clock time when the decision became durable.
    pub(crate) decided_at_ms: i64,
}

impl ConversationStore {
    /// Appends one decision before an approval-required invocation can execute or finish.
    pub(crate) fn checkpoint_tool_approval(
        &self,
        approval: NewToolApproval,
    ) -> Result<(), StorageError> {
        let call_id = normalized_identity(
            &approval.provider_call_id,
            MAX_PROVIDER_CALL_ID_CHARACTERS,
            "A tool approval requires a bounded provider call identity.",
        )?;
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        require_active_run(&transaction, &approval.provider_run_id)?;
        let (invocation_id, policy, has_approval, has_result) = transaction
            .query_row(
                "SELECT tool_invocations.id, tool_invocations.execution_policy,
                        EXISTS (
                            SELECT 1 FROM tool_approvals
                            WHERE tool_approvals.tool_invocation_id = tool_invocations.id
                        ),
                        EXISTS (
                            SELECT 1 FROM tool_results
                            WHERE tool_results.tool_invocation_id = tool_invocations.id
                        )
                 FROM tool_invocations
                 WHERE provider_run_id = ?1 AND provider_call_id = ?2",
                params![approval.provider_run_id, call_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, bool>(2)?,
                        row.get::<_, bool>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| StorageError::not_found("That tool invocation is not retained."))?;
        if policy != ToolAuditPolicy::ApprovalRequired.as_str() {
            return Err(StorageError::invalid(
                "Only approval-required tool calls accept a decision.",
            ));
        }
        if has_approval || has_result {
            return Err(StorageError::invalid(
                "That tool invocation can no longer accept an approval decision.",
            ));
        }
        transaction.execute(
            "INSERT INTO tool_approvals
             (id, tool_invocation_id, decision, created_at_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                uuid::Uuid::new_v4().to_string(),
                invocation_id,
                approval.decision.as_str(),
                now_ms()?,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }
}

/// Prevents an approval-required success or denial outcome from contradicting its decision record.
pub(super) fn validate_result_approval(
    policy: &str,
    approval: Option<&str>,
    outcome: ToolAuditOutcome,
) -> Result<(), StorageError> {
    if policy != ToolAuditPolicy::ApprovalRequired.as_str() {
        return Ok(());
    }
    match approval {
        Some("approved") => Ok(()),
        Some("denied") if outcome == ToolAuditOutcome::ApprovalRequired => Ok(()),
        None if outcome != ToolAuditOutcome::Success => Ok(()),
        _ => Err(StorageError::invalid(
            "Tool-result outcome does not match its approval decision.",
        )),
    }
}
