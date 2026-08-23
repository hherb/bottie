//! Pure mapping from native tool envelopes into durable audit outcomes.

use crate::{
    storage::ToolAuditOutcome,
    tool_dispatch::{MemoryToolExecution, MemoryToolExecutionErrorCode},
};

/// Maps one bounded dispatcher envelope into its durable path-free audit category.
pub(crate) fn audit_outcome(execution: &MemoryToolExecution) -> ToolAuditOutcome {
    let MemoryToolExecution::Error { error } = execution else {
        return ToolAuditOutcome::Success;
    };
    match error.code {
        MemoryToolExecutionErrorCode::UnsupportedTool => ToolAuditOutcome::UnsupportedTool,
        MemoryToolExecutionErrorCode::InvalidArguments => ToolAuditOutcome::InvalidArguments,
        MemoryToolExecutionErrorCode::ApprovalRequired => ToolAuditOutcome::ApprovalRequired,
        MemoryToolExecutionErrorCode::Unavailable => ToolAuditOutcome::Unavailable,
        MemoryToolExecutionErrorCode::ExecutionFailed => ToolAuditOutcome::ExecutionFailed,
        MemoryToolExecutionErrorCode::OutputTooLarge => ToolAuditOutcome::OutputTooLarge,
    }
}
