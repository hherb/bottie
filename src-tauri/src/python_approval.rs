//! Process-local approval lifecycle for one exact bounded Python proposal.

#![allow(
    dead_code,
    reason = "provider mapping into this native orchestration is intentionally deferred"
)]

use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Notify;

use crate::{
    AppState, tool_contract::validate_python_tool_arguments, tool_loop::NativeToolCall,
    tool_loop::ToolLoopCancellation, tool_policy::ApprovedToolCall,
};

/// Explicit user decision accepted for one pending Python proposal.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PythonApprovalDecision {
    /// Permit only the exact retained call to receive one native grant.
    Approve,
    /// Reject the exact retained call without producing a grant.
    Deny,
}

/// Closed WebView request carrying no source, purpose, or provider call identity.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PythonApprovalDecisionRequest {
    /// Process-local opaque token copied from the native review status.
    pub(crate) request_id: String,
    /// One explicit terminal decision.
    pub(crate) decision: PythonApprovalDecision,
}

/// Path-free user-visible lifecycle for one process-local Python proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PythonApprovalPhase {
    /// The exact purpose and source are awaiting one explicit decision.
    Pending,
    /// The user approved the exact proposal, but no execution occurred.
    Approved,
    /// The user denied the exact proposal and no execution occurred.
    Denied,
}

/// Complete review state exposed without provider call identity or native paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PythonApprovalStatus {
    /// Process-local opaque token used only to submit the decision.
    pub(crate) request_id: String,
    /// Current native-owned decision phase.
    pub(crate) phase: PythonApprovalPhase,
    /// Complete bounded source retained from the validated proposal.
    pub(crate) source: String,
    /// Complete bounded user-visible purpose retained from the validated proposal.
    pub(crate) purpose: String,
}

/// Stable approval-lifecycle failure returned without caller-controlled content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PythonApprovalErrorCode {
    /// The proposed tool name or arguments failed the closed Python contract.
    InvalidRequest,
    /// One earlier proposal still owns the bounded process-local slot.
    RequestPending,
    /// The supplied WebView token did not identify the current proposal.
    RequestNotFound,
    /// The current proposal already received its one explicit decision.
    AlreadyDecided,
    /// Future orchestration tried to consume the decision for a changed call.
    CallMismatch,
}

/// Fixed path-free error for the approval command and future native orchestration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PythonApprovalError {
    /// Stable machine-readable failure category.
    pub(crate) code: PythonApprovalErrorCode,
    /// Fixed explanation that never reflects source, purpose, identities, or tokens.
    pub(crate) message: &'static str,
}

/// One terminal decision consumed by future provider-neutral orchestration.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ConsumedPythonApproval {
    /// Exact one-use grant suitable for the existing native tool policy.
    Approved(ApprovedToolCall),
    /// Explicit refusal that cannot authorize the call.
    Denied,
}

/// Provider-neutral terminal outcome of waiting for one exact Python decision.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PythonApprovalResolution {
    /// Resume native orchestration with one grant bound to the unchanged call.
    Approved(ApprovedToolCall),
    /// Stop the proposed call after the user's explicit refusal.
    Denied,
    /// Stop the proposed call after shared generation cancellation.
    Cancelled,
}

/// Thread-safe owner of the single process-local Python approval slot.
#[derive(Default)]
pub(crate) struct PythonApprovalController {
    current: Mutex<Option<PythonApprovalRecord>>,
    decision_changed: Notify,
}

struct PythonApprovalRecord {
    request_id: String,
    call: NativeToolCall,
    source: String,
    purpose: String,
    decision: Option<PythonApprovalDecision>,
}

/// Drop guard that prevents an aborted provider waiter from retaining the process-local slot.
struct PendingPythonApproval<'a> {
    controller: &'a PythonApprovalController,
    call: &'a NativeToolCall,
    active: bool,
}

impl PythonApprovalController {
    /// Publishes one exact proposal and waits without blocking for approval, denial, or cancellation.
    pub(crate) async fn request_and_wait(
        &self,
        call: NativeToolCall,
        cancellation: &ToolLoopCancellation,
    ) -> Result<PythonApprovalResolution, PythonApprovalError> {
        if cancellation.is_cancelled() {
            return Ok(PythonApprovalResolution::Cancelled);
        }
        self.request(call.clone())?;
        let mut pending = PendingPythonApproval {
            controller: self,
            call: &call,
            active: true,
        };
        loop {
            if cancellation.is_cancelled() {
                pending.clear()?;
                return Ok(PythonApprovalResolution::Cancelled);
            }
            if let Some(decision) = self.take_decision(&call)? {
                pending.disarm();
                return Ok(match decision {
                    ConsumedPythonApproval::Approved(grant) => {
                        PythonApprovalResolution::Approved(grant)
                    }
                    ConsumedPythonApproval::Denied => PythonApprovalResolution::Denied,
                });
            }
            tokio::select! {
                biased;
                _ = cancellation.cancelled() => {}
                _ = self.decision_changed.notified() => {}
            }
        }
    }

    /// Retains one validated exact call for explicit review without executing it.
    pub(crate) fn request(
        &self,
        call: NativeToolCall,
    ) -> Result<PythonApprovalStatus, PythonApprovalError> {
        let arguments = validate_python_tool_arguments(&call.tool_name, &call.arguments)
            .map_err(|_| approval_error(PythonApprovalErrorCode::InvalidRequest))?;
        let mut current = lock(&self.current);
        if current.is_some() {
            return Err(approval_error(PythonApprovalErrorCode::RequestPending));
        }
        let record = PythonApprovalRecord {
            request_id: uuid::Uuid::new_v4().to_string(),
            call,
            source: arguments.source,
            purpose: arguments.purpose,
            decision: None,
        };
        let status = record.status();
        *current = Some(record);
        Ok(status)
    }

