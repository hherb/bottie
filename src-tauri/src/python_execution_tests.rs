//! Provider-neutral approved Python execution orchestration tests.

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

#[cfg(target_os = "macos")]
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

use serde_json::json;

use crate::{
    python_approval::{
        PythonApprovalController, PythonApprovalDecision, PythonApprovalPublisher,
        PythonApprovalStatus,
    },
    python_execution::{
        PythonExecutionError, PythonExecutionOutcome, PythonExecutionResult, PythonExecutionStatus,
        PythonRunner, PythonRunnerOutcome, decode_helper_result, encode_helper_request,
        execute_approved_python, linux_helper_arguments, windows_appcontainer_controller_arguments,
    },
    tool_contract::{PythonToolArguments, RUN_PYTHON_TOOL_NAME},
    tool_loop::{NativeToolCall, ToolLoopCancellation},
};

#[cfg(target_os = "macos")]
use crate::python_execution::{MacosXpcPythonRunner, macos_xpc_client_arguments};

#[derive(Clone, Default)]
struct RecordingPublisher {
    updates: Arc<Mutex<Vec<Option<PythonApprovalStatus>>>>,
}

impl PythonApprovalPublisher for RecordingPublisher {
    /// Retains the public review so a test can resolve the pending proposal.
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
    /// Records only the validated helper request supplied after approval.
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

#[derive(Default)]
struct CancellationRunner {
    started: tokio::sync::Notify,
}

impl PythonRunner for CancellationRunner {
    /// Waits for the shared signal so cancellation is exercised after approval.
    fn execute<'a>(
        &'a self,
        _arguments: PythonToolArguments,
        cancellation: &'a ToolLoopCancellation,
    ) -> Pin<Box<dyn Future<Output = Result<PythonRunnerOutcome, PythonExecutionError>> + Send + 'a>>
    {
        Box::pin(async move {
            self.started.notify_one();
            cancellation.cancelled().await;
            Ok(PythonRunnerOutcome::Cancelled)
        })
    }
}

/// Builds one exact bounded Python call for orchestration tests.
fn python_call() -> NativeToolCall {
    NativeToolCall {
        call_id: "private-provider-call".into(),
        tool_name: RUN_PYTHON_TOOL_NAME.into(),
        arguments: json!({
            "source": "print(6 * 7)",
            "purpose": "Calculate the answer exactly."
        }),
    }
}

/// Creates one executable bridge fixture without putting request data in its path or arguments.
#[cfg(target_os = "macos")]
fn macos_bridge_fixture(body: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("bottie-python-xpc-client-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).expect("the bridge fixture directory should be created");
    let executable = directory.join("bottie-python-xpc-client");
    fs::write(&executable, format!("#!/bin/sh\nset -eu\n{body}\n"))
        .expect("the bridge fixture should be written");
    let mut permissions = fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&executable, permissions).unwrap();
    executable
}

/// Waits until the publisher has received one pending review.
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
    .expect("the proposal should become visible for approval")
}

#[tokio::test]
async fn executes_once_only_after_the_exact_approval_is_consumed() {
    let publisher = RecordingPublisher::default();
    let controller = Arc::new(PythonApprovalController::with_publisher(publisher.clone()));
    let runner = Arc::new(RecordingRunner::default());
    let waiting_controller = controller.clone();
    let waiting_runner = runner.clone();
    let execution = tokio::spawn(async move {
        execute_approved_python(
            &waiting_controller,
            waiting_runner.as_ref(),
            python_call(),
            &ToolLoopCancellation::default(),
        )
        .await
    });

    let pending = pending_review(&publisher).await;
    assert!(runner.requests.lock().unwrap().is_empty());
    controller
        .decide(&pending.request_id, PythonApprovalDecision::Approve)
        .expect("the exact visible proposal should be approved");

    assert_eq!(
        execution.await.unwrap().unwrap(),
        PythonExecutionOutcome::Executed(PythonExecutionResult {
            status: PythonExecutionStatus::Ok,
            stdout: "42\n".into(),
            stderr: String::new(),
            duration_ms: 12,
        })
    );
    assert_eq!(
        *runner.requests.lock().unwrap(),
        vec![PythonToolArguments {
            source: "print(6 * 7)".into(),
            purpose: "Calculate the answer exactly.".into(),
        }]
    );
    assert_eq!(controller.current(), None);
}

