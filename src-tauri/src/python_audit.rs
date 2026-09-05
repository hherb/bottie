//! Durable provider-neutral orchestration for approved Python execution.
//!
//! Explicitly tool-capable local routes call this seam. It appends the exact invocation, records
//! any explicit decision before starting the runner, and then appends one bounded terminal outcome
//! through the existing native tool audit.

use serde::Serialize;

use crate::{
    python_approval::{PythonApprovalController, PythonApprovalResolution},
    python_execution::{
        PythonExecutionError, PythonExecutionErrorCode, PythonExecutionOutcome,
        PythonExecutionResult, PythonRunner, execute_authorized_python, resolve_python_approval,
    },
    storage::{
        ConversationStore, NewToolApproval, NewToolInvocation, NewToolResult, StorageError,
        ToolApprovalDecision, ToolAuditOutcome, ToolAuditPolicy,
    },
    tool_dispatch::{
        MemoryToolExecution, MemoryToolExecutionError, MemoryToolExecutionErrorCode,
        bounded_memory_tool_success,
    },
    tool_loop::{NativeToolCall, ToolLoopCancellation},
};

/// Failure returned after preserving every audit record that could be appended safely.
#[derive(Debug)]
pub(crate) enum AuditedPythonError {
    /// Native storage could not append the next ordered audit record.
    Storage(StorageError),
    /// Approval or helper execution failed behind its fixed path-free boundary.
    Execution(PythonExecutionError),
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum PythonAuditPayload<'a> {
    Executed { result: &'a PythonExecutionResult },
    Denied,
    Cancelled,
    Failed { code: PythonExecutionErrorCode },
}

enum PythonTerminal {
    Outcome(PythonExecutionOutcome),
    Failure(PythonExecutionError),
}

/// Executes one Python call only after its invocation and approval are durably ordered.
pub(crate) async fn execute_audited_python(
    store: &ConversationStore,
    provider_run_id: &str,
    controller: &PythonApprovalController,
    runner: &(impl PythonRunner + ?Sized),
    call: NativeToolCall,
    cancellation: &ToolLoopCancellation,
) -> Result<PythonExecutionOutcome, AuditedPythonError> {
    store
        .checkpoint_tool_invocation(NewToolInvocation {
            provider_run_id: provider_run_id.into(),
            provider_call_id: call.call_id.clone(),
            tool_name: call.tool_name.clone(),
            arguments: call.arguments.clone(),
            audit_policy: ToolAuditPolicy::ApprovalRequired,
        })
        .map_err(AuditedPythonError::Storage)?;
    let (terminal, duration_ms) =
        match resolve_python_approval(controller, call.clone(), cancellation).await {
            Ok(PythonApprovalResolution::Approved(grant)) => {
                checkpoint_approval(
                    store,
                    provider_run_id,
                    &call.call_id,
                    ToolApprovalDecision::Approved,
                )?;
                let started = std::time::Instant::now();
                match execute_authorized_python(runner, call.clone(), grant, cancellation).await {
                    Ok(outcome) => (PythonTerminal::Outcome(outcome), elapsed_ms(started)),
                    Err(error) => (PythonTerminal::Failure(error), elapsed_ms(started)),
                }
            }
            Ok(PythonApprovalResolution::Denied) => {
                checkpoint_approval(
                    store,
                    provider_run_id,
                    &call.call_id,
                    ToolApprovalDecision::Denied,
                )?;
                (PythonTerminal::Outcome(PythonExecutionOutcome::Denied), 0)
            }
            Ok(PythonApprovalResolution::Cancelled) => (
                PythonTerminal::Outcome(PythonExecutionOutcome::Cancelled),
                0,
            ),
            Err(error) => (PythonTerminal::Failure(error), 0),
        };
    checkpoint_terminal(
        store,
        provider_run_id,
        &call.call_id,
        &terminal,
        duration_ms,
    )?;
    match terminal {
        PythonTerminal::Outcome(outcome) => Ok(outcome),
        PythonTerminal::Failure(error) => Err(AuditedPythonError::Execution(error)),
    }
}

/// Executes one mapped-provider Python call and converts its audited terminal state into a bounded reply.
pub(crate) async fn execute_audited_python_for_provider(
    store: &ConversationStore,
    provider_run_id: &str,
    controller: &PythonApprovalController,
    runner: &(impl PythonRunner + ?Sized),
    call: NativeToolCall,
    cancellation: &ToolLoopCancellation,
) -> Result<MemoryToolExecution, StorageError> {
    match execute_audited_python(
        store,
        provider_run_id,
        controller,
        runner,
        call,
        cancellation,
    )
    .await
    {
        Ok(PythonExecutionOutcome::Executed(result)) => {
            let result = serde_json::to_value(PythonAuditPayload::Executed { result: &result })
                .map_err(|_| StorageError::internal())?;
            Ok(bounded_memory_tool_success(result))
        }
        Ok(PythonExecutionOutcome::Denied) => Ok(python_provider_error(
            MemoryToolExecutionErrorCode::ApprovalRequired,
            "The user denied this Python proposal.",
        )),
        Ok(PythonExecutionOutcome::Cancelled) => Ok(python_provider_error(
            MemoryToolExecutionErrorCode::ExecutionFailed,
            "The Python proposal was cancelled.",
        )),
        Err(AuditedPythonError::Storage(error)) => Err(error),
        Err(AuditedPythonError::Execution(error)) => Ok(python_execution_error(&error)),
    }
}

/// Maps one path-free execution failure into the common provider-facing result contract.
fn python_execution_error(error: &PythonExecutionError) -> MemoryToolExecution {
    let code = match error.code {
        PythonExecutionErrorCode::ApprovalFailed => MemoryToolExecutionErrorCode::ApprovalRequired,
        PythonExecutionErrorCode::InvalidRequest => MemoryToolExecutionErrorCode::InvalidArguments,
        PythonExecutionErrorCode::HelperFailed | PythonExecutionErrorCode::InvalidResult => {
            MemoryToolExecutionErrorCode::ExecutionFailed
        }
    };
    python_provider_error(code, error.message)
}

/// Builds one bounded Python provider error without reflecting source, purpose, or native paths.
fn python_provider_error(
    code: MemoryToolExecutionErrorCode,
    message: &'static str,
) -> MemoryToolExecution {
    MemoryToolExecution::Error {
        error: MemoryToolExecutionError { code, message },
    }
}

/// Appends the exact decision before any approved helper launch can occur.
fn checkpoint_approval(
    store: &ConversationStore,
    provider_run_id: &str,
    provider_call_id: &str,
    decision: ToolApprovalDecision,
) -> Result<(), AuditedPythonError> {
    store
        .checkpoint_tool_approval(NewToolApproval {
            provider_run_id: provider_run_id.into(),
            provider_call_id: provider_call_id.into(),
            decision,
        })
        .map_err(AuditedPythonError::Storage)
}

/// Appends one bounded terminal payload and its stable generic audit category.
fn checkpoint_terminal(
    store: &ConversationStore,
    provider_run_id: &str,
    provider_call_id: &str,
    terminal: &PythonTerminal,
    duration_ms: u64,
) -> Result<(), AuditedPythonError> {
    let (payload, audit_outcome) = terminal_payload(terminal);
    let output = serde_json::to_value(payload)
        .map_err(|_| AuditedPythonError::Storage(StorageError::internal()))?;
    store
        .checkpoint_tool_result(NewToolResult {
            provider_run_id: provider_run_id.into(),
            provider_call_id: provider_call_id.into(),
            output,
            is_error: audit_outcome != ToolAuditOutcome::Success,
            audit_outcome,
            duration_ms,
        })
        .map_err(AuditedPythonError::Storage)
}

/// Converts monotonic native work time without overflowing the durable integer contract.
fn elapsed_ms(started: std::time::Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Maps Python-specific outcomes into a bounded payload plus the existing generic audit taxonomy.
fn terminal_payload(terminal: &PythonTerminal) -> (PythonAuditPayload<'_>, ToolAuditOutcome) {
    match terminal {
        PythonTerminal::Outcome(PythonExecutionOutcome::Executed(result)) => (
            PythonAuditPayload::Executed { result },
            ToolAuditOutcome::Success,
        ),
        PythonTerminal::Outcome(PythonExecutionOutcome::Denied) => (
            PythonAuditPayload::Denied,
            ToolAuditOutcome::ApprovalRequired,
        ),
        PythonTerminal::Outcome(PythonExecutionOutcome::Cancelled) => (
            PythonAuditPayload::Cancelled,
            ToolAuditOutcome::ExecutionFailed,
        ),
        PythonTerminal::Failure(error) => (
            PythonAuditPayload::Failed { code: error.code },
            match error.code {
                PythonExecutionErrorCode::ApprovalFailed => ToolAuditOutcome::ApprovalRequired,
                PythonExecutionErrorCode::InvalidRequest => ToolAuditOutcome::InvalidArguments,
                PythonExecutionErrorCode::HelperFailed
                | PythonExecutionErrorCode::InvalidResult => ToolAuditOutcome::ExecutionFailed,
            },
        ),
    }
}
