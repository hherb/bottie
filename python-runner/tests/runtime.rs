use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{LazyLock, Mutex},
};

use bottie_python_runner::{ExecutionStatus, PythonExecutionRequest, PythonSandbox};
use serde_json::Value;

static RUNTIME_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
static SANDBOX: LazyLock<PythonSandbox> = LazyLock::new(|| {
    let runtime = required_runtime_path(std::env::var_os("BOTTIE_PYTHON_WASI_RUNTIME"))
        .expect("BOTTIE_PYTHON_WASI_RUNTIME must identify the extracted runtime");
    PythonSandbox::load(&runtime).expect("configured runtime should load")
});

fn required_runtime_path(configured: Option<OsString>) -> Result<PathBuf, &'static str> {
    configured
        .map(PathBuf::from)
        .ok_or("runtime is not configured")
}

fn sandbox() -> &'static PythonSandbox {
    &SANDBOX
}

fn execute(sandbox: &PythonSandbox, code: &str) -> bottie_python_runner::PythonExecutionResult {
    sandbox
        .execute(&PythonExecutionRequest {
            code: code.to_owned(),
            purpose: "Exercise the sandbox boundary".to_owned(),
        })
        .expect("sandbox should execute the request")
}

#[test]
#[ignore = "requires the checksum-pinned CPython/WASI runtime"]
fn executes_python_and_denies_ambient_host_capabilities() {
    let _guard = RUNTIME_TEST_LOCK.lock().expect("runtime test lock");
    let sandbox = sandbox();
    let code = r#"
import json
import os

def outcome(action):
    try:
        action()
        return "allowed"
    except Exception as error:
        return type(error).__name__

result = {
    "environment": sorted(os.environ),
    "host_file": outcome(lambda: open("/etc/passwd").read()),
    "network": outcome(lambda: __import__("socket").socket()),
    "runtime_read": outcome(lambda: open("/runtime/LICENSE").read(1)),
    "runtime_write": outcome(lambda: open("/runtime/probe", "w").write("x")),
    "subprocess": outcome(lambda: os.system("echo unsafe")),
    "work_write": outcome(lambda: open("/work/result.txt", "w").write("safe")),
}
print(json.dumps(result, sort_keys=True))
"#;

    let result = execute(sandbox, code);
    assert_eq!(result.status, ExecutionStatus::Ok, "{}", result.stderr);
    let observed: Value = serde_json::from_str(result.stdout.trim()).expect("JSON probe output");
    assert_eq!(observed["environment"], serde_json::json!(["PYTHONHOME"]));
    assert_eq!(observed["host_file"], "FileNotFoundError");
    assert_eq!(observed["network"], "OSError");
    assert_eq!(observed["runtime_read"], "allowed");
    assert_eq!(observed["runtime_write"], "PermissionError");
    assert_ne!(observed["subprocess"], "allowed");
    assert_eq!(observed["work_write"], "PermissionError");
}

#[test]
#[ignore = "requires the checksum-pinned CPython/WASI runtime and thirty seconds"]
fn stops_infinite_execution_at_the_deadline() {
    let _guard = RUNTIME_TEST_LOCK.lock().expect("runtime test lock");
    let sandbox = sandbox();
    let result = execute(sandbox, "while True:\n    pass");
    assert_eq!(result.status, ExecutionStatus::TimedOut);
    assert!(result.duration_ms >= 29_000);
    assert!(result.duration_ms < 35_000);
}

#[test]
#[ignore = "requires the checksum-pinned CPython/WASI runtime"]
fn classifies_output_and_memory_denials_without_returning_backtraces() {
    let _guard = RUNTIME_TEST_LOCK.lock().expect("runtime test lock");
    let sandbox = sandbox();

    let output = execute(sandbox, "print('x' * 100_000)");
    assert_eq!(output.status, ExecutionStatus::OutputLimit);
    assert!(output.stdout.len() <= 32 * 1_024);
    assert!(!output.stderr.contains("wasmtime"));

    let memory = execute(sandbox, "value = bytearray(512 * 1024 * 1024)");
    assert_eq!(memory.status, ExecutionStatus::ResourceLimit);
    assert!(!memory.stderr.contains("wasmtime"));
}

#[test]
fn explicit_runtime_suite_requires_a_configured_runtime() {
    assert!(required_runtime_path(None).is_err());
    assert_eq!(
        required_runtime_path(Some(OsString::from("/runtime"))).expect("configured path"),
        PathBuf::from("/runtime")
    );
}
