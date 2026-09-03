//! Approval-gated launch boundary for Bottie's standalone Python helper.
//!
//! Provider adapters intentionally do not call this module yet. Source and purpose are written as
//! one bounded JSON request over a private stdin pipe and never enter process arguments or the
//! child environment.

#![allow(
    dead_code,
    reason = "provider mapping into this native execution boundary is intentionally deferred"
)]

use std::{ffi::OsString, future::Future, path::Path, pin::Pin};

#[cfg(target_os = "linux")]
use std::{path::PathBuf, process::Stdio, time::Duration};

use serde::{Deserialize, Serialize};
#[cfg(target_os = "linux")]
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::{Child, Command},
};

use crate::{
    python_approval::{PythonApprovalController, PythonApprovalError, PythonApprovalResolution},
    tool_contract::{PythonToolArguments, validate_python_tool_arguments},
    tool_loop::{NativeToolCall, ToolLoopCancellation},
    tool_policy::authorize_tool_call,
};

/// Maximum stdout bytes accepted from the Linux helper transport, including JSON escaping overhead.
#[cfg(target_os = "linux")]
const MAX_HELPER_RESPONSE_BYTES: u64 = 96 * 1_024;
/// Maximum JSON bytes accepted by the standalone helper's stdin contract.
const MAX_HELPER_REQUEST_BYTES: usize = 256 * 1_024;
/// Maximum stdout or stderr payload accepted inside a decoded helper result.
const MAX_HELPER_STREAM_BYTES: usize = 32 * 1_024;
/// Outer process deadline covering runtime load plus the helper's own execution deadline.
#[cfg(target_os = "linux")]
const HELPER_PROCESS_TIMEOUT: Duration = Duration::from_secs(45);

/// Stable result categories emitted by the standalone Python helper.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PythonExecutionStatus {
    /// Python completed successfully.
    Ok,
    /// Python completed with an interpreter-level error.
    PythonError,
    /// The helper's internal execution deadline elapsed.
    TimedOut,
    /// Captured stdout or stderr exceeded its fixed ceiling.
    OutputLimit,
    /// A WebAssembly memory, table, or other resource ceiling was reached.
    ResourceLimit,
    /// The helper rejected the serialized request.
    InvalidRequest,
    /// The helper could not safely start or complete execution.
    InternalError,
}

/// Complete bounded, path-free result decoded from the helper's stdout pipe.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PythonExecutionResult {
    /// Stable helper-owned completion category.
    pub(crate) status: PythonExecutionStatus,
    /// Bounded Python standard output.
    pub(crate) stdout: String,
    /// Bounded Python standard error.
    pub(crate) stderr: String,
    /// Helper-measured execution duration in milliseconds.
    pub(crate) duration_ms: u64,
}

/// Terminal provider-neutral outcome after approval review and optional execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PythonExecutionOutcome {
    /// One exact approved proposal reached the helper and returned a bounded result.
    Executed(PythonExecutionResult),
    /// The user denied the proposal and no helper was launched.
    Denied,
    /// Shared generation cancellation stopped approval waiting or helper execution.
    Cancelled,
}

/// Stable native execution-boundary failure categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PythonExecutionErrorCode {
    /// Approval publication or exact-call resolution failed closed.
    ApprovalFailed,
    /// The approved call no longer satisfied the closed Python contract.
    InvalidRequest,
    /// The bundled helper could not be launched or completed safely.
    HelperFailed,
    /// Helper stdout was oversized, malformed, or outside the closed result contract.
    InvalidResult,
}

/// Fixed path-free error retained inside native orchestration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PythonExecutionError {
    /// Stable machine-readable failure category.
    pub(crate) code: PythonExecutionErrorCode,
    /// Fixed explanation that never reflects source, purpose, identities, output, or paths.
    pub(crate) message: &'static str,
}

/// Internal runner outcome separating caller cancellation from a completed helper response.
pub(crate) enum PythonRunnerOutcome {
    /// The helper returned one valid bounded result.
    Completed(PythonExecutionResult),
    /// Shared cancellation killed and reaped the helper.
    Cancelled,
}

/// Injected private-process boundary used by native orchestration and focused tests.
pub(crate) trait PythonRunner: Send + Sync {
    /// Executes one validated request while observing shared generation cancellation.
    fn execute<'a>(
        &'a self,
        arguments: PythonToolArguments,
        cancellation: &'a ToolLoopCancellation,
    ) -> Pin<Box<dyn Future<Output = Result<PythonRunnerOutcome, PythonExecutionError>> + Send + 'a>>;
}

