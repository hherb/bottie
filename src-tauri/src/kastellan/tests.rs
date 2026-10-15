//! Kastellan run persistence tests: where a reply lands and how a pending task holds its conversation.

use super::*;
use crate::storage::{MessageState, NewStoredMessage, StoredRole};
use transport::TaskOutcome;

/// Creates a store with one conversation whose latest message is a user request.
fn store_with_request() -> (ConversationStore, String, String) {
    let directory =
        std::env::temp_dir().join(format!("bottie-kastellan-store-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let store = ConversationStore::initialize(directory.join("bottie.sqlite3")).unwrap();
    let conversation = store.create_conversation("Kastellan").unwrap();
    let request = store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation.id.clone(),
                role: StoredRole::User,
                text: "@kastellan: do it".into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[],
        )
        .unwrap();
    (store, conversation.id, request.id)
}

fn start_run(store: &ConversationStore, conversation_id: &str, request_id: &str) -> String {
    let run_id = uuid::Uuid::new_v4().to_string();
    store
        .start_provider_run(NewProviderRun {
            id: run_id.clone(),
            conversation_id: conversation_id.into(),
            request_message_id: request_id.into(),
            provider_id: KASTELLAN_PROVIDER_ID.into(),
            model_id: "localhost".into(),
            reasoning_effort: StoredReasoningEffort::Off,
            temperature: None,
            max_output_tokens: None,
        })
        .unwrap();
    run_id
}

#[test]
fn a_reply_becomes_the_final_response_to_its_request_with_provenance() {
    let (store, conversation_id, request_id) = store_with_request();
    let run_id = start_run(&store, &conversation_id, &request_id);
    persist_outcome(
        &store,
        &run_id,
        &TaskOutcome::Reply("done".into()),
        "Kastellan task 9 on localhost · 3s",
    )
    .unwrap();
    let messages = store.load_conversation(&conversation_id).unwrap().messages;
    let reply = messages.last().unwrap();
    assert_eq!(reply.text, "done");
    assert_eq!(
        reply.reasoning.as_deref(),
        Some("Kastellan task 9 on localhost · 3s")
    );
    assert_eq!(reply.state, MessageState::Final);
    assert_eq!(messages[messages.len() - 2].id, request_id);
}

#[test]
fn a_failure_keeps_its_explanation_and_code() {
    let (store, conversation_id, request_id) = store_with_request();
    let run_id = start_run(&store, &conversation_id, &request_id);
    let outcome = TaskOutcome::Failed {
        text: "Could not reach the Kastellan host over ssh.".into(),
        code: "unavailable",
    };
    persist_outcome(
        &store,
        &run_id,
        &outcome,
        "Kastellan task (id unknown) on dgx · 0s",
    )
    .unwrap();
    let reply = store
        .load_conversation(&conversation_id)
        .unwrap()
        .messages
        .pop()
        .unwrap();
    assert_eq!(reply.state, MessageState::Failed);
    assert!(reply.text.contains("over ssh"));
    assert_eq!(
        reply.provider_run.unwrap().error_code.as_deref(),
        Some("unavailable")
    );
}

/// Appends one user message, as the composer does.
fn send(
    store: &ConversationStore,
    conversation_id: &str,
    text: &str,
) -> Result<(), crate::storage::StorageError> {
    let message = NewStoredMessage {
        conversation_id: conversation_id.into(),
        role: StoredRole::User,
        text: text.into(),
        reasoning: None,
        state: MessageState::Final,
        provider_id: None,
        model_id: None,
    };
    store
        .append_message_with_attachments(message, &[])
        .map(|_| ())
}

#[test]
fn a_pending_task_holds_its_own_conversation_until_it_answers() {
    let (store, conversation_id, request_id) = store_with_request();
    let run_id = start_run(&store, &conversation_id, &request_id);
    // Bottie keeps at most one running response per conversation; Kastellan inherits that.
    let busy = send(&store, &conversation_id, "meanwhile").unwrap_err();
    assert_eq!(busy.code, "invalid_request");
    let other = store.create_conversation("Elsewhere").unwrap();
    send(&store, &other.id, "chatting elsewhere").expect("other conversations stay usable");

    persist_outcome(&store, &run_id, &TaskOutcome::Reply("late".into()), "p").unwrap();
    send(&store, &conversation_id, "thanks").expect("the reply frees the conversation");
    let texts: Vec<_> = store
        .load_conversation(&conversation_id)
        .unwrap()
        .messages
        .into_iter()
        .map(|message| message.text)
        .collect();
    assert_eq!(texts, ["@kastellan: do it", "late", "thanks"]);
}

#[test]
fn provenance_names_the_task_host_and_duration() {
    assert_eq!(
        provenance(Some("204"), "dgx", Duration::from_millis(34_600)),
        "Kastellan task 204 on dgx · 34s"
    );
    assert_eq!(
        provenance(None, "localhost", Duration::ZERO),
        "Kastellan task (id unknown) on localhost · 0s"
    );
}

#[test]
fn a_rejected_task_is_saved_as_a_failed_response_the_user_can_regenerate() {
    let (store, conversation_id, request_id) = store_with_request();
    let run_id = start_run(&store, &conversation_id, &request_id);
    let error = prepare(
        &config::KastellanConfig::default(),
        "@kastellan: --long x",
        None,
    )
    .unwrap_err();
    let outcome = TaskOutcome::Failed {
        text: error.message,
        code: error.code.as_str(),
    };
    persist_outcome(
        &store,
        &run_id,
        &outcome,
        &provenance(None, "localhost", Duration::ZERO),
    )
    .unwrap();
    let reply = store
        .load_conversation(&conversation_id)
        .unwrap()
        .messages
        .pop()
        .unwrap();
    assert_eq!(reply.state, MessageState::Failed);
    assert!(
        reply.text.contains("cannot start with '-'"),
        "{}",
        reply.text
    );
    send(&store, &conversation_id, "@kastellan: x").expect("the failure frees the conversation");
}

/// A store whose text checkpoint fails, recording every call.
#[derive(Default)]
struct FailingTextStore {
    calls: std::cell::RefCell<Vec<String>>,
}

impl RunStore for FailingTextStore {
    fn checkpoint(&self, _: &str, kind: RunBlockKind, _: &str) -> Result<(), StorageError> {
        let text = matches!(kind, RunBlockKind::Text);
        self.calls.borrow_mut().push(
            if text {
                "checkpoint text"
            } else {
                "checkpoint reasoning"
            }
            .into(),
        );
        if text {
            Err(StorageError::internal())
        } else {
            Ok(())
        }
    }

    fn finish(
        &self,
        _: &str,
        state: ProviderRunState,
        code: Option<&str>,
    ) -> Result<(), StorageError> {
        let failed = matches!(state, ProviderRunState::Failed);
        self.calls
            .borrow_mut()
            .push(format!("finish failed={failed} {code:?}"));
        Ok(())
    }
}

#[test]
fn a_failed_save_still_closes_the_run_so_the_conversation_is_not_held() {
    let store = FailingTextStore::default();
    assert!(persist_outcome(&store, "run", &TaskOutcome::Reply("done".into()), "p").is_err());
    assert_eq!(
        store.calls.into_inner(),
        [
            "checkpoint reasoning",
            "checkpoint text",
            "finish failed=true Some(\"internal\")"
        ]
    );
}
