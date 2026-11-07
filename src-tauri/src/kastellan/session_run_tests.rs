//! Session runner tests against a stand-in `kastellan-cli session`: a shell script that reads request lines and
//! answers with canned JSON-RPC, so the whole exchange runs through real pipes.

use std::{
    os::unix::fs::PermissionsExt,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tokio::sync::mpsc;

use super::*;

/// Writes an executable stand-in for `kastellan-cli` running `body`.
fn fake_cli(body: &str) -> Invocation {
    let directory =
        std::env::temp_dir().join(format!("bottie-kastellan-session-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("kastellan-cli");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    Invocation {
        program: path.to_string_lossy().into_owned(),
        args: vec!["session".into()],
    }
}

/// The opening every scripted session shares: check the submit names the instruction, assign task 42, accept the
/// follow. Exits 8 or 9 when Bottie sent something else, which the test then sees as a failure.
const OPENING: &str = r#"
read submit
case "$submit" in *'"task.submit"'*'"instruction":"find the invoice"'* | *'"instruction":"find the invoice"'*'"task.submit"'*) ;; *) echo "bad submit: $submit" >&2; exit 8;; esac
echo '{"jsonrpc":"2.0","id":1,"result":{"task_id":42,"lane":"fast"}}'
read follow
case "$follow" in *'"task.follow"'*) ;; *) echo "bad follow: $follow" >&2; exit 9;; esac
echo '{"jsonrpc":"2.0","id":2,"result":{"task_id":42,"after_seq":0}}'
"#;

const STEPS: &str = r#"echo '{"jsonrpc":"2.0","method":"notebook.entry","params":{"task_id":42,"seq":1,"kind":"steps","payload":{"steps":[{"id":1,"purpose":"find it"}]}}}'"#;

async fn run(
    invocation: &Invocation,
    controls: Vec<Control>,
) -> (TaskOutcome, Option<String>, Vec<String>) {
    let (sender, receiver) = mpsc::unbounded_channel();
    for control in controls {
        sender.send(control).unwrap();
    }
    let progress = Arc::new(Mutex::new(Vec::new()));
    let sink = progress.clone();
    let (outcome, task_id) = run_session(
        invocation,
        "find the invoice",
        Duration::from_secs(10),
        receiver,
        move |line| sink.lock().unwrap().push(line),
    )
    .await;
    drop(sender);
    let lines = progress.lock().unwrap().clone();
    (outcome, task_id, lines)
}

#[tokio::test]
async fn a_task_streams_its_progress_then_saves_its_answer() {
    let cli = fake_cli(&format!(
        r#"{OPENING}
{STEPS}
echo '{{"jsonrpc":"2.0","method":"notebook.entry","params":{{"task_id":42,"seq":2,"kind":"tick","payload":{{"step":1,"note":"found"}}}}}}'
echo '{{"jsonrpc":"2.0","method":"task.finished","params":{{"task_id":42,"state":"completed","answer":"invoice 42"}}}}'
read nothing"#
    ));
    let (outcome, task_id, progress) = run(&cli, vec![]).await;
    assert_eq!(outcome, TaskOutcome::Reply("invoice 42".into()));
    assert_eq!(task_id.as_deref(), Some("42"));
    assert_eq!(progress, ["Plan: 1. find it", "Step 1 done: found"]);
}

#[tokio::test]
async fn stop_cancels_the_followed_task_and_saves_it_as_stopped() {
    // Stop is pressed before the task id is known: it must wait for the id, then cancel exactly that task.
    let cli = fake_cli(&format!(
        r#"{OPENING}
read control
case "$control" in *'"task.cancel"'*'42'*) ;; *) echo "bad control: $control" >&2; exit 7;; esac
echo '{{"jsonrpc":"2.0","id":3,"result":{{"state":"cancelled","audited":true}}}}'
echo '{{"jsonrpc":"2.0","method":"task.finished","params":{{"task_id":42,"state":"cancelled","answer":null}}}}'
read nothing"#
    ));
    let (outcome, _, progress) = run(&cli, vec![Control::Stop]).await;
    assert!(matches!(outcome, TaskOutcome::Stopped(_)), "{outcome:?}");
    assert_eq!(progress, ["Stopping…"]);
}

