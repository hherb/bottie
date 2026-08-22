//! Provider-neutral bounded state machine for repeated native tool execution.

#![allow(
    dead_code,
    reason = "provider adapter mapping into the native loop is intentionally deferred"
)]

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use serde::Serialize;
use serde_json::Value;

use crate::{
    storage::{ConversationStore, SemanticEmbedder},
    tool_dispatch::{MemoryToolExecution, dispatch_memory_tool},
};

/// Maximum native tool calls accepted across one provider generation.
pub(crate) const MAX_TOOL_LOOP_CALLS: usize = 8;
/// Maximum provider-to-tool recursion rounds accepted across one generation.
pub(crate) const MAX_TOOL_LOOP_ROUNDS: usize = 4;
/// Maximum serialized provider-facing tool output retained across one generation.
pub(crate) const MAX_TOOL_LOOP_OUTPUT_BYTES: usize = 256 * 1_024;
/// Overall wall-clock budget for one provider-neutral tool loop.
pub(crate) const TOOL_LOOP_TIMEOUT: Duration = Duration::from_secs(30);

/// One provider-neutral raw call awaiting validation and native execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NativeToolCall {
    /// Opaque provider-scoped identity returned unchanged with the result.
    pub(crate) call_id: String,
    /// Stable native tool definition name requested by the provider.
    pub(crate) tool_name: String,
    /// Raw JSON arguments validated by the selected native tool contract.
    pub(crate) arguments: Value,
}

/// One correlated provider-neutral result produced by the native dispatcher.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeToolResult {
    /// Opaque provider-scoped identity copied from the matching call.
    pub(crate) call_id: String,
    /// Common bounded success or redacted error envelope.
    pub(crate) execution: MemoryToolExecution,
}

/// Stable reason a provider-neutral tool loop stopped before normal completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolLoopErrorCode {
    /// The provider requested more total calls than Bottie permits.
    CallLimitExceeded,
    /// The provider requested more tool recursion rounds than Bottie permits.
    RecursionLimitExceeded,
    /// Correlated serialized results exceeded the aggregate output budget.
    AggregateOutputExceeded,
    /// The overall native tool-loop deadline elapsed.
    TimedOut,
    /// The shared generation cancellation signal was raised.
    Cancelled,
    /// Work was submitted after the loop reached a terminal state.
    InvalidState,
}

/// Redacted terminal tool-loop failure safe for future provider mapping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolLoopError {
    /// Stable machine-readable loop policy category.
    pub(crate) code: ToolLoopErrorCode,
    /// Fixed explanation that never reflects provider-controlled content.
    pub(crate) message: &'static str,
}

/// Cloneable cancellation signal shared with future provider-loop orchestration.
#[derive(Clone, Debug, Default)]
pub(crate) struct ToolLoopCancellation {
    cancelled: Arc<AtomicBool>,
}

impl ToolLoopCancellation {
    /// Raises the cancellation signal for the active loop and later checks.
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Returns whether cancellation has been requested.
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// Terminal or active lifecycle of one provider-neutral tool loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolLoopStatus {
    Active,
    Completed,
    Failed,
    Cancelled,
}

/// Bounded provider-neutral state accumulated across tool recursion rounds.
#[derive(Debug)]
pub(crate) struct ToolLoopState {
    started_at: Instant,
    status: ToolLoopStatus,
    round_count: usize,
    call_count: usize,
    aggregate_output_bytes: usize,
}

impl ToolLoopState {
    /// Starts one active loop at the native acceptance time.
    pub(crate) fn new(started_at: Instant) -> Self {
        Self {
            started_at,
            status: ToolLoopStatus::Active,
            round_count: 0,
            call_count: 0,
            aggregate_output_bytes: 0,
        }
    }

    /// Executes one provider-neutral round through Bottie's strict memory-tool dispatcher.
    pub(crate) fn execute_memory_round(
        &mut self,
        store: &ConversationStore,
        embedder: &mut impl SemanticEmbedder,
        calls: Vec<NativeToolCall>,
        cancellation: &ToolLoopCancellation,
    ) -> Result<Vec<NativeToolResult>, ToolLoopError> {
        self.execute_round_with(calls, cancellation, Instant::now, |call| {
            dispatch_memory_tool(store, embedder, &call.tool_name, &call.arguments)
        })
    }