    /// Returns the current bounded review or resolved decision without provider identity.
    pub(crate) fn current(&self) -> Option<PythonApprovalStatus> {
        lock(&self.current)
            .as_ref()
            .map(PythonApprovalRecord::status)
    }

    /// Records one explicit WebView decision for the exact process-local request token.
    pub(crate) fn decide(
        &self,
        request_id: &str,
        decision: PythonApprovalDecision,
    ) -> Result<PythonApprovalStatus, PythonApprovalError> {
        let mut current = lock(&self.current);
        let record = current
            .as_mut()
            .filter(|record| record.request_id == request_id)
            .ok_or_else(|| approval_error(PythonApprovalErrorCode::RequestNotFound))?;
        if record.decision.is_some() {
            return Err(approval_error(PythonApprovalErrorCode::AlreadyDecided));
        }
        record.decision = Some(decision);
        let status = record.status();
        drop(current);
        self.decision_changed.notify_one();
        Ok(status)
    }

    /// Consumes a ready decision only when every provider-controlled call field still matches.
    pub(crate) fn take_decision(
        &self,
        call: &NativeToolCall,
    ) -> Result<Option<ConsumedPythonApproval>, PythonApprovalError> {
        let mut current = lock(&self.current);
        let Some(record) = current.as_ref() else {
            return Ok(None);
        };
        if !record.matches(call) {
            return Err(approval_error(PythonApprovalErrorCode::CallMismatch));
        }
        let Some(decision) = record.decision else {
            return Ok(None);
        };
        let record = current
            .take()
            .expect("the matched approval record should remain present");
        Ok(Some(match decision {
            PythonApprovalDecision::Approve => {
                ConsumedPythonApproval::Approved(ApprovedToolCall::for_call(&record.call))
            }
            PythonApprovalDecision::Deny => ConsumedPythonApproval::Denied,
        }))
    }

    /// Clears only the exact retained proposal when its owning generation is cancelled.
    fn clear_matching(&self, call: &NativeToolCall) -> Result<(), PythonApprovalError> {
        let mut current = lock(&self.current);
        if current.as_ref().is_some_and(|record| !record.matches(call)) {
            return Err(approval_error(PythonApprovalErrorCode::CallMismatch));
        }
        *current = None;
        Ok(())
    }
}

impl PendingPythonApproval<'_> {
    /// Clears the exact pending record and prevents duplicate cleanup on normal cancellation.
    fn clear(&mut self) -> Result<(), PythonApprovalError> {
        self.controller.clear_matching(self.call)?;
        self.active = false;
        Ok(())
    }

    /// Marks a decision as already consumed by the normal approval path.
    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for PendingPythonApproval<'_> {
    /// Releases this waiter's exact slot if its future is dropped or aborted while pending.
    fn drop(&mut self) {
        if self.active {
            let _ = self.controller.clear_matching(self.call);
        }
    }
}

impl PythonApprovalRecord {
    /// Builds the path-free public status while retaining exact call identity in Rust.
    fn status(&self) -> PythonApprovalStatus {
        let phase = match self.decision {
            None => PythonApprovalPhase::Pending,
            Some(PythonApprovalDecision::Approve) => PythonApprovalPhase::Approved,
            Some(PythonApprovalDecision::Deny) => PythonApprovalPhase::Denied,
        };
        PythonApprovalStatus {
            request_id: self.request_id.clone(),
            phase,
            source: self.source.clone(),
            purpose: self.purpose.clone(),
        }
    }

    /// Matches the complete native call rather than trusting any WebView-visible field.
    fn matches(&self, call: &NativeToolCall) -> bool {
        self.call.call_id == call.call_id
            && self.call.tool_name == call.tool_name
            && self.call.arguments == call.arguments
    }
}

#[tauri::command]
/// Returns the current process-local Python approval review, when one exists.
pub(crate) fn get_python_approval(state: State<'_, AppState>) -> Option<PythonApprovalStatus> {
    state.python_approval.current()
}

#[tauri::command]
/// Records one approve or deny decision for the exact opaque native request token.
pub(crate) fn decide_python_approval(
    request: PythonApprovalDecisionRequest,
    state: State<'_, AppState>,
) -> Result<PythonApprovalStatus, PythonApprovalError> {
    state
        .python_approval
        .decide(&request.request_id, request.decision)
}

/// Recovers a poisoned lock without exposing panic or caller-controlled state.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Returns one fixed safe error for a closed approval lifecycle category.
fn approval_error(code: PythonApprovalErrorCode) -> PythonApprovalError {
    let message = match code {
        PythonApprovalErrorCode::InvalidRequest => "The Python proposal is invalid.",
        PythonApprovalErrorCode::RequestPending => {
            "Another Python proposal still requires resolution."
        }
        PythonApprovalErrorCode::RequestNotFound => {
            "That Python approval request is no longer available."
        }
        PythonApprovalErrorCode::AlreadyDecided => {
            "That Python approval request already has a decision."
        }
        PythonApprovalErrorCode::CallMismatch => "The Python proposal changed after review.",
    };
    PythonApprovalError { code, message }
}
