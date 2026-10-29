//! `kastellan-cli` invocation: planning, bounded execution, and interpretation of its exit contract.
//!
//! A loopback host runs the CLI directly; any other host runs it through `ssh`. Untrusted text never passes through a
//! shell: a task is an argv element locally and stdin remotely, where the remote command line is fixed. Planning and
//! interpretation are pure so the whole contract is testable without a Kastellan installation.

use std::{path::Path, process::Output, time::Duration};

use tokio::io::AsyncWriteExt;

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
/// Hint `kastellan-cli ask` prints when a task parks on an operator question.
const OPERATOR_WAIT_MARKER: &str = "waiting for an operator decision";

/// What to ask the CLI to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action<'a> {
    /// Submit one task and wait for its reply.
    Ask(&'a str),
    /// Read the operator inbox, which proves the CLI runs and reaches Kastellan's database without side effects.
    Probe,
}

/// One concrete process to spawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Invocation {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    /// Bytes written to the child's stdin, which is then closed.
    pub(crate) stdin: Option<String>,
}

/// How a finished task invocation is presented.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TaskOutcome {
    /// Displayable reply text; the run completes.
    Reply(String),
    /// Displayable failure text plus the durable error code; the run fails.
    Failed { text: String, code: &'static str },
}

/// Extracts the instruction from a prefixed prompt, rejecting forms the CLI cannot accept.
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
    // `kastellan-cli ask` parses any argument starting with `--` as a flag.
    if instruction.starts_with('-') {
        return Err(ProviderError::invalid_request(
            "A Kastellan task cannot start with '-'.",
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
    action: Action<'_>,
    home: Option<&Path>,
) -> Result<Invocation, ProviderError> {
    config.validate()?;
    let (subcommand, stdin): (&[&str], Option<&str>) = match action {
        Action::Ask(instruction) => (&["ask"], Some(instruction)),
        Action::Probe => (&["inbox", "list"], None),
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
        let mut args: Vec<String> = subcommand.iter().map(|part| (*part).to_owned()).collect();
        args.extend(stdin.map(str::to_owned));
        return Ok(Invocation {
            program,
            args,
            stdin: None,
        });
    }
    // Non-interactive ssh shells rarely have ~/.local/bin on PATH, so default to the install path.
    let remote_cli = config.cli_path().unwrap_or(DEFAULT_REMOTE_CLI_PATH);
    let mut remote_command = format!("exec {remote_cli} {}", subcommand.join(" "));
    if stdin.is_some() {
        remote_command.push_str(" \"$(cat)\"");
    }
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
        stdin: stdin.map(str::to_owned),
    })
}

/// Maps the `ask` exit contract onto a displayable outcome. Pure.
pub(crate) fn interpret_output(
    exit_code: Option<i32>,
    remote: bool,
    stdout: &[u8],
    stderr: &[u8],
) -> TaskOutcome {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    let reply = stdout.trim();
    if exit_code == Some(0) {
        if !reply.is_empty() {
            return TaskOutcome::Reply(reply.to_owned());
        }
        // Exit 0 with no reply is the CLI's "parked on an operator question" contract.
        if let Some(line) = stderr
            .lines()
            .find(|line| line.contains(OPERATOR_WAIT_MARKER))
        {
            return TaskOutcome::Reply(format!(
                "Kastellan paused this task for an operator decision.\n\n{}",
                line.trim_start_matches("ask: ")
            ));
        }
        return TaskOutcome::Reply("Kastellan finished the task without a reply.".into());
    }
    let detail = stderr_tail(&stderr);
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

/// Extracts the task id from the CLI's submission line, for provenance.
pub(crate) fn submitted_task_id(stderr: &[u8]) -> Option<String> {
    String::from_utf8_lossy(stderr).lines().find_map(|line| {
        let rest = line.strip_prefix("ask: submitted task ")?;
        let id = rest.split_whitespace().next()?;
        id.chars()
            .all(|c| c.is_ascii_digit())
            .then(|| id.to_owned())
    })
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

/// Spawns one invocation and waits for it under a timeout.
pub(crate) async fn run_invocation(
    invocation: &Invocation,
    timeout: Duration,
) -> Result<Output, ProviderError> {
    let mut command = tokio::process::Command::new(&invocation.program);
    command
        .args(&invocation.args)
        .stdin(if invocation.stdin.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|error| {
        ProviderError::unavailable(
            format!("Could not start {}.", invocation.program),
            Some(error.to_string()),
        )
    })?;
    // Feed stdin while waiting, inside the timeout: a child that stops reading must not block the write forever.
    // A write error is ignored because the child exited early, and its exit status and stderr explain why.
    let pipe = child.stdin.take();
    let input = invocation.stdin.clone();
    let feed = async move {
        if let (Some(mut pipe), Some(input)) = (pipe, input) {
            let _ = pipe.write_all(input.as_bytes()).await;
        }
    };
    let finished = async move { tokio::join!(feed, child.wait_with_output()).1 };
    // On timeout the dropped future drops the child, and `kill_on_drop` reaps it.
    match tokio::time::timeout(timeout, finished).await {
        Ok(result) => result.map_err(|error| {
            ProviderError::unavailable("kastellan-cli failed.", Some(error.to_string()))
        }),
        Err(_) => Err(ProviderError {
            code: ProviderErrorCode::Timeout,
            message: format!(
                "No answer from Kastellan within {}. A submitted task may still finish; check \
                 `kastellan-cli tasks list` on the host.",
                describe_duration(timeout)
            ),
            retryable: false,
            diagnostic: None,
        }),
    }
}

/// Runs one planned task and turns every result, including transport failures, into an outcome.
pub(crate) async fn execute(
    invocation: &Invocation,
    timeout: Duration,
) -> (TaskOutcome, Option<String>) {
    let remote = invocation.program == "ssh";
    match run_invocation(invocation, timeout).await {
        Ok(output) => (
            interpret_output(output.status.code(), remote, &output.stdout, &output.stderr),
            submitted_task_id(&output.stderr),
        ),
        Err(error) => (
            TaskOutcome::Failed {
                text: error.message,
                code: error.code.as_str(),
            },
            None,
        ),
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
