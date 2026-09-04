//! Native private-process transport tests for platform-isolated Python runners.

#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

use crate::{
    python_execution::{
        PythonExecutionResult, PythonExecutionStatus, PythonRunner, PythonRunnerOutcome,
        WindowsAppContainerPythonRunner,
    },
    tool_contract::PythonToolArguments,
    tool_loop::ToolLoopCancellation,
};

/// Creates one executable controller fixture without putting request data in its path or arguments.
fn windows_controller_fixture(body: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "bottie-python-appcontainer-controller-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&directory).expect("the controller fixture directory should be created");
    let executable = directory.join("bottie-python-appcontainer-controller");
    fs::write(&executable, format!("#!/bin/sh\nset -eu\n{body}\n"))
        .expect("the controller fixture should be written");
    let mut permissions = fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&executable, permissions).unwrap();
    executable
}

#[tokio::test]
async fn windows_appcontainer_runner_uses_the_closed_private_pipe_contract() {
    let controller = windows_controller_fixture(
        r#"
[ "$#" -eq 4 ]
[ "$1" = "execute" ]
[ "$2" = "com.bottie.python-runner" ]
[ "$3" = "/native/bottie-python-runner.exe" ]
[ "$4" = "/native/python" ]
request=$(/bin/cat)
[ "$request" = '{"code":"print(6 * 7)","purpose":"Calculate the answer exactly."}' ]
printf '%s' '{"status":"ok","stdout":"42\n","stderr":"","durationMs":12}'
"#,
    );
    let runner = WindowsAppContainerPythonRunner::new(
        controller.clone(),
        "/native/bottie-python-runner.exe".into(),
        "/native/python".into(),
    );

    let outcome = runner
        .execute(
            PythonToolArguments {
                source: "print(6 * 7)".into(),
                purpose: "Calculate the answer exactly.".into(),
            },
            &ToolLoopCancellation::default(),
        )
        .await
        .expect("the AppContainer controller fixture should return one bounded result");

    assert!(matches!(
        outcome,
        PythonRunnerOutcome::Completed(PythonExecutionResult {
            status: PythonExecutionStatus::Ok,
            ref stdout,
            ref stderr,
            duration_ms: 12,
        }) if stdout == "42\n" && stderr.is_empty()
    ));
    fs::remove_dir_all(controller.parent().unwrap()).unwrap();
}

#[tokio::test]
async fn windows_appcontainer_runner_closes_the_controller_on_shared_cancellation() {
    let controller = windows_controller_fixture(
        r#"
: > "${0%/*}/started"
while :; do :; done
"#,
    );
    let started = controller.parent().unwrap().join("started");
    let runner = WindowsAppContainerPythonRunner::new(
        controller.clone(),
        "/native/bottie-python-runner.exe".into(),
        "/native/python".into(),
    );
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
    .expect("the AppContainer controller fixture should start before cancellation");

    cancellation.cancel();

    assert!(matches!(
        execution.await.unwrap().unwrap(),
        PythonRunnerOutcome::Cancelled
    ));
    fs::remove_dir_all(controller.parent().unwrap()).unwrap();
}
