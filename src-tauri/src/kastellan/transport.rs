//! `kastellan-cli` invocation: planning, bounded execution, and interpretation of its exit status.
//!
//! A loopback host runs the CLI directly; any other host runs it through `ssh`. Untrusted text never passes through a
//! shell: the remote command line is fixed, and a task travels as a JSON-RPC line on the session's stdin (see
//! [`super::session`]). Planning and interpretation are pure so the contract is testable without a Kastellan
//! installation.

use std::{path::Path, process::Output, time::Duration};

use super::config::{DEFAULT_CLI_RELATIVE_PATH, DEFAULT_REMOTE_CLI_PATH, KastellanConfig};
use crate::inference::{ProviderError, ProviderErrorCode};

/// Prompt prefix that routes a message to Kastellan instead of the selected model.
pub(crate) const KASTELLAN_PREFIX: &str = "@kastellan:";
/// Bound on the CLI stderr retained in a failure message.
const STDERR_TAIL_BYTES: usize = 2048;
/// `ssh` reserves this exit status for its own connection failures.
const SSH_FAILURE_STATUS: i32 = 255;
/// Seconds `ssh` may spend establishing a connection.
const SSH_CONNECT_TIMEOUT_SECONDS: u32 = 10;
/// Largest accepted task, matching the composer's draft limit.
pub(crate) const MAX_INSTRUCTION_BYTES: usize = 32 * 1024;

/// What to ask the CLI to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    /// Open a `kastellan-cli session`; the task and the user's controls travel on its stdin.
    Session,
    /// Read the operator inbox, which proves the CLI runs and reaches Kastellan's database without side effects.
    Probe,
}

/// One concrete process to spawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Invocation {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
}

/// How a finished task invocation is presented.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TaskOutcome {
    /// Displayable reply text; the run completes.
    Reply(String),
    /// Displayable failure text plus the durable error code; the run fails.
    Failed { text: String, code: &'static str },
    /// The user stopped the task; the run is saved as cancelled with this text.
    Stopped(String),
}

/// Extracts the instruction from a prefixed prompt, rejecting forms Kastellan cannot accept.
pub(crate) fn instruction_from_prompt(prompt: &str) -> Result<&str, ProviderError> {
    let instruction = prompt
        .trim_start()
        .strip_prefix(KASTELLAN_PREFIX)
        .ok_or_else(|| {
            ProviderError::invalid_request(format!(
                "Kastellan requests start with {KASTELLAN_PREFIX}"
            ))
        })?
        .trim();
    if instruction.is_empty() {
        return Err(ProviderError::invalid_request(
            "Write the task for Kastellan after the prefix.",
        ));
    }
    if instruction.len() > MAX_INSTRUCTION_BYTES {
        return Err(ProviderError::invalid_request(
            "A Kastellan task is limited to 32 KiB of text.",
        ));
    }
    if instruction.contains('\0') {
        return Err(ProviderError::invalid_request(
            "A Kastellan task cannot contain NUL characters.",
        ));
    }
    Ok(instruction)
}

/// Builds the process to spawn. Pure: `home` is the local home directory, when known.
pub(crate) fn plan_invocation(
    config: &KastellanConfig,
    action: Action,
    home: Option<&Path>,
) -> Result<Invocation, ProviderError> {
    config.validate()?;
    let subcommand: &[&str] = match action {
        Action::Session => &["session"],
        Action::Probe => &["inbox", "list"],
    };
    if config.is_local() {
        let program = match config.cli_path() {
            Some(path) => path.to_owned(),
            None => home
                .map(|home| home.join(DEFAULT_CLI_RELATIVE_PATH))
                .ok_or_else(|| {
                    ProviderError::invalid_request(
                        "Set the kastellan-cli path in Settings; the home folder is unknown.",
                    )
                })?
                .to_string_lossy()
                .into_owned(),
        };
        return Ok(Invocation {
            program,
            args: subcommand.iter().map(|part| (*part).to_owned()).collect(),
        });
    }
    // Non-interactive ssh shells rarely have ~/.local/bin on PATH, so default to the install path.
    let remote_cli = config.cli_path().unwrap_or(DEFAULT_REMOTE_CLI_PATH);
    let remote_command = format!("exec {remote_cli} {}", subcommand.join(" "));
    Ok(Invocation {
        program: "ssh".into(),
        args: vec![
            "-o".into(),
            "BatchMode=yes".into(),
            "-o".into(),
            format!("ConnectTimeout={SSH_CONNECT_TIMEOUT_SECONDS}"),
            "--".into(),
            config.host().into(),
            remote_command,
        ],
    })
}

