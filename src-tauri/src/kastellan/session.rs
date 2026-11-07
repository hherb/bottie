//! The `kastellan-cli session` protocol: the request lines Bottie writes and the messages it reads back. Pure.
//!
//! A session is JSON-RPC 2.0, one JSON object per line, on the CLI's stdin and stdout. Bottie opens one session per
//! Kastellan request: `task.submit` answers with the task id, `task.follow` then streams the task's notebook
//! (`notebook.entry`), says when it parks on an operator question (`task.awaiting_operator`), and ends with
//! `task.finished` (or `task.follow_failed`). While it runs, Stop sends `task.cancel` and Correct `task.correct`.
//! Kastellan's own method table is in its `core/src/bin/kastellan-cli/session.rs`.

use serde_json::{Value, json};

use super::transport::TaskOutcome;

/// Request id of the `task.submit` that opens every session.
pub(crate) const SUBMIT_ID: i64 = 1;
/// Request id of the `task.follow` sent once the task id is known.
pub(crate) const FOLLOW_ID: i64 = 2;
/// Most characters of one model-written text a progress line shows.
const PROGRESS_TEXT_CAP: usize = 160;

/// What the user asked of a running request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Control {
    /// Cancel the task.
    Stop,
    /// Add an operator correction the planner reads on its next turn.
    Correct(String),
}

/// One line read from the session, as far as Bottie cares.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SessionMessage {
    /// The answer to request `id`: its `result`, or its error's message.
    Response {
        id: i64,
        result: Result<Value, String>,
    },
    /// A notebook entry, as one short progress line; `None` for kinds the final reply covers.
    Progress(Option<String>),
    /// The task parked on an operator question; the session is done with it.
    Parked,
    /// The task ended.
    Finished(TaskOutcome),
    /// The follow lost track of the task, with Kastellan's reason.
    FollowFailed(String),
    /// Anything else, including a line that is not JSON.
    Ignored,
}

/// One JSON-RPC request line, newline-terminated.
pub(crate) fn request_line(id: i64, method: &str, params: Value) -> String {
    let mut line =
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string();
    line.push('\n');
    line
}

/// The `task.submit` that opens a session. Kastellan's own keyword classifier sets the data floor.
pub(crate) fn submit_line(instruction: &str) -> String {
    request_line(
        SUBMIT_ID,
        "task.submit",
        json!({"instruction": instruction}),
    )
}

/// The `task.follow` for `task_id`, from its first entry.
pub(crate) fn follow_line(task_id: i64) -> String {
    request_line(FOLLOW_ID, "task.follow", json!({"task_id": task_id}))
}

/// The request line for one user control, numbered `id`.
pub(crate) fn control_line(id: i64, task_id: i64, control: &Control) -> String {
    match control {
        Control::Stop => request_line(id, "task.cancel", json!({"task_id": task_id})),
        Control::Correct(text) => request_line(
            id,
            "task.correct",
            json!({"task_id": task_id, "text": text}),
        ),
    }
}

/// Reads one line from the session.
pub(crate) fn parse_message(line: &str) -> SessionMessage {
    let Ok(message) = serde_json::from_str::<Value>(line) else {
        return SessionMessage::Ignored;
    };
    let params = &message["params"];
    if let Some(id) = message.get("id").and_then(Value::as_i64) {
        let result = match message.get("error") {
            Some(error) => Err(error["message"]
                .as_str()
                .unwrap_or("request refused")
                .to_owned()),
            None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
        };
        return SessionMessage::Response { id, result };
    }
    match message["method"].as_str() {
        Some("notebook.entry") => SessionMessage::Progress(entry_line(
            params["kind"].as_str().unwrap_or(""),
            &params["payload"],
        )),
        Some("task.awaiting_operator") => SessionMessage::Parked,
        Some("task.finished") => SessionMessage::Finished(finished_outcome(params)),
        Some("task.follow_failed") => SessionMessage::FollowFailed(
            params["error"]
                .as_str()
                .unwrap_or("unknown error")
                .to_owned(),
        ),
        _ => SessionMessage::Ignored,
    }
}

/// The progress line for one notebook entry, matching `kastellan-cli ask`'s stderr lines without their `ask:`
/// prefix. The text is model-written, so control characters are blanked and each text is capped.
pub(crate) fn entry_line(kind: &str, payload: &Value) -> Option<String> {
    let text = |key: &str| clean(payload.get(key).and_then(Value::as_str).unwrap_or(""));
    let step = || payload.get("step").and_then(Value::as_u64).unwrap_or(0);
    Some(match kind {
        "steps" => {
            let steps = payload
                .get("steps")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let listed: Vec<String> = steps
                .iter()
                .map(|s| {
                    let id = s.get("id").and_then(Value::as_u64).unwrap_or(0);
                    format!(
                        "{id}. {}",
                        clean(s.get("purpose").and_then(Value::as_str).unwrap_or(""))
                    )
                })
                .collect();
            format!("Plan: {}", listed.join("; "))
        }
        "tick" if payload.get("partial").and_then(Value::as_bool) == Some(true) => {
            format!("Step {} progress: {}", step(), text("note"))
        }
        "tick" => format!("Step {} done: {}", step(), text("note")),
        "failed_step" => format!(
            "Step {} failed ({}): {}",
            step(),
            text("code"),
            text("detail")
        ),
        "correction" => format!("Correction: {}", text("text")),
        "infeasible" => format!("Cannot be done: {}", text("reason")),
        _ => return None,
    })
}

/// How a finished task is saved: a completed task's answer, a cancelled task as stopped, anything else as a failure
/// with Kastellan's reason.
pub(crate) fn finished_outcome(params: &Value) -> TaskOutcome {
    let state = params["state"].as_str().unwrap_or("unknown");
    match (state, params["answer"].as_str()) {
        ("completed", Some(answer)) => TaskOutcome::Reply(answer.to_owned()),
        // A result of a kind other than text: show it rather than lose it.
        ("completed", None) => TaskOutcome::Reply(
            serde_json::to_string_pretty(&params["result"])
                .unwrap_or_else(|_| params["result"].to_string()),
        ),
        ("cancelled", _) => TaskOutcome::Stopped("Kastellan stopped this task.".into()),
        (state, _) => TaskOutcome::Failed {
            text: match params["detail"].as_str() {
                Some(detail) => {
                    format!("Kastellan could not complete the task ({state}).\n\n{detail}")
                }
                None => format!("Kastellan could not complete the task ({state})."),
            },
            code: "server",
        },
    }
}

/// `s` on one line with no control character, capped at [`PROGRESS_TEXT_CAP`] characters (an ellipsis marks a cut).
fn clean(s: &str) -> String {
    let mut out: String = s
        .chars()
        .take(PROGRESS_TEXT_CAP)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if s.chars().nth(PROGRESS_TEXT_CAP).is_some() {
        out.push('…');
    }
    out
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