    /// Executes one recursion round while enforcing all cross-call loop policy.
    pub(crate) fn execute_round_with(
        &mut self,
        calls: Vec<NativeToolCall>,
        cancellation: &ToolLoopCancellation,
        mut now: impl FnMut() -> Instant,
        mut execute: impl FnMut(&NativeToolCall) -> MemoryToolExecution,
    ) -> Result<Vec<NativeToolResult>, ToolLoopError> {
        self.require_active()?;
        self.require_live(cancellation, now())?;
        if self.round_count >= MAX_TOOL_LOOP_ROUNDS {
            return Err(self.fail(ToolLoopErrorCode::RecursionLimitExceeded));
        }
        if calls.is_empty() {
            return Err(self.fail(ToolLoopErrorCode::InvalidState));
        }
        if self
            .call_count
            .checked_add(calls.len())
            .is_none_or(|count| count > MAX_TOOL_LOOP_CALLS)
        {
            return Err(self.fail(ToolLoopErrorCode::CallLimitExceeded));
        }

        self.round_count += 1;
        let mut results = Vec::with_capacity(calls.len());
        for call in calls {
            self.require_live(cancellation, now())?;
            self.call_count += 1;
            let result = NativeToolResult {
                call_id: call.call_id.clone(),
                execution: execute(&call),
            };
            self.require_live(cancellation, now())?;
            let output_bytes = serde_json::to_vec(&result)
                .map_err(|_| self.fail(ToolLoopErrorCode::AggregateOutputExceeded))?
                .len();
            let Some(aggregate_output_bytes) =
                self.aggregate_output_bytes.checked_add(output_bytes)
            else {
                return Err(self.fail(ToolLoopErrorCode::AggregateOutputExceeded));
            };
            if aggregate_output_bytes > MAX_TOOL_LOOP_OUTPUT_BYTES {
                return Err(self.fail(ToolLoopErrorCode::AggregateOutputExceeded));
            }
            self.aggregate_output_bytes = aggregate_output_bytes;
            results.push(result);
        }
        Ok(results)
    }

    /// Marks an active loop complete after a provider returns no further tool calls.
    pub(crate) fn complete(
        &mut self,
        cancellation: &ToolLoopCancellation,
    ) -> Result<(), ToolLoopError> {
        self.complete_with(cancellation, Instant::now())
    }

    /// Completes at an explicit instant so deadline policy remains independently testable.
    pub(crate) fn complete_with(
        &mut self,
        cancellation: &ToolLoopCancellation,
        now: Instant,
    ) -> Result<(), ToolLoopError> {
        self.require_active()?;
        self.require_live(cancellation, now)?;
        self.status = ToolLoopStatus::Completed;
        Ok(())
    }

    /// Returns the number of accepted provider-to-tool rounds.
    pub(crate) fn round_count(&self) -> usize {
        self.round_count
    }

    /// Returns the number of tool calls whose execution began.
    pub(crate) fn call_count(&self) -> usize {
        self.call_count
    }

    /// Returns the correlated serialized output retained so far.
    pub(crate) fn aggregate_output_bytes(&self) -> usize {
        self.aggregate_output_bytes
    }

    /// Rejects work after normal or exceptional termination without reopening the loop.
    fn require_active(&self) -> Result<(), ToolLoopError> {
        if self.status == ToolLoopStatus::Active {
            Ok(())
        } else {
            Err(loop_error(ToolLoopErrorCode::InvalidState))
        }
    }

    /// Applies shared cancellation and overall-deadline checks at every execution boundary.
    fn require_live(
        &mut self,
        cancellation: &ToolLoopCancellation,
        now: Instant,
    ) -> Result<(), ToolLoopError> {
        if cancellation.is_cancelled() {
            self.status = ToolLoopStatus::Cancelled;
            return Err(loop_error(ToolLoopErrorCode::Cancelled));
        }
        if now
            .checked_duration_since(self.started_at)
            .is_some_and(|elapsed| elapsed >= TOOL_LOOP_TIMEOUT)
        {
            return Err(self.fail(ToolLoopErrorCode::TimedOut));
        }
        Ok(())
    }

    /// Transitions the active loop into one terminal policy failure.
    fn fail(&mut self, code: ToolLoopErrorCode) -> ToolLoopError {
        self.status = ToolLoopStatus::Failed;
        loop_error(code)
    }
}

/// Builds one fixed redacted loop-policy failure.
fn loop_error(code: ToolLoopErrorCode) -> ToolLoopError {
    let message = match code {
        ToolLoopErrorCode::CallLimitExceeded => "The native tool loop exceeded its call limit.",
        ToolLoopErrorCode::RecursionLimitExceeded => {
            "The native tool loop exceeded its recursion limit."
        }
        ToolLoopErrorCode::AggregateOutputExceeded => {
            "The native tool loop exceeded its aggregate output limit."
        }
        ToolLoopErrorCode::TimedOut => "The native tool loop exceeded its time limit.",
        ToolLoopErrorCode::Cancelled => "The native tool loop was cancelled.",
        ToolLoopErrorCode::InvalidState => "The native tool loop is not accepting more work.",
    };
    ToolLoopError { code, message }
}
