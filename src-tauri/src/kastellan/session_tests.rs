//! Session protocol tests: the request lines Bottie writes and how each message Kastellan sends is read.

use serde_json::{Value, json};

use super::*;

fn line_json(line: &str) -> Value {
    assert!(line.ends_with('\n'), "one request per line: {line:?}");
    serde_json::from_str(line.trim_end()).unwrap()
}

#[test]
fn a_session_submits_then_follows_the_task_it_got() {
    assert_eq!(
        line_json(&submit_line("it's \"quoted\"\nand two lines")),
        json!({"jsonrpc": "2.0", "id": SUBMIT_ID, "method": "task.submit",
               "params": {"instruction": "it's \"quoted\"\nand two lines"}})
    );
    assert_eq!(
        line_json(&follow_line(42)),
        json!({"jsonrpc": "2.0", "id": FOLLOW_ID, "method": "task.follow", "params": {"task_id": 42}})
    );
}

#[test]
fn stop_cancels_and_correct_corrects_the_followed_task() {
    assert_eq!(
        line_json(&control_line(3, 42, &Control::Stop)),
        json!({"jsonrpc": "2.0", "id": 3, "method": "task.cancel", "params": {"task_id": 42}})
    );
    assert_eq!(
        line_json(&control_line(
            4,
            42,
            &Control::Correct("the other account".into())
        )),
        json!({"jsonrpc": "2.0", "id": 4, "method": "task.correct",
               "params": {"task_id": 42, "text": "the other account"}})
    );
}

#[test]
fn responses_carry_their_result_or_their_error() {
    assert_eq!(
        parse_message(r#"{"jsonrpc":"2.0","id":1,"result":{"task_id":7,"lane":"fast"}}"#),
        SessionMessage::Response {
            id: 1,
            result: Ok(json!({"task_id": 7, "lane": "fast"}))
        }
    );
    assert_eq!(
        parse_message(
            r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32002,"message":"task 7 is not running"}}"#
        ),
        SessionMessage::Response {
            id: 3,
            result: Err("task 7 is not running".into())
        }
    );
}

#[test]
fn notebook_entries_become_progress_lines() {
    let steps = json!({"jsonrpc": "2.0", "method": "notebook.entry", "params": {
        "task_id": 7, "seq": 1, "kind": "steps",
        "payload": {"steps": [{"id": 1, "purpose": "find it"}, {"id": 2, "purpose": "read it"}]}}});
    assert_eq!(
        parse_message(&steps.to_string()),
        SessionMessage::Progress(Some("Plan: 1. find it; 2. read it".into()))
    );
    let answer = json!({"jsonrpc": "2.0", "method": "notebook.entry",
                        "params": {"kind": "answer", "payload": {"body": "42"}}});
    assert_eq!(
        parse_message(&answer.to_string()),
        SessionMessage::Progress(None),
        "the reply shows the answer"
    );
}

#[test]
fn every_progress_kind_says_what_happened() {
    let tick = json!({"step": 2, "note": "total is 40"});
    assert_eq!(
        entry_line("tick", &tick).unwrap(),
        "Step 2 done: total is 40"
    );
    let partial = json!({"step": 2, "note": "page 1", "partial": true});
    assert_eq!(
        entry_line("tick", &partial).unwrap(),
        "Step 2 progress: page 1"
    );
    let failed = json!({"step": 1, "code": "denied", "detail": "not allowed"});
    assert_eq!(
        entry_line("failed_step", &failed).unwrap(),
        "Step 1 failed (denied): not allowed"
    );
    assert_eq!(
        entry_line("correction", &json!({"text": "x"})).unwrap(),
        "Correction: x"
    );
    assert_eq!(
        entry_line("infeasible", &json!({"reason": "no mailbox"})).unwrap(),
        "Cannot be done: no mailbox"
    );
    assert_eq!(entry_line("cancelled", &json!({})), None);
}

#[test]
fn model_text_is_one_bounded_line() {
    let line = entry_line("correction", &json!({"text": "a\u{1b}[2Jb\nc"})).unwrap();
    assert_eq!(line, "Correction: a [2Jb c");
    let long = "x".repeat(PROGRESS_TEXT_CAP + 1);
    let line = entry_line("correction", &json!({"text": long})).unwrap();
    assert_eq!(
        line,
        format!("Correction: {}…", "x".repeat(PROGRESS_TEXT_CAP))
    );
}

#[test]
fn a_finished_task_becomes_its_saved_outcome() {
    let done = json!({"state": "completed", "answer": "invoice 42", "detail": null, "result": {}});
    assert_eq!(
        finished_outcome(&done),
        TaskOutcome::Reply("invoice 42".into())
    );
    let other_kind = json!({"state": "completed", "answer": null, "result": {"kind": "table"}});
    let TaskOutcome::Reply(text) = finished_outcome(&other_kind) else {
        panic!("a completed task is a reply")
    };
    assert!(text.contains("\"table\""), "{text}");
    let stopped = json!({"state": "cancelled", "answer": null, "detail": null});
    assert!(matches!(
        finished_outcome(&stopped),
        TaskOutcome::Stopped(_)
    ));
    let failed = json!({"state": "failed", "answer": null, "detail": "not a plan"});
    assert_eq!(
        finished_outcome(&failed),
        TaskOutcome::Failed {
            text: "Kastellan could not complete the task (failed).\n\nnot a plan".into(),
            code: "server"
        }
    );
}

#[test]
fn the_end_of_a_follow_is_recognised() {
    let finished = json!({"jsonrpc": "2.0", "method": "task.finished",
                          "params": {"task_id": 7, "state": "completed", "answer": "ok"}});
    assert_eq!(
        parse_message(&finished.to_string()),
        SessionMessage::Finished(TaskOutcome::Reply("ok".into()))
    );
    let parked =
        json!({"jsonrpc": "2.0", "method": "task.awaiting_operator", "params": {"task_id": 7}});
    assert_eq!(parse_message(&parked.to_string()), SessionMessage::Parked);
    let lost = json!({"jsonrpc": "2.0", "method": "task.follow_failed",
                      "params": {"task_id": 7, "last_seq": 2, "error": "listener: gone"}});
    assert_eq!(
        parse_message(&lost.to_string()),
        SessionMessage::FollowFailed("listener: gone".into())
    );
}

#[test]
fn noise_is_ignored() {
    assert_eq!(parse_message("not json"), SessionMessage::Ignored);
    assert_eq!(
        parse_message(r#"{"jsonrpc":"2.0","method":"something.new","params":{}}"#),
        SessionMessage::Ignored
    );
}
