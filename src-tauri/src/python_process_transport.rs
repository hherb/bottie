//! Shared bounded private-process transport for native Python containment bridges.

use std::{ffi::OsString, path::Path, process::Stdio, time::Duration};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::{Child, Command},
};

use crate::{
    python_execution::{
        PythonExecutionError, PythonExecutionErrorCode, PythonRunnerOutcome, decode_helper_result,
        execution_error,
    },
    tool_loop::ToolLoopCancellation,
};

/// Maximum stdout bytes accepted from a private transport, including JSON escaping overhead.
const MAX_HELPER_RESPONSE_BYTES: u64 = 96 * 1_024;
/// Outer deadline covering native transport startup plus the helper's execution deadline.
const HELPER_PROCESS_TIMEOUT: Duration = Duration::from_secs(45);

/// Executes one trusted native transport with private bounded pipes and drop-safe termination.
pub(super) async fn execute_private_pipe_process(
    executable: &Path,
    arguments: Vec<OsString>,
    request: Vec<u8>,
    cancellation: &ToolLoopCancellation,
) -> Result<PythonRunnerOutcome, PythonExecutionError> {
    let mut command = Command::new(executable);
    command
        .args(arguments)
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
            let stopped = stop_private_pipe_child(&mut child).await;
            writer.abort();
            stdout_reader.abort();
            stderr_reader.abort();
            stopped?;
            return Ok(PythonRunnerOutcome::Cancelled);
        }
        _ = &mut process_timeout => {
            let stopped = stop_private_pipe_child(&mut child).await;
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

/// Kills and reaps one retained native transport, accepting a concurrent clean exit.
async fn stop_private_pipe_child(child: &mut Child) -> Result<(), PythonExecutionError> {
    match child.kill().await {
        Ok(()) => Ok(()),
        Err(_) if child.try_wait().is_ok_and(|status| status.is_some()) => Ok(()),
        Err(_) => Err(execution_error(PythonExecutionErrorCode::HelperFailed)),
    }
}

/// Reads at most one byte beyond a private-pipe ceiling so overflow is detectable.
async fn read_bounded(
    reader: impl AsyncRead + Unpin,
    limit: u64,
) -> Result<Vec<u8>, std::io::Error> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes).await?;
    Ok(bytes)
}

/// Joins one private-pipe reader and rejects transport or size failure without raw detail.
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