#[tokio::test]
async fn denial_is_terminal_and_never_touches_the_runner() {
    let publisher = RecordingPublisher::default();
    let controller = Arc::new(PythonApprovalController::with_publisher(publisher.clone()));
    let runner = Arc::new(RecordingRunner::default());
    let waiting_controller = controller.clone();
    let waiting_runner = runner.clone();
    let execution = tokio::spawn(async move {
        execute_approved_python(
            &waiting_controller,
            waiting_runner.as_ref(),
            python_call(),
            &ToolLoopCancellation::default(),
        )
        .await
    });

    let pending = pending_review(&publisher).await;
    controller
        .decide(&pending.request_id, PythonApprovalDecision::Deny)
        .expect("the exact visible proposal should be denied");

    assert_eq!(
        execution.await.unwrap().unwrap(),
        PythonExecutionOutcome::Denied
    );
    assert!(runner.requests.lock().unwrap().is_empty());
    assert_eq!(controller.current(), None);
}

#[tokio::test]
async fn cancellation_while_pending_is_terminal_and_never_touches_the_runner() {
    let publisher = RecordingPublisher::default();
    let controller = Arc::new(PythonApprovalController::with_publisher(publisher.clone()));
    let runner = Arc::new(RecordingRunner::default());
    let cancellation = ToolLoopCancellation::default();
    let waiting_controller = controller.clone();
    let waiting_runner = runner.clone();
    let waiting_cancellation = cancellation.clone();
    let execution = tokio::spawn(async move {
        execute_approved_python(
            &waiting_controller,
            waiting_runner.as_ref(),
            python_call(),
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
    assert!(runner.requests.lock().unwrap().is_empty());
    assert_eq!(controller.current(), None);
}

#[tokio::test]
async fn shared_cancellation_stops_an_already_approved_execution() {
    let publisher = RecordingPublisher::default();
    let controller = Arc::new(PythonApprovalController::with_publisher(publisher.clone()));
    let runner = Arc::new(CancellationRunner::default());
    let cancellation = ToolLoopCancellation::default();
    let waiting_controller = controller.clone();
    let waiting_runner = runner.clone();
    let waiting_cancellation = cancellation.clone();
    let execution = tokio::spawn(async move {
        execute_approved_python(
            &waiting_controller,
            waiting_runner.as_ref(),
            python_call(),
            &waiting_cancellation,
        )
        .await
    });

    let pending = pending_review(&publisher).await;
    controller
        .decide(&pending.request_id, PythonApprovalDecision::Approve)
        .expect("the exact visible proposal should be approved");
    runner.started.notified().await;
    cancellation.cancel();

    assert_eq!(
        execution.await.unwrap().unwrap(),
        PythonExecutionOutcome::Cancelled
    );
}

#[test]
fn helper_request_uses_only_the_closed_stdin_shape() {
    let request = encode_helper_request(&PythonToolArguments {
        source: "print(6 * 7)".into(),
        purpose: "Calculate the answer exactly.".into(),
    })
    .expect("bounded approved arguments should encode");
    let value: serde_json::Value = serde_json::from_slice(&request).unwrap();

    assert_eq!(
        value,
        json!({
            "code": "print(6 * 7)",
            "purpose": "Calculate the answer exactly."
        })
    );
    assert!(
        !String::from_utf8(request)
            .unwrap()
            .contains("private-provider-call")
    );

    let oversized = encode_helper_request(&PythonToolArguments {
        source: "x".repeat(256 * 1_024),
        purpose: "Reject the transport overflow.".into(),
    })
    .expect_err("the helper stdin ceiling must be checked independently");
    assert_eq!(
        oversized.code,
        crate::python_execution::PythonExecutionErrorCode::InvalidRequest
    );
    assert!(!oversized.message.contains('x'));
}

#[test]
fn linux_helper_arguments_contain_only_containment_mode_and_the_native_runtime_path() {
    let arguments = linux_helper_arguments(std::path::Path::new("/native/runtime"));

    assert_eq!(
        arguments,
        vec![
            std::ffi::OsString::from("--linux-contained"),
            std::ffi::OsString::from("--runtime"),
            std::ffi::OsString::from("/native/runtime"),
        ]
    );
    let rendered = arguments
        .iter()
        .map(|argument| argument.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(!rendered.contains("print(6 * 7)"));
    assert!(!rendered.contains("private-provider-call"));
}

#[test]
fn windows_controller_arguments_select_the_fixed_profile_and_native_bundle_paths() {
    let arguments = windows_appcontainer_controller_arguments(
        std::path::Path::new(r"C:\Program Files\bottie\bottie-python-runner.exe"),
        std::path::Path::new(r"C:\Program Files\bottie\python"),
    );

    assert_eq!(
        arguments,
        vec![
            std::ffi::OsString::from("execute"),
            std::ffi::OsString::from("com.bottie.python-runner"),
            std::ffi::OsString::from(r"C:\Program Files\bottie\bottie-python-runner.exe"),
            std::ffi::OsString::from(r"C:\Program Files\bottie\python"),
        ]
    );
    let rendered = arguments
        .iter()
        .map(|argument| argument.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(!rendered.contains("print(6 * 7)"));
    assert!(!rendered.contains("private-provider-call"));
}

#[cfg(target_os = "macos")]
#[test]
fn macos_xpc_client_arguments_select_only_the_fixed_execution_mode() {
    assert_eq!(
        macos_xpc_client_arguments(),
        vec![std::ffi::OsString::from("execute")]
    );
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn macos_xpc_runner_uses_the_closed_private_pipe_contract() {
    let executable = macos_bridge_fixture(
        r#"
[ "$#" -eq 1 ]
[ "$1" = "execute" ]
request=$(/bin/cat)
[ "$request" = '{"code":"print(6 * 7)","purpose":"Calculate the answer exactly."}' ]
printf '%s' '{"status":"ok","stdout":"42\n","stderr":"","durationMs":12}'
"#,
    );
    let runner = MacosXpcPythonRunner::new(executable.clone());

    let outcome = runner
        .execute(
            PythonToolArguments {
                source: "print(6 * 7)".into(),
                purpose: "Calculate the answer exactly.".into(),
            },
            &ToolLoopCancellation::default(),
        )
        .await
        .expect("the XPC client fixture should return one bounded result");

    assert!(matches!(
        outcome,
        PythonRunnerOutcome::Completed(PythonExecutionResult {
            status: PythonExecutionStatus::Ok,
            ref stdout,
            ref stderr,
            duration_ms: 12,
        }) if stdout == "42\n" && stderr.is_empty()
    ));
    fs::remove_dir_all(executable.parent().unwrap()).unwrap();
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn macos_xpc_runner_kills_the_client_when_shared_cancellation_arrives() {
    let executable = macos_bridge_fixture(
        r#"
: > "${0%/*}/started"
while :; do :; done
"#,
    );
    let started = executable.parent().unwrap().join("started");
    let runner = MacosXpcPythonRunner::new(executable.clone());
    let cancellation = ToolLoopCancellation::default();
    let waiting_cancellation = cancellation.clone();
    let execution = tokio::spawn(async move {
        runner
            .execute(
                PythonToolArguments {
                    source: "while True:\n    pass".into(),
                    purpose: "Exercise shared cancellation.".into(),
                },
                &waiting_cancellation,
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while !started.exists() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the XPC client fixture should start before cancellation");

    cancellation.cancel();

    assert!(matches!(
        execution.await.unwrap().unwrap(),
        PythonRunnerOutcome::Cancelled
    ));
    fs::remove_dir_all(executable.parent().unwrap()).unwrap();
}

#[test]
fn helper_result_decoder_accepts_only_the_bounded_closed_contract() {
    let valid = br#"{"status":"ok","stdout":"42\n","stderr":"","durationMs":12}"#;
    assert_eq!(
        decode_helper_result(valid).unwrap(),
        PythonExecutionResult {
            status: PythonExecutionStatus::Ok,
            stdout: "42\n".into(),
            stderr: String::new(),
            duration_ms: 12,
        }
    );

    for invalid in [
        br#"{"status":"ok","stdout":"42\n","stderr":"","durationMs":12,"path":"/private"}"#
            .as_slice(),
        br#"{"status":"future","stdout":"","stderr":"","durationMs":0}"#.as_slice(),
        br#"{"status":"ok","stdout":42,"stderr":"","durationMs":0}"#.as_slice(),
    ] {
        assert!(decode_helper_result(invalid).is_err());
    }

    let oversized = serde_json::to_vec(&json!({
        "status": "ok",
        "stdout": "x".repeat(32 * 1_024 + 1),
        "stderr": "",
        "durationMs": 1
    }))
    .unwrap();
    assert!(decode_helper_result(&oversized).is_err());
}
