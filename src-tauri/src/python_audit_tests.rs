//! Durable provider-neutral Python approval and execution audit tests.

use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use serde_json::json;

use crate::{
    python_approval::{
        PythonApprovalController, PythonApprovalDecision, PythonApprovalPublisher,
        PythonApprovalStatus,
    },
    python_audit::{AuditedPythonError, execute_audited_python},
    python_execution::{
        PythonExecutionError, PythonExecutionErrorCode, PythonExecutionOutcome,
        PythonExecutionResult, PythonExecutionStatus, PythonRunner, PythonRunnerOutcome,
    },
    storage::{
        ConversationStore, MessageState, NewProviderRun, NewStoredMessage, ProviderRunState,
        StoredReasoningEffort, StoredRole, ToolApprovalDecision, ToolAuditOutcome, ToolAuditPolicy,
    },
    tool_contract::{PythonToolArguments, RUN_PYTHON_TOOL_NAME},
    tool_loop::{NativeToolCall, ToolLoopCancellation},
};

#[derive(Clone, Default)]
struct RecordingPublisher {
    updates: Arc<Mutex<Vec<Option<PythonApprovalStatus>>>>,
}

impl PythonApprovalPublisher for RecordingPublisher {
    /// Retains public proposals so tests can resolve them without a WebView.
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
    /// Returns one bounded helper result after recording the validated request.
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
                duration_ms: 12,
            }))
        })
    }
}

struct FailingRunner;

impl PythonRunner for FailingRunner {
    /// Simulates a fixed path-free helper launch failure after approval.
    fn execute<'a>(
        &'a self,
        _arguments: PythonToolArguments,
        _cancellation: &'a ToolLoopCancellation,
    ) -> Pin<Box<dyn Future<Output = Result<PythonRunnerOutcome, PythonExecutionError>> + Send + 'a>>
    {
        Box::pin(async {
            Err(PythonExecutionError {
                code: PythonExecutionErrorCode::HelperFailed,
                message: "The contained Python helper could not complete safely.",
            })
        })
    }
}

struct ApprovalCheckingRunner {
    store: ConversationStore,
    conversation_id: String,
    approval_was_durable: AtomicBool,
}

impl PythonRunner for ApprovalCheckingRunner {
    /// Confirms the approval is durable and the result absent before helper work starts.
    fn execute<'a>(
        &'a self,
        _arguments: PythonToolArguments,
        _cancellation: &'a ToolLoopCancellation,
    ) -> Pin<Box<dyn Future<Output = Result<PythonRunnerOutcome, PythonExecutionError>> + Send + 'a>>
    {
        Box::pin(async move {
            let conversation = self.store.load_conversation(&self.conversation_id).unwrap();
            let tool = &conversation.messages[1]
                .provider_run
                .as_ref()
                .unwrap()
                .tool_invocations[0];
            self.approval_was_durable.store(
                tool.audit
                    .approval
                    .as_ref()
                    .map(|approval| approval.decision)
                    == Some(ToolApprovalDecision::Approved)
                    && tool.result.is_none(),
                Ordering::SeqCst,
            );
            Ok(PythonRunnerOutcome::Completed(PythonExecutionResult {
                status: PythonExecutionStatus::Ok,
                stdout: "42\n".into(),
                stderr: String::new(),
                duration_ms: 12,
            }))
        })
    }
}

/// Builds a path-backed active provider run for durable orchestration checks.
fn started_run() -> (ConversationStore, String, String) {
    let path = std::env::temp_dir().join(format!(
        "bottie-python-audit-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let store = ConversationStore::initialize(path).expect("storage should initialize");
    let conversation = store
        .create_conversation("Python audit")
        .expect("conversation should be created");
    let request = store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation.id.clone(),
                role: StoredRole::User,
                text: "Calculate exactly".into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[],
        )
        .expect("request should append");
    let run_id = uuid::Uuid::new_v4().to_string();
    store
        .start_provider_run(NewProviderRun {
            id: run_id.clone(),
            conversation_id: conversation.id.clone(),
            request_message_id: request.id,
            provider_id: "test-provider".into(),
            model_id: "test-model".into(),
            reasoning_effort: StoredReasoningEffort::Off,
            temperature: None,
            max_output_tokens: Some(512),
        })
        .expect("provider run should start");
    (store, conversation.id, run_id)
}

/// Builds one exact bounded Python call.
fn python_call(call_id: &str) -> NativeToolCall {
    NativeToolCall {
        call_id: call_id.into(),
        tool_name: RUN_PYTHON_TOOL_NAME.into(),
        arguments: json!({
            "source": "print(6 * 7)",
            "purpose": "Calculate the answer exactly."
        }),
    }
}

/// Waits until the native publisher exposes one pending review.
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
    .expect("the proposal should become visible")
}

