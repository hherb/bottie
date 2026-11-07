//! Runs one Kastellan request through a `kastellan-cli session`: submit, follow, relay the user's controls, and turn
//! however it ends into a [`TaskOutcome`].
//!
//! The protocol itself is pure and lives in [`super::session`]; this module only does the I/O. A session that ends
//! before its task finishes is read by its exit status and stderr ([`early_exit_outcome`]). The whole request is
//! bounded by a timeout, after which the CLI is killed; the Kastellan task keeps running, because closing a session
//! never cancels its tasks.

use std::time::Duration;

use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{ChildStdin, Command},
    sync::mpsc::UnboundedReceiver,
};

use super::session::{
    Control, FOLLOW_ID, SUBMIT_ID, SessionMessage, control_line, follow_line, parse_message,
    submit_line,
};
use super::transport::{Invocation, TaskOutcome, early_exit_outcome, timeout_error};

/// Most stderr bytes kept while a session runs; the failure text shows a tail of it.
const STDERR_KEEP_BYTES: usize = 16 * 1024;

/// Runs `instruction` as one session of `invocation`, calling `on_progress` with each progress line, and returns the
/// outcome plus the Kastellan task id (for provenance) when one was assigned.
///
/// `controls` is read only once the task id is known, so a Stop pressed while the task is being submitted waits and
/// is sent then.
pub(crate) async fn run_session(
    invocation: &Invocation,
    instruction: &str,
    timeout: Duration,
    controls: UnboundedReceiver<Control>,
    on_progress: impl FnMut(String) + Send,
) -> (TaskOutcome, Option<String>) {
    let mut task_id = None;
    let driven = tokio::time::timeout(
        timeout,
        drive(invocation, instruction, controls, on_progress, &mut task_id),
    )
    .await;
    // On timeout the dropped future drops the child, and `kill_on_drop` reaps it.
    let outcome = driven.unwrap_or_else(|_| {
        let error = timeout_error(timeout);
        TaskOutcome::Failed {
            text: error.message,
            code: error.code.as_str(),
        }
    });
    (outcome, task_id.map(|id: i64| id.to_string()))
}

/// The session itself; `task_id` is filled in as soon as Kastellan assigns one, so a timeout can still report it.
async fn drive(
    invocation: &Invocation,
    instruction: &str,
    mut controls: UnboundedReceiver<Control>,
    mut on_progress: impl FnMut(String) + Send,
    task_id: &mut Option<i64>,
) -> TaskOutcome {
    let remote = invocation.program == "ssh";
    let mut child = match Command::new(&invocation.program)
        .args(&invocation.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return TaskOutcome::Failed {
                text: format!("Could not start {}: {error}", invocation.program),
                code: "unavailable",
            };
        }
    };
    let (Some(mut stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        return TaskOutcome::Failed {
            text: "Could not open the session's pipes.".into(),
            code: "internal",
        };
    };
    // Drained concurrently so a chatty stderr can never block the child.
    let stderr_tail = tokio::spawn(read_bounded_tail(stderr));
    let mut lines = BufReader::new(stdout).lines();
    // A write that fails means the child exited; its exit status and stderr below explain why.
    let mut alive = send(&mut stdin, &submit_line(instruction)).await;
    let mut next_id = FOLLOW_ID + 1;
    let mut controls_open = true;
    while alive {
        tokio::select! {
            line = lines.next_line() => {
                let Ok(Some(line)) = line else { break };
                match parse_message(&line) {
                    SessionMessage::Response { id: SUBMIT_ID, result: Ok(result) } => {
                        let Some(id) = result["task_id"].as_i64() else {
                            return TaskOutcome::Failed {
                                text: format!("Kastellan accepted the task but sent no task id: {result}"),
                                code: "malformed_response",
                            };
                        };
                        *task_id = Some(id);
                        alive = send(&mut stdin, &follow_line(id)).await;
                    }
                    SessionMessage::Response { id: SUBMIT_ID | FOLLOW_ID, result: Err(error) } => {
                        return TaskOutcome::Failed {
                            text: format!("Kastellan refused the task.\n\n{error}"),
                            code: "server",
                        };
                    }
                    // A refused Stop or Correct (the task already ended, say) is news, not a failure.
                    SessionMessage::Response { result: Err(error), .. } => on_progress(format!("Refused: {error}")),
                    SessionMessage::Response { .. } | SessionMessage::Progress(None) | SessionMessage::Ignored => {}
                    SessionMessage::Progress(Some(line)) => on_progress(line),
                    SessionMessage::Parked => return parked_outcome(task_id.unwrap_or_default()),
                    SessionMessage::Finished(outcome) => return outcome,
                    SessionMessage::FollowFailed(error) => {
                        let id = task_id.unwrap_or_default();
                        return TaskOutcome::Failed {
                            text: format!(
                                "Bottie lost track of Kastellan task {id}: {error}\n\nThe task may still finish; \
                                 `kastellan-cli tasks status {id}` on the host shows it."
                            ),
                            code: "unavailable",
                        };
                    }
                }
            }
            control = controls.recv(), if controls_open && task_id.is_some() => match control {
                Some(control) => {
                    if control == Control::Stop {
                        on_progress("Stopping…".into());
                    }
                    let line = control_line(next_id, task_id.unwrap_or_default(), &control);
                    next_id += 1;
                    alive = send(&mut stdin, &line).await;
                }
                None => controls_open = false,
            },
        }
    }
    // The session ended without finishing its task.
    drop(stdin);
    let status = child.wait().await.ok().and_then(|status| status.code());
    let stderr = stderr_tail.await.unwrap_or_default();
    early_exit_outcome(status, remote, &stderr)
}

/// The reply for a task that parked on an operator question: where to answer it.
fn parked_outcome(task_id: i64) -> TaskOutcome {
    TaskOutcome::Reply(format!(
        "Kastellan paused task {task_id} for an operator decision.\n\nRun `kastellan-cli inbox list` on the host to \
         see the question and `kastellan-cli inbox resolve <id> approve|deny` to answer it; \
         `kastellan-cli tasks status {task_id}` then shows the result."
    ))
}

/// Writes one request line; `false` when the session's stdin is closed.
async fn send(stdin: &mut ChildStdin, line: &str) -> bool {
    stdin.write_all(line.as_bytes()).await.is_ok() && stdin.flush().await.is_ok()
}

/// Reads `stream` to its end, keeping only the last [`STDERR_KEEP_BYTES`].
async fn read_bounded_tail(mut stream: impl AsyncReadExt + Unpin) -> Vec<u8> {
    let mut kept = Vec::new();
    let mut chunk = [0_u8; 4096];
    while let Ok(read) = stream.read(&mut chunk).await {
        if read == 0 {
            break;
        }
        kept.extend_from_slice(&chunk[..read]);
        if kept.len() > STDERR_KEEP_BYTES {
            kept.drain(..kept.len() - STDERR_KEEP_BYTES);
        }
    }
    kept
}

#[cfg(all(test, unix))]
#[path = "session_run_tests.rs"]
mod tests;
