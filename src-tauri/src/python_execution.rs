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

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::python_process_transport::execute_private_pipe_process;
use crate::{
    python_approval::{PythonApprovalController, PythonApprovalError, PythonApprovalResolution},
    tool_contract::{PythonToolArguments, validate_python_tool_arguments},
    tool_loop::{NativeToolCall, ToolLoopCancellation},
    tool_policy::authorize_tool_call,
};

/// Maximum JSON bytes accepted by the standalone helper's stdin contract.
const MAX_HELPER_REQUEST_BYTES: usize = 256 * 1_024;
/// Maximum stdout or stderr payload accepted inside a decoded helper result.
const MAX_HELPER_STREAM_BYTES: usize = 32 * 1_024;
/// Stable result categories emitted by the standalone Python helper.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
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
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
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

/// Native-owned location for Bottie's macOS bridge into its private XPC service.
///
/// The bridge receives only the fixed `execute` mode in its process arguments. It forwards the
/// bounded request through XPC and owns the connection whose invalidation stops the service child.
#[cfg(target_os = "macos")]
pub(crate) struct MacosXpcPythonRunner {
    client_executable: PathBuf,
}

#[cfg(target_os = "macos")]
impl MacosXpcPythonRunner {
    /// Retains the absolute native client path resolved inside Bottie's application bundle.
    pub(crate) fn new(client_executable: PathBuf) -> Self {
        Self { client_executable }
    }
}

#[cfg(target_os = "macos")]
impl PythonRunner for MacosXpcPythonRunner {
    /// Runs the private XPC client with bounded pipes and shared cancellation.
    fn execute<'a>(
        &'a self,
        arguments: PythonToolArguments,
        cancellation: &'a ToolLoopCancellation,
    ) -> Pin<Box<dyn Future<Output = Result<PythonRunnerOutcome, PythonExecutionError>> + Send + 'a>>
    {
        Box::pin(execute_macos_xpc_client(
            &self.client_executable,
            arguments,
            cancellation,
        ))
    }
}

/// Native-owned locations for Bottie's Windows AppContainer controller and contained resources.
///
/// The controller owns the restricted token, zero-capability AppContainer launch, one-process Job
/// Object, and private helper pipes. Closing the controller terminates the retained helper job.
#[cfg(any(
    target_os = "windows",
    all(test, any(target_os = "linux", target_os = "macos"))
))]
pub(crate) struct WindowsAppContainerPythonRunner {
    controller_executable: PathBuf,
    profile_moniker: String,
    runner_executable: PathBuf,
    runtime_directory: PathBuf,
}

#[cfg(any(
    target_os = "windows",
    all(test, any(target_os = "linux", target_os = "macos"))
))]
impl WindowsAppContainerPythonRunner {
    /// Retains native bundle paths without exposing them to the WebView or provider adapters.
    pub(crate) fn new(
        controller_executable: PathBuf,
        profile_moniker: String,
        runner_executable: PathBuf,
        runtime_directory: PathBuf,
    ) -> Self {
        Self {
            controller_executable,
            profile_moniker,
            runner_executable,
            runtime_directory,
        }
    }
}

#[cfg(any(
    target_os = "windows",
    all(test, any(target_os = "linux", target_os = "macos"))
))]
impl PythonRunner for WindowsAppContainerPythonRunner {
    /// Runs the AppContainer controller with bounded pipes and shared cancellation.
    fn execute<'a>(
        &'a self,
        arguments: PythonToolArguments,
        cancellation: &'a ToolLoopCancellation,
    ) -> Pin<Box<dyn Future<Output = Result<PythonRunnerOutcome, PythonExecutionError>> + Send + 'a>>
    {
        Box::pin(execute_windows_appcontainer_controller(
            &self.controller_executable,
            &self.profile_moniker,
            &self.runner_executable,
            &self.runtime_directory,
            arguments,
            cancellation,
        ))
    }
}