#[tokio::test]
async fn a_correction_reaches_the_task_and_a_refusal_is_shown_not_fatal() {
    let cli = fake_cli(&format!(
        r#"{OPENING}
read control
case "$control" in *'"task.correct"'*'the other account'*) ;; *) echo "bad control: $control" >&2; exit 7;; esac
echo '{{"jsonrpc":"2.0","id":3,"error":{{"code":-32002,"message":"task 42 is not running"}}}}'
echo '{{"jsonrpc":"2.0","method":"task.finished","params":{{"task_id":42,"state":"completed","answer":"done"}}}}'
read nothing"#
    ));
    let (outcome, _, progress) =
        run(&cli, vec![Control::Correct("the other account".into())]).await;
    assert_eq!(outcome, TaskOutcome::Reply("done".into()));
    assert_eq!(progress, ["Refused: task 42 is not running"]);
}

#[tokio::test]
async fn a_parked_task_says_where_its_question_waits() {
    let cli = fake_cli(&format!(
        r#"{OPENING}
echo '{{"jsonrpc":"2.0","method":"task.awaiting_operator","params":{{"task_id":42}}}}'
read nothing"#
    ));
    let (outcome, _, _) = run(&cli, vec![]).await;
    let TaskOutcome::Reply(text) = outcome else {
        panic!("a parked task is not a failure: {outcome:?}")
    };
    assert!(
        text.contains("paused task 42") && text.contains("kastellan-cli inbox list"),
        "{text}"
    );
}

#[tokio::test]
async fn a_refused_submit_and_a_lost_follow_are_failures_with_kastellans_reason() {
    let refused = fake_cli(
        r#"read submit; echo '{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"instruction is empty"}}'; read x"#,
    );
    let (outcome, task_id, _) = run(&refused, vec![]).await;
    assert!(
        matches!(&outcome, TaskOutcome::Failed { text, .. } if text.contains("instruction is empty")),
        "{outcome:?}"
    );
    assert_eq!(task_id, None);

    let lost = fake_cli(&format!(
        r#"{OPENING}
echo '{{"jsonrpc":"2.0","method":"task.follow_failed","params":{{"task_id":42,"last_seq":0,"error":"listener: gone"}}}}'
read nothing"#
    ));
    let (outcome, task_id, _) = run(&lost, vec![]).await;
    let TaskOutcome::Failed { text, code } = outcome else {
        panic!("a lost follow is a failure")
    };
    assert_eq!(code, "unavailable");
    assert!(
        text.contains("task 42") && text.contains("listener: gone"),
        "{text}"
    );
    assert_eq!(task_id.as_deref(), Some("42"));
}

#[tokio::test]
async fn a_cli_without_sessions_fails_with_its_own_error() {
    // An older Kastellan has no `session` subcommand.
    let old = fake_cli(r#"echo "unknown command: session" >&2; exit 2"#);
    let (outcome, _, _) = run(&old, vec![]).await;
    let TaskOutcome::Failed { text, code } = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(code, "server");
    assert!(text.contains("unknown command: session"), "{text}");
}

#[tokio::test]
async fn a_child_that_exits_without_reading_keeps_its_own_error() {
    // ssh failing authentication exits before reading stdin; its stderr must survive the failed write.
    let (outcome, _, _) = run(
        &fake_cli("echo 'Permission denied (publickey).' >&2; exit 255"),
        vec![],
    )
    .await;
    assert!(
        matches!(&outcome, TaskOutcome::Failed { text, .. } if text.contains("Permission denied")),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn a_missing_and_a_hung_cli_both_fail_promptly() {
    let missing = Invocation {
        program: "/nonexistent/kastellan-cli".into(),
        args: vec![],
    };
    assert!(matches!(
        run(&missing, vec![]).await.0,
        TaskOutcome::Failed {
            code: "unavailable",
            ..
        }
    ));

    let hung = fake_cli("sleep 30");
    let (sender, receiver) = mpsc::unbounded_channel::<Control>();
    let started = Instant::now();
    let (outcome, _) = run_session(&hung, "x", Duration::from_millis(300), receiver, |_| {}).await;
    drop(sender);
    assert!(
        matches!(
            outcome,
            TaskOutcome::Failed {
                code: "timeout",
                ..
            }
        ),
        "{outcome:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the timeout must not wait for the child"
    );
}
