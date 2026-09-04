//! oMLX-only approval-gated Python tool-round tests.

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use super::*;
use crate::{
    python_approval::{
        PythonApprovalController, PythonApprovalDecision, PythonApprovalPublisher,
        PythonApprovalStatus,
    },
    python_audit::execute_audited_python_for_provider,
    python_execution::{
        PythonExecutionError, PythonExecutionResult, PythonExecutionStatus, PythonRunner,
        PythonRunnerOutcome,
    },
    storage::{StorageError, ToolApprovalDecision},
    tool_contract::{PythonToolArguments, RUN_PYTHON_TOOL_NAME},
    tool_loop::{AsyncToolExecutor, NativeToolCall},
};

#[derive(Clone, Default)]
struct RecordingPublisher {
    updates: Arc<Mutex<Vec<Option<PythonApprovalStatus>>>>,
}

impl PythonApprovalPublisher for RecordingPublisher {
    /// Retains approval publications for deterministic generation-loop decisions.
    fn publish(&self, approval: Option<PythonApprovalStatus>) -> Result<(), ()> {
        self.updates.lock().unwrap().push(approval);
        Ok(())
    }
}

#[derive(Default)]
struct RecordingRunner {
    requests: Mutex<Vec<PythonToolArguments>>,
}

impl PythonRunner for RecordingRunner {
    /// Returns one bounded result after retaining only the validated helper request.
    fn execute<'a>(
        &'a self,
        arguments: PythonToolArguments,
        _cancellation: &'a ToolLoopCancellation,
    ) -> Pin<Box<dyn Future<Output = Result<PythonRunnerOutcome, PythonExecutionError>> + Send + 'a>>
    {
        Box::pin(async move {
            self.requests.lock().unwrap().push(arguments);
            Ok(PythonRunnerOutcome::Completed(PythonExecutionResult {
                status: PythonExecutionStatus::Ok,
                stdout: "42\n".into(),
                stderr: String::new(),
                duration_ms: 7,
            }))
        })
    }
}

struct AuditedPythonExecutor {
    store: ConversationStore,
    run_id: String,
    controller: Arc<PythonApprovalController>,
    runner: Arc<RecordingRunner>,
    cancellation: ToolLoopCancellation,
}

impl AsyncToolExecutor for AuditedPythonExecutor {
    type Error = StorageError;

    /// Exercises the same audited async seam selected by production oMLX orchestration.
    fn execute<'a>(
        &'a mut self,
        call: &'a NativeToolCall,
    ) -> Pin<Box<dyn Future<Output = Result<MemoryToolExecution, Self::Error>> + Send + 'a>> {
        Box::pin(execute_audited_python_for_provider(
            &self.store,
            &self.run_id,
            &self.controller,
            self.runner.as_ref(),
            call.clone(),
            &self.cancellation,
        ))
    }
}

/// Builds one exact oMLX-shaped Python call.
fn python_call(call_id: &str) -> OpenAiToolCall {
    OpenAiToolCall::fixture(
        call_id,
        RUN_PYTHON_TOOL_NAME,
        json!({
            "source": "print(6 * 7)",
            "purpose": "Calculate the answer exactly."
        }),
    )
}