/// Waits for one exact approval and launches only the unchanged authorized Python proposal.
pub(crate) async fn execute_approved_python(
    controller: &PythonApprovalController,
    runner: &(impl PythonRunner + ?Sized),
    call: NativeToolCall,
    cancellation: &ToolLoopCancellation,
) -> Result<PythonExecutionOutcome, PythonExecutionError> {
    let approval = resolve_python_approval(controller, call.clone(), cancellation).await?;
    match approval {
        PythonApprovalResolution::Approved(grant) => {
            execute_authorized_python(runner, call, grant, cancellation).await
        }
        PythonApprovalResolution::Denied => Ok(PythonExecutionOutcome::Denied),
        PythonApprovalResolution::Cancelled => Ok(PythonExecutionOutcome::Cancelled),
    }
}

/// Waits for the explicit decision without crossing the helper execution boundary.
pub(crate) async fn resolve_python_approval(
    controller: &PythonApprovalController,
    call: NativeToolCall,
    cancellation: &ToolLoopCancellation,
) -> Result<PythonApprovalResolution, PythonExecutionError> {
    controller
        .request_and_wait(call, cancellation)
        .await
        .map_err(approval_execution_error)
}

/// Revalidates one durably approved exact call before starting the injected runner.
pub(crate) async fn execute_authorized_python(
    runner: &(impl PythonRunner + ?Sized),
    call: NativeToolCall,
    grant: crate::tool_policy::ApprovedToolCall,
    cancellation: &ToolLoopCancellation,
) -> Result<PythonExecutionOutcome, PythonExecutionError> {
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

    execute_private_pipe_process(
        executable,
        linux_helper_arguments(runtime_directory),
        request,
        cancellation,
    )
    .await
}

/// Runs the native macOS XPC client without source-bearing arguments or ambient environment.
#[cfg(target_os = "macos")]
async fn execute_macos_xpc_client(
    client_executable: &Path,
    arguments: PythonToolArguments,
    cancellation: &ToolLoopCancellation,
) -> Result<PythonRunnerOutcome, PythonExecutionError> {
    if cancellation.is_cancelled() {
        return Ok(PythonRunnerOutcome::Cancelled);
    }
    let request = encode_helper_request(&arguments)?;

    execute_private_pipe_process(
        client_executable,
        macos_xpc_client_arguments(),
        request,
        cancellation,
    )
    .await
}

/// Runs the native Windows controller without source-bearing arguments or ambient environment.
#[cfg(any(
    target_os = "windows",
    all(test, any(target_os = "linux", target_os = "macos"))
))]
async fn execute_windows_appcontainer_controller(
    controller_executable: &Path,
    profile_moniker: &str,
    runner_executable: &Path,
    runtime_directory: &Path,
    arguments: PythonToolArguments,
    cancellation: &ToolLoopCancellation,
) -> Result<PythonRunnerOutcome, PythonExecutionError> {
    if cancellation.is_cancelled() {
        return Ok(PythonRunnerOutcome::Cancelled);
    }
    execute_private_pipe_process(
        controller_executable,
        windows_appcontainer_controller_arguments(
            profile_moniker,
            runner_executable,
            runtime_directory,
        ),
        encode_helper_request(&arguments)?,
        cancellation,
    )
    .await
}

/// Returns Linux's fixed native-only arguments without source, purpose, or provider identity.
pub(crate) fn linux_helper_arguments(runtime_directory: &Path) -> Vec<OsString> {
    vec![
        "--linux-contained".into(),
        "--runtime".into(),
        runtime_directory.as_os_str().to_owned(),
    ]
}

/// Returns the macOS client's sole fixed mode without request data or native paths.
#[cfg(target_os = "macos")]
pub(crate) fn macos_xpc_client_arguments() -> Vec<OsString> {
    vec!["execute".into()]
}

/// Returns the Windows controller's fixed mode, owned profile, and native-only bundle paths.
pub(crate) fn windows_appcontainer_controller_arguments(
    profile_moniker: &str,
    runner_executable: &Path,
    runtime_directory: &Path,
) -> Vec<OsString> {
    vec![
        "execute".into(),
        profile_moniker.into(),
        runner_executable.as_os_str().to_owned(),
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
pub(super) fn execution_error(code: PythonExecutionErrorCode) -> PythonExecutionError {
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
