//! Provider-run checkpoint and interruption-recovery tests.

use std::fs;

use super::*;

/// Creates an isolated database path for one provider-run storage test.
fn test_database_path() -> std::path::PathBuf {
    let directory =
        std::env::temp_dir().join(format!("bottie-run-storage-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).expect("test directory should be created");
    directory.join("bottie.sqlite3")
}

/// Creates one conversation with a persisted user request.
fn stored_request(store: &ConversationStore) -> (StoredConversation, StoredMessage) {
    let conversation = store
        .create_conversation("Checkpointed generation")
        .expect("conversation should be created");
    let request = store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "Keep the partial response".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("request should be stored");
    (conversation, request)
}

/// Starts one test provider run and its native-owned assistant checkpoint.
fn start_run(
    store: &ConversationStore,
    conversation: &StoredConversation,
    request: &StoredMessage,
) -> String {
    let run_id = uuid::Uuid::new_v4().to_string();
    store
        .start_provider_run(NewProviderRun {
            id: run_id.clone(),
            conversation_id: conversation.id.clone(),
            request_message_id: request.id.clone(),
            provider_id: "ollama".into(),
            model_id: "qwen3:latest".into(),
            reasoning_effort: StoredReasoningEffort::Low,
            temperature: None,
            max_output_tokens: Some(1_024),
        })
        .expect("provider run should start");
    run_id
}

#[test]
fn checkpoints_partial_blocks_and_finishes_the_native_owned_response() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let (conversation, request) = stored_request(&store);
    let run_id = start_run(&store, &conversation, &request);

    store
        .checkpoint_provider_delta(&run_id, RunBlockKind::Reasoning, "Check ")
        .expect("reasoning checkpoint should persist");
    store
        .checkpoint_provider_delta(&run_id, RunBlockKind::Reasoning, "carefully.")
        .expect("reasoning checkpoint should append");
    store
        .checkpoint_provider_delta(&run_id, RunBlockKind::Text, "Saved ")
        .expect("text checkpoint should persist");
    store
        .checkpoint_provider_delta(&run_id, RunBlockKind::Text, "exactly.\n")
        .expect("text checkpoint should preserve whitespace");
    store
        .checkpoint_provider_usage(
            &run_id,
            StoredUsage {
                input_tokens: Some(12),
                output_tokens: Some(7),
                cost_usd: None,
            },
        )
        .expect("usage checkpoint should persist");
    store
        .finish_provider_run(&run_id, ProviderRunState::Completed, None, None)
        .expect("provider run should complete");

    let reopened = ConversationStore::initialize(path)
        .expect("storage should reopen")
        .load_conversation(&conversation.id)
        .expect("conversation should load");
    let response = &reopened.messages[1];
    let run = response
        .provider_run
        .as_ref()
        .expect("response should retain provider provenance");

    assert_eq!(response.state, MessageState::Final);
    assert_eq!(response.text, "Saved exactly.\n");
    assert_eq!(response.reasoning.as_deref(), Some("Check carefully."));
    assert_eq!(run.state, ProviderRunState::Completed);
    assert_eq!(run.error_code, None);
    assert_eq!(
        run.usage.as_ref().and_then(|usage| usage.output_tokens),
        Some(7)
    );
}

#[test]
fn reopens_running_generations_as_interrupted_partial_responses() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let (conversation, request) = stored_request(&store);
    let run_id = start_run(&store, &conversation, &request);
    store
        .checkpoint_provider_delta(&run_id, RunBlockKind::Text, "Recovered words")
        .expect("partial text should persist");
    drop(store);

    let reopened_store = ConversationStore::initialize(path).expect("storage should recover");
    let reopened = reopened_store
        .load_conversation(&conversation.id)
        .expect("conversation should load");
    let response = &reopened.messages[1];
    let run = response
        .provider_run
        .as_ref()
        .expect("partial response should retain its run");

    assert_eq!(response.state, MessageState::Partial);
    assert_eq!(response.text, "Recovered words");
    assert_eq!(run.state, ProviderRunState::Failed);
    assert_eq!(run.error_code.as_deref(), Some("interrupted"));
    assert!(run.completed_at_ms.is_some());

    reopened_store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id,
            role: StoredRole::User,
            text: "Continue after recovery".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("recovered conversation should accept another prompt");
}

#[test]
fn recovers_a_pre_checkpoint_running_record_with_an_empty_partial_response() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let (conversation, request) = stored_request(&store);
    let run_id = start_run(&store, &conversation, &request);
    store
        .open()
        .expect("test connection should open")
        .execute("DELETE FROM messages WHERE provider_run_id = ?1", [&run_id])
        .expect("test should simulate a version-three running record");
    drop(store);

    let reopened = ConversationStore::initialize(path)
        .expect("storage should recover")
        .load_conversation(&conversation.id)
        .expect("conversation should load");
    let response = &reopened.messages[1];

    assert_eq!(response.state, MessageState::Partial);
    assert_eq!(response.text, "");
    assert_eq!(
        response
            .provider_run
            .as_ref()
            .and_then(|run| run.error_code.as_deref()),
        Some("interrupted")
    );
}

#[test]
fn rejects_checkpoints_after_a_run_reaches_terminal_state() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let (conversation, request) = stored_request(&store);
    let run_id = start_run(&store, &conversation, &request);
    store
        .finish_provider_run(&run_id, ProviderRunState::Cancelled, None, None)
        .expect("provider run should cancel");

    let error = store
        .checkpoint_provider_delta(&run_id, RunBlockKind::Text, "too late")
        .expect_err("terminal run should reject checkpoints");

    assert_eq!(error.code, "not_found");
}

#[test]
fn rejects_a_new_user_message_while_native_provider_work_is_running() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let (conversation, request) = stored_request(&store);
    start_run(&store, &conversation, &request);

    let error = store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id,
            role: StoredRole::User,
            text: "Do not race the active response".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect_err("active provider work should serialize user messages");

    assert_eq!(error.code, "invalid_request");
}