/// Native-owned locations for Linux's bundled helper and CPython/WASI runtime.
///
/// macOS and Windows intentionally have no direct-process implementation: their product path must
/// use the separately sandboxed XPC or AppContainer transport proven by the feasibility slices.
#[cfg(target_os = "linux")]
pub(crate) struct LinuxContainedPythonRunner {
    executable: PathBuf,
    runtime_directory: PathBuf,
}

#[cfg(target_os = "linux")]
impl LinuxContainedPythonRunner {
    /// Retains absolute bundle paths resolved by Tauri without exposing them outside Rust.
    pub(crate) fn new(executable: PathBuf, runtime_directory: PathBuf) -> Self {
        Self {
            executable,
            runtime_directory,
        }
    }
}

#[cfg(target_os = "linux")]
impl PythonRunner for LinuxContainedPythonRunner {
    /// Launches Linux's built-in containment mode with private pipes and an empty environment.
    fn execute<'a>(
        &'a self,
        arguments: PythonToolArguments,
        cancellation: &'a ToolLoopCancellation,
    ) -> Pin<Box<dyn Future<Output = Result<PythonRunnerOutcome, PythonExecutionError>> + Send + 'a>>
    {
        Box::pin(execute_linux_contained_helper(
            &self.executable,
            &self.runtime_directory,
            arguments,
            cancellation,
        ))
    }
}

/// Waits for one exact approval and launches only the unchanged authorized Python proposal.
pub(crate) async fn execute_approved_python(
    controller: &PythonApprovalController,
    runner: &impl PythonRunner,
    call: NativeToolCall,
    cancellation: &ToolLoopCancellation,
) -> Result<PythonExecutionOutcome, PythonExecutionError> {
    let approval = controller
        .request_and_wait(call.clone(), cancellation)
        .await
        .map_err(approval_execution_error)?;
    let grant = match approval {
        PythonApprovalResolution::Approved(grant) => grant,
        PythonApprovalResolution::Denied => return Ok(PythonExecutionOutcome::Denied),
        PythonApprovalResolution::Cancelled => return Ok(PythonExecutionOutcome::Cancelled),
    };
    let authorized = authorize_tool_call(&call, Some(grant))
        .map_err(|_| execution_error(PythonExecutionErrorCode::InvalidRequest))?;
    let arguments =
        validate_python_tool_arguments(&authorized.call().tool_name, &authorized.call().arguments)
            .map_err(|_| execution_error(PythonExecutionErrorCode::InvalidRequest))?;
    if cancellation.is_cancelled() {
        return Ok(PythonExecutionOutcome::Cancelled);
    }
    match runner.execute(arguments, cancellation).await? {
        PythonRunnerOutcome::Completed(result) => Ok(PythonExecutionOutcome::Executed(result)),
        PythonRunnerOutcome::Cancelled => Ok(PythonExecutionOutcome::Cancelled),
    }
}

#[derive(Serialize)]
struct HelperRequest<'a> {
    code: &'a str,
    purpose: &'a str,
}

/// Runs Linux's built-in containment mode without a shell, ambient environment, or unbounded output.
#[cfg(target_os = "linux")]
async fn execute_linux_contained_helper(
    executable: &Path,
    runtime_directory: &Path,
    arguments: PythonToolArguments,
    cancellation: &ToolLoopCancellation,
) -> Result<PythonRunnerOutcome, PythonExecutionError> {
    if cancellation.is_cancelled() {
        return Ok(PythonRunnerOutcome::Cancelled);
    }
    let request = encode_helper_request(&arguments)?;

    let mut command = Command::new(executable);
    command
        .args(linux_helper_arguments(runtime_directory))
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|_| execution_error(PythonExecutionErrorCode::HelperFailed))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| execution_error(PythonExecutionErrorCode::HelperFailed))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| execution_error(PythonExecutionErrorCode::HelperFailed))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| execution_error(PythonExecutionErrorCode::HelperFailed))?;

    let writer = tokio::spawn(async move {
        stdin.write_all(&request).await?;
        stdin.shutdown().await
    });
    let stdout_reader = tokio::spawn(read_bounded(stdout, MAX_HELPER_RESPONSE_BYTES));
    let stderr_reader = tokio::spawn(read_bounded(stderr, MAX_HELPER_RESPONSE_BYTES));
    let process_timeout = tokio::time::sleep(HELPER_PROCESS_TIMEOUT);
    tokio::pin!(process_timeout);

    let status = tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            let stopped = stop_child(&mut child).await;
            writer.abort();
            stdout_reader.abort();
            stderr_reader.abort();
            stopped?;
            return Ok(PythonRunnerOutcome::Cancelled);
        }
        _ = &mut process_timeout => {
            let stopped = stop_child(&mut child).await;
            writer.abort();
            stdout_reader.abort();
            stderr_reader.abort();
            stopped?;
            return Err(execution_error(PythonExecutionErrorCode::HelperFailed));
        }
        result = child.wait() => {
            result.map_err(|_| execution_error(PythonExecutionErrorCode::HelperFailed))?
        }
    };
    writer
        .await
        .map_err(|_| execution_error(PythonExecutionErrorCode::HelperFailed))?
        .map_err(|_| execution_error(PythonExecutionErrorCode::HelperFailed))?;
    let stdout = join_reader(stdout_reader).await?;
    let _stderr = join_reader(stderr_reader).await?;
    if !status.success() {
        return Err(execution_error(PythonExecutionErrorCode::HelperFailed));
    }
    decode_helper_result(&stdout).map(PythonRunnerOutcome::Completed)
}