/// Waits until generation publishes one exact proposal for review.
async fn pending_review(publisher: &RecordingPublisher) -> PythonApprovalStatus {
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            if let Some(status) = publisher
                .updates
                .lock()
                .unwrap()
                .iter()
                .find_map(Clone::clone)
            {
                return status;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("oMLX Python approval should become visible")
}

#[tokio::test]
async fn approved_python_result_is_bounded_correlated_and_durable_before_provider_reuse() {
    let (store, conversation_id, _message_id, run_id) = active_run("omlx");
    let publisher = RecordingPublisher::default();
    let controller = Arc::new(PythonApprovalController::with_publisher(publisher.clone()));
    let runner = Arc::new(RecordingRunner::default());
    let cancellation = ToolLoopCancellation::default();
    let mut executor = AuditedPythonExecutor {
        store: store.clone(),
        run_id: run_id.clone(),
        controller: controller.clone(),
        runner: runner.clone(),
        cancellation: cancellation.clone(),
    };
    let mut state = ToolLoopState::new(Instant::now());
    let execution = tokio::spawn(async move {
        let result = execute_omlx_tool_round(
            &mut state,
            vec![python_call("omlx-python-approved")],
            &cancellation,
            &mut executor,
        )
        .await;
        (state, result)
    });

    let pending = pending_review(&publisher).await;
    controller
        .decide(&pending.request_id, PythonApprovalDecision::Approve)
        .expect("the visible exact call should be approved");
    let (state, results) = execution.await.unwrap();
    let results = results.expect("approved Python should return one correlated oMLX result");

    assert_eq!(state.call_count(), 1);
    assert!(state.aggregate_output_bytes() > 0);
    assert_eq!(results[0].tool_call_id, "omlx-python-approved");
    assert!(results[0].content.contains(r#""status":"executed""#));
    assert!(results[0].content.contains(r#""stdout":"42\n""#));
    assert_eq!(runner.requests.lock().unwrap().len(), 1);
    store
        .finish_provider_run(&run_id, ProviderRunState::Completed, None, None)
        .unwrap();
    let reopened = store.load_conversation(&conversation_id).unwrap();
    let tool = &reopened.messages[1]
        .provider_run
        .as_ref()
        .unwrap()
        .tool_invocations[0];
    assert_eq!(tool.tool_name, RUN_PYTHON_TOOL_NAME);
    assert_eq!(
        tool.audit
            .approval
            .as_ref()
            .map(|approval| approval.decision),
        Some(ToolApprovalDecision::Approved)
    );
    assert_eq!(tool.audit.outcome, Some(ToolAuditOutcome::Success));
    assert_eq!(tool.result.as_ref().unwrap().output["status"], "executed");
}

#[tokio::test]
async fn denied_python_returns_a_correlated_error_without_starting_the_runner() {
    let (store, conversation_id, _message_id, run_id) = active_run("omlx");
    let publisher = RecordingPublisher::default();
    let controller = Arc::new(PythonApprovalController::with_publisher(publisher.clone()));
    let runner = Arc::new(RecordingRunner::default());
    let cancellation = ToolLoopCancellation::default();
    let mut executor = AuditedPythonExecutor {
        store: store.clone(),
        run_id: run_id.clone(),
        controller: controller.clone(),
        runner: runner.clone(),
        cancellation: cancellation.clone(),
    };
    let mut state = ToolLoopState::new(Instant::now());
    let execution = tokio::spawn(async move {
        execute_omlx_tool_round(
            &mut state,
            vec![python_call("omlx-python-denied")],
            &cancellation,
            &mut executor,
        )
        .await
    });

    let pending = pending_review(&publisher).await;
    controller
        .decide(&pending.request_id, PythonApprovalDecision::Deny)
        .expect("the visible exact call should be denied");
    let results = execution
        .await
        .unwrap()
        .expect("denial should be returned safely to oMLX");

    assert_eq!(results[0].tool_call_id, "omlx-python-denied");
    assert!(results[0].content.contains(r#""ok":false"#));
    assert!(results[0].content.contains(r#""code":"approval_required""#));
    assert!(runner.requests.lock().unwrap().is_empty());
    store
        .finish_provider_run(&run_id, ProviderRunState::Completed, None, None)
        .unwrap();
    let reopened = store.load_conversation(&conversation_id).unwrap();
    let tool = &reopened.messages[1]
        .provider_run
        .as_ref()
        .unwrap()
        .tool_invocations[0];
    assert_eq!(
        tool.audit
            .approval
            .as_ref()
            .map(|approval| approval.decision),
        Some(ToolApprovalDecision::Denied)
    );
    assert_eq!(tool.result.as_ref().unwrap().output["status"], "denied");
}

#[tokio::test]
async fn cancellation_clears_the_review_and_stops_before_provider_reuse() {
    let (store, conversation_id, _message_id, run_id) = active_run("omlx");
    let publisher = RecordingPublisher::default();
    let controller = Arc::new(PythonApprovalController::with_publisher(publisher.clone()));
    let runner = Arc::new(RecordingRunner::default());
    let cancellation = ToolLoopCancellation::default();
    let mut executor = AuditedPythonExecutor {
        store: store.clone(),
        run_id: run_id.clone(),
        controller: controller.clone(),
        runner: runner.clone(),
        cancellation: cancellation.clone(),
    };
    let mut state = ToolLoopState::new(Instant::now());
    let waiting_cancellation = cancellation.clone();
    let execution = tokio::spawn(async move {
        execute_omlx_tool_round(
            &mut state,
            vec![python_call("omlx-python-cancelled")],
            &waiting_cancellation,
            &mut executor,
        )
        .await
    });

    pending_review(&publisher).await;
    cancellation.cancel();
    let error = execution
        .await
        .unwrap()
        .expect_err("cancelled Python must not return a tool result to oMLX");

    assert!(error.message.contains("cancelled"));
    assert!(controller.current().is_none());
    assert!(runner.requests.lock().unwrap().is_empty());
    store
        .finish_provider_run(&run_id, ProviderRunState::Cancelled, None, None)
        .unwrap();
    let reopened = store.load_conversation(&conversation_id).unwrap();
    let tool = &reopened.messages[1]
        .provider_run
        .as_ref()
        .unwrap()
        .tool_invocations[0];
    assert!(tool.audit.approval.is_none());
    assert_eq!(tool.result.as_ref().unwrap().output["status"], "cancelled");
}