/// The outcome of a session that ended before its task finished: ssh's own failure, or the CLI's error. Pure.
pub(crate) fn early_exit_outcome(
    exit_code: Option<i32>,
    remote: bool,
    stderr: &[u8],
) -> TaskOutcome {
    let detail = stderr_tail(&String::from_utf8_lossy(stderr));
    if remote && exit_code == Some(SSH_FAILURE_STATUS) {
        return TaskOutcome::Failed {
            text: format!("Could not reach the Kastellan host over ssh.\n\n{detail}"),
            code: "unavailable",
        };
    }
    TaskOutcome::Failed {
        text: format!("Kastellan could not complete the task.\n\n{detail}"),
        code: "server",
    }
}

/// Maps a probe's exit status onto a connection-test result. Pure; the probe's stdout is never shown.
pub(crate) fn interpret_probe(
    exit_code: Option<i32>,
    remote: bool,
    host: &str,
    stderr: &[u8],
) -> Result<String, ProviderError> {
    if exit_code == Some(0) {
        return Ok(if remote {
            format!("Reached Kastellan on {host} over ssh.")
        } else {
            "Reached Kastellan on this computer.".into()
        });
    }
    let detail = stderr_tail(&String::from_utf8_lossy(stderr));
    if remote && exit_code == Some(SSH_FAILURE_STATUS) {
        return Err(ProviderError::unavailable(
            format!("Could not connect to {host} over ssh. {detail}"),
            None,
        ));
    }
    Err(ProviderError::unavailable(
        format!("kastellan-cli ran but could not reach Kastellan. {detail}"),
        None,
    ))
}

/// Keeps the last bounded slice of stderr on a character boundary.
pub(crate) fn stderr_tail(stderr: &str) -> String {
    let trimmed = stderr.trim();
    if trimmed.len() <= STDERR_TAIL_BYTES {
        return trimmed.to_owned();
    }
    let mut start = trimmed.len() - STDERR_TAIL_BYTES;
    while !trimmed.is_char_boundary(start) {
        start += 1;
    }
    format!("…{}", &trimmed[start..])
}

/// Renders a timeout in the unit a person would use.
pub(crate) fn describe_duration(duration: Duration) -> String {
    const SECONDS_PER_MINUTE: u64 = 60;
    let seconds = duration.as_secs();
    if seconds >= SECONDS_PER_MINUTE && seconds % SECONDS_PER_MINUTE == 0 {
        let minutes = seconds / SECONDS_PER_MINUTE;
        format!("{minutes} minute{}", if minutes == 1 { "" } else { "s" })
    } else {
        format!("{seconds} second{}", if seconds == 1 { "" } else { "s" })
    }
}

/// Spawns one invocation with no input and waits for it under a timeout (the probe; a task uses
/// [`super::session_run::run_session`]).
pub(crate) async fn run_invocation(
    invocation: &Invocation,
    timeout: Duration,
) -> Result<Output, ProviderError> {
    let mut command = tokio::process::Command::new(&invocation.program);
    command
        .args(&invocation.args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let child = command.spawn().map_err(|error| {
        ProviderError::unavailable(
            format!("Could not start {}.", invocation.program),
            Some(error.to_string()),
        )
    })?;
    // On timeout the dropped future drops the child, and `kill_on_drop` reaps it.
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(result) => result.map_err(|error| {
            ProviderError::unavailable("kastellan-cli failed.", Some(error.to_string()))
        }),
        Err(_) => Err(timeout_error(timeout)),
    }
}

/// The failure for a CLI that did not finish within `timeout`. The Kastellan task itself keeps running.
pub(crate) fn timeout_error(timeout: Duration) -> ProviderError {
    ProviderError {
        code: ProviderErrorCode::Timeout,
        message: format!(
            "No answer from Kastellan within {}. A submitted task may still finish; check \
             `kastellan-cli tasks list` on the host.",
            describe_duration(timeout)
        ),
        retryable: false,
        diagnostic: None,
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