#[tokio::test]
async fn appends_approval_before_execution_and_reopens_the_bounded_result() {
    let (store, conversation_id, run_id) = started_run();
    let publisher = RecordingPublisher::default();
    let controller = Arc::new(PythonApprovalController::with_publisher(publisher.clone()));
    let runner = Arc::new(ApprovalCheckingRunner {
        store: store.clone(),
        conversation_id: conversation_id.clone(),
        approval_was_durable: AtomicBool::new(false),
    });
    let waiting_store = store.clone();
    let waiting_controller = controller.clone();
    let waiting_runner = runner.clone();
    let waiting_run_id = run_id.clone();
    let execution = tokio::spawn(async move {
        execute_audited_python(
            &waiting_store,
            &waiting_run_id,
            &waiting_controller,
            waiting_runner.as_ref(),
            python_call("private-approved-call"),
            &ToolLoopCancellation::default(),
        )
        .await
    });

    let pending = pending_review(&publisher).await;
    controller
        .decide(&pending.request_id, PythonApprovalDecision::Approve)
        .expect("the exact proposal should be approved");
    assert!(matches!(
        execution.await.unwrap().unwrap(),
        PythonExecutionOutcome::Executed(_)
    ));
    assert!(runner.approval_was_durable.load(Ordering::SeqCst));
    store
        .finish_provider_run(&run_id, ProviderRunState::Completed, None, None)
        .expect("provider run should finish");

    let conversation = store
        .load_conversation(&conversation_id)
        .expect("audited conversation should reopen");
    let tool = &conversation.messages[1]
        .provider_run
        .as_ref()
        .unwrap()
        .tool_invocations[0];
    assert_eq!(tool.audit.policy, ToolAuditPolicy::ApprovalRequired);
    assert_eq!(
        tool.audit
            .approval
            .as_ref()
            .map(|approval| approval.decision),
        Some(ToolApprovalDecision::Approved)
    );
    assert_eq!(tool.audit.outcome, Some(ToolAuditOutcome::Success));
    assert_eq!(
        tool.result.as_ref().unwrap().output,
        json!({
            "status": "executed",
            "result": {
                "status": "ok",
                "stdout": "42\n",
                "stderr": "",
                "durationMs": 12
            }
        })
    );
    let serialized = serde_json::to_string(tool).unwrap();
    assert!(!serialized.contains("private-approved-call"));
    assert!(!serialized.contains("sqlite3"));
}

#[tokio::test]
async fn denial_is_durable_and_never_starts_the_runner() {
    let (store, conversation_id, run_id) = started_run();
    let publisher = RecordingPublisher::default();
    let controller = Arc::new(PythonApprovalController::with_publisher(publisher.clone()));
    let runner = Arc::new(RecordingRunner::default());
    let waiting_store = store.clone();
    let waiting_controller = controller.clone();
    let waiting_runner = runner.clone();
    let waiting_run_id = run_id.clone();
    let execution = tokio::spawn(async move {
        execute_audited_python(
            &waiting_store,
            &waiting_run_id,
            &waiting_controller,
            waiting_runner.as_ref(),
            python_call("private-denied-call"),
            &ToolLoopCancellation::default(),
        )
        .await
    });

    let pending = pending_review(&publisher).await;
    controller
        .decide(&pending.request_id, PythonApprovalDecision::Deny)
        .expect("the exact proposal should be denied");
    assert_eq!(
        execution.await.unwrap().unwrap(),
        PythonExecutionOutcome::Denied
    );
    assert!(runner.requests.lock().unwrap().is_empty());
    store
        .finish_provider_run(&run_id, ProviderRunState::Completed, None, None)
        .unwrap();

    let conversation = store.load_conversation(&conversation_id).unwrap();
    let tool = &conversation.messages[1]
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
    assert_eq!(tool.audit.outcome, Some(ToolAuditOutcome::ApprovalRequired));
    assert_eq!(
        tool.result.as_ref().unwrap().output,
        json!({"status": "denied"})
    );
    assert!(tool.result.as_ref().unwrap().is_error);
}