/// Kills and reaps one retained Linux helper, accepting a child that already exited concurrently.
#[cfg(target_os = "linux")]
async fn stop_child(child: &mut Child) -> Result<(), PythonExecutionError> {
    match child.kill().await {
        Ok(()) => Ok(()),
        Err(_) if child.try_wait().is_ok_and(|status| status.is_some()) => Ok(()),
        Err(_) => Err(execution_error(PythonExecutionErrorCode::HelperFailed)),
    }
}

/// Returns Linux's fixed native-only arguments without source, purpose, or provider identity.
pub(crate) fn linux_helper_arguments(runtime_directory: &Path) -> Vec<OsString> {
    vec![
        "--linux-contained".into(),
        "--runtime".into(),
        runtime_directory.as_os_str().to_owned(),
    ]
}

/// Translates product terminology to the helper's exact stdin-only request shape.
pub(crate) fn encode_helper_request(
    arguments: &PythonToolArguments,
) -> Result<Vec<u8>, PythonExecutionError> {
    let request = serde_json::to_vec(&HelperRequest {
        code: &arguments.source,
        purpose: &arguments.purpose,
    })
    .map_err(|_| execution_error(PythonExecutionErrorCode::InvalidRequest))?;
    if request.len() > MAX_HELPER_REQUEST_BYTES {
        Err(execution_error(PythonExecutionErrorCode::InvalidRequest))
    } else {
        Ok(request)
    }
}

/// Reads at most one byte beyond a Linux private-pipe ceiling so overflow is detectable.
#[cfg(target_os = "linux")]
async fn read_bounded(
    reader: impl AsyncRead + Unpin,
    limit: u64,
) -> Result<Vec<u8>, std::io::Error> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes).await?;
    Ok(bytes)
}

/// Joins one Linux private-pipe reader and rejects transport or size failure without raw detail.
#[cfg(target_os = "linux")]
async fn join_reader(
    reader: tokio::task::JoinHandle<Result<Vec<u8>, std::io::Error>>,
) -> Result<Vec<u8>, PythonExecutionError> {
    let bytes = reader
        .await
        .map_err(|_| execution_error(PythonExecutionErrorCode::HelperFailed))?
        .map_err(|_| execution_error(PythonExecutionErrorCode::HelperFailed))?;
    if bytes.len() as u64 > MAX_HELPER_RESPONSE_BYTES {
        Err(execution_error(PythonExecutionErrorCode::InvalidResult))
    } else {
        Ok(bytes)
    }
}

/// Decodes only the helper's exact path-free result contract and rechecks stream ceilings.
pub(crate) fn decode_helper_result(
    encoded: &[u8],
) -> Result<PythonExecutionResult, PythonExecutionError> {
    let result: PythonExecutionResult = serde_json::from_slice(encoded)
        .map_err(|_| execution_error(PythonExecutionErrorCode::InvalidResult))?;
    if result.stdout.len() > MAX_HELPER_STREAM_BYTES
        || result.stderr.len() > MAX_HELPER_STREAM_BYTES
    {
        return Err(execution_error(PythonExecutionErrorCode::InvalidResult));
    }
    Ok(result)
}

/// Maps approval lifecycle failures without reflecting proposal or event details.
fn approval_execution_error(_error: PythonApprovalError) -> PythonExecutionError {
    execution_error(PythonExecutionErrorCode::ApprovalFailed)
}

/// Returns one fixed path-free error for a closed execution-boundary category.
fn execution_error(code: PythonExecutionErrorCode) -> PythonExecutionError {
    let message = match code {
        PythonExecutionErrorCode::ApprovalFailed => {
            "Bottie could not complete Python approval safely."
        }
        PythonExecutionErrorCode::InvalidRequest => {
            "The approved Python request is no longer valid."
        }
        PythonExecutionErrorCode::HelperFailed => {
            "Bottie could not complete the isolated Python helper."
        }
        PythonExecutionErrorCode::InvalidResult => {
            "The isolated Python helper returned an invalid result."
        }
    };
    PythonExecutionError { code, message }
}