#[tokio::test]
async fn cancellation_without_a_decision_retains_no_approval() {
    let (store, conversation_id, run_id) = started_run();
    let publisher = RecordingPublisher::default();
    let controller = Arc::new(PythonApprovalController::with_publisher(publisher.clone()));
    let runner = Arc::new(RecordingRunner::default());
    let cancellation = ToolLoopCancellation::default();
    let waiting_store = store.clone();
    let waiting_controller = controller.clone();
    let waiting_runner = runner.clone();
    let waiting_cancellation = cancellation.clone();
    let waiting_run_id = run_id.clone();
    let execution = tokio::spawn(async move {
        execute_audited_python(
            &waiting_store,
            &waiting_run_id,
            &waiting_controller,
            waiting_runner.as_ref(),
            python_call("private-cancelled-call"),
            &waiting_cancellation,
        )
        .await
    });

    pending_review(&publisher).await;
    cancellation.cancel();
    assert_eq!(
        execution.await.unwrap().unwrap(),
        PythonExecutionOutcome::Cancelled
    );
    store
        .finish_provider_run(&run_id, ProviderRunState::Cancelled, None, None)
        .unwrap();

    let conversation = store.load_conversation(&conversation_id).unwrap();
    let tool = &conversation.messages[1]
        .provider_run
        .as_ref()
        .unwrap()
        .tool_invocations[0];
    assert_eq!(tool.audit.approval, None);
    assert_eq!(tool.audit.outcome, Some(ToolAuditOutcome::ExecutionFailed));
    assert_eq!(
        tool.result.as_ref().unwrap().output,
        json!({"status": "cancelled"})
    );
}

#[tokio::test]
async fn approved_helper_failure_is_audited_before_returning_the_fixed_error() {
    let (store, conversation_id, run_id) = started_run();
    let publisher = RecordingPublisher::default();
    let controller = Arc::new(PythonApprovalController::with_publisher(publisher.clone()));
    let waiting_store = store.clone();
    let waiting_controller = controller.clone();
    let waiting_run_id = run_id.clone();
    let execution = tokio::spawn(async move {
        execute_audited_python(
            &waiting_store,
            &waiting_run_id,
            &waiting_controller,
            &FailingRunner,
            python_call("private-failed-call"),
            &ToolLoopCancellation::default(),
        )
        .await
    });

    let pending = pending_review(&publisher).await;
    controller
        .decide(&pending.request_id, PythonApprovalDecision::Approve)
        .unwrap();
    let error = execution.await.unwrap().unwrap_err();
    assert!(matches!(
        error,
        AuditedPythonError::Execution(PythonExecutionError {
            code: PythonExecutionErrorCode::HelperFailed,
            ..
        })
    ));
    store
        .finish_provider_run(
            &run_id,
            ProviderRunState::Failed,
            Some("python_failed"),
            None,
        )
        .unwrap();

    let conversation = store.load_conversation(&conversation_id).unwrap();
    let tool = &conversation.messages[1]
        .provider_run
        .as_ref()
        .unwrap()
        .tool_invocations[0];
    assert_eq!(
        tool.audit
            .approval
            .as_ref()
            .map(|approval| approval.decision),
        Some(ToolApprovalDecision::Approved)
    );
    assert_eq!(tool.audit.outcome, Some(ToolAuditOutcome::ExecutionFailed));
    assert_eq!(
        tool.result.as_ref().unwrap().output,
        json!({"status": "failed", "code": "helper_failed"})
    );
}
