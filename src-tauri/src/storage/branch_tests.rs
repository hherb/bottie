//! Edit-and-regenerate branch storage tests.

use rusqlite::params;

use super::tests::test_database_path;
use super::*;

/// Appends one final message through the internal test boundary.
fn append_final(
    store: &ConversationStore,
    conversation_id: &str,
    role: StoredRole,
    text: &str,
) -> StoredMessage {
    store
        .append_message(NewStoredMessage {
            conversation_id: conversation_id.into(),
            role,
            text: text.into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("message should append")
}

#[test]
fn forks_an_edited_user_message_without_rewriting_the_original_branch() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Branching")
        .expect("conversation should be created");
    append_final(&store, &conversation.id, StoredRole::User, "First question");
    append_final(
        &store,
        &conversation.id,
        StoredRole::Assistant,
        "First answer",
    );
    let original_request = append_final(
        &store,
        &conversation.id,
        StoredRole::User,
        "Original follow-up",
    );
    append_final(
        &store,
        &conversation.id,
        StoredRole::Assistant,
        "Original follow-up answer",
    );
    let connection = store.open().expect("database should open");
    let main_branch_id: String = connection
        .query_row(
            "SELECT current_branch_id FROM conversations WHERE id = ?1",
            [&conversation.id],
            |row| row.get(0),
        )
        .expect("main branch should be selected");
    drop(connection);

    let forked = store
        .fork_from_user_message(&conversation.id, &original_request.id, "Edited follow-up")
        .expect("visible user request should fork");

    assert_eq!(
        forked.request_message_id,
        forked.conversation.messages[2].id
    );
    assert_eq!(forked.conversation.messages.len(), 3);
    assert_eq!(forked.conversation.messages[0].text, "First question");
    assert_eq!(forked.conversation.messages[1].text, "First answer");
    assert_eq!(forked.conversation.messages[2].text, "Edited follow-up");
    assert_eq!(forked.conversation.branches.len(), 2);
    assert_ne!(forked.conversation.current_branch_id, main_branch_id);

    let run_id = uuid::Uuid::new_v4().to_string();
    store
        .start_provider_run(NewProviderRun {
            id: run_id.clone(),
            conversation_id: conversation.id.clone(),
            request_message_id: forked.request_message_id,
            provider_id: "ollama".into(),
            model_id: "branch-model".into(),
            reasoning_effort: StoredReasoningEffort::Off,
            temperature: None,
            max_output_tokens: Some(256),
        })
        .expect("the forked request should start generation");
    let active_error = store
        .select_branch(&conversation.id, &main_branch_id)
        .expect_err("an active run should prevent branch switching");
    assert_eq!(active_error.code, "invalid_request");
    store
        .checkpoint_provider_delta(&run_id, RunBlockKind::Text, "Edited answer")
        .expect("the alternative response should checkpoint");
    store
        .finish_provider_run(&run_id, ProviderRunState::Completed, None, None)
        .expect("the alternative response should complete");
    drop(store);
    let store = ConversationStore::initialize(path).expect("branched storage should reopen");
    let alternative = store
        .load_conversation(&conversation.id)
        .expect("selected alternative should load");
    assert_eq!(alternative.messages.len(), 4);
    assert_eq!(alternative.messages[3].text, "Edited answer");

    let original = store
        .select_branch(&conversation.id, &main_branch_id)
        .expect("original branch should remain selectable");
    assert_eq!(original.messages.len(), 4);
    assert_eq!(original.messages[2].text, "Original follow-up");
    assert_eq!(original.messages[3].text, "Original follow-up answer");
}

#[test]
fn forks_a_retry_without_rewriting_the_failed_response() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Response retry")
        .expect("conversation should be created");
    let request = append_final(
        &store,
        &conversation.id,
        StoredRole::User,
        "Try the provider",
    );
    let failed_run_id = uuid::Uuid::new_v4().to_string();
    store
        .start_provider_run(NewProviderRun {
            id: failed_run_id.clone(),
            conversation_id: conversation.id.clone(),
            request_message_id: request.id.clone(),
            provider_id: "ollama".into(),
            model_id: "retry-model".into(),
            reasoning_effort: StoredReasoningEffort::Off,
            temperature: None,
            max_output_tokens: Some(256),
        })
        .expect("the original run should start");
    store
        .checkpoint_provider_delta(&failed_run_id, RunBlockKind::Text, "Saved partial")
        .expect("the partial response should checkpoint");
    store
        .finish_provider_run(
            &failed_run_id,
            ProviderRunState::Failed,
            Some("timeout"),
            None,
        )
        .expect("the original run should fail durably");

    let retried = store
        .fork_from_user_message(&conversation.id, &request.id, &request.text)
        .expect("the failed request should fork for retry");

    assert_eq!(retried.conversation.messages.len(), 1);
    assert_eq!(retried.conversation.messages[0].text, "Try the provider");
    let original_branch = retried
        .conversation
        .branches
        .first()
        .expect("the original branch should remain available");
    let original = store
        .select_branch(&conversation.id, &original_branch.id)
        .expect("the original failed attempt should remain selectable");
    assert_eq!(original.messages.len(), 2);
    assert_eq!(original.messages[1].text, "Saved partial");
    assert_eq!(original.messages[1].state, MessageState::Failed);
}

#[test]
fn rejects_branching_from_a_message_outside_the_selected_ancestry() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Branch validation")
        .expect("conversation should be created");
    let original = append_final(&store, &conversation.id, StoredRole::User, "Original");
    append_final(
        &store,
        &conversation.id,
        StoredRole::Assistant,
        "Original answer",
    );
    store
        .fork_from_user_message(&conversation.id, &original.id, "Alternative")
        .expect("first fork should succeed");

    let error = store
        .fork_from_user_message(&conversation.id, &original.id, "Hidden rewrite")
        .expect_err("a hidden sibling request must not be branchable");

    assert_eq!(error.code, "invalid_request");
    assert_eq!(
        error.message,
        "Only a user message on the selected branch can be edited or regenerated."
    );
}

#[test]
fn version_four_stores_gain_a_selected_main_branch() {
    let path = test_database_path();
    let connection = rusqlite::Connection::open(&path).expect("version four database should open");
    connection
        .execute_batch(MIGRATION_1)
        .expect("foundation migration should apply");
    connection
        .execute(
            "INSERT INTO profiles (id, name, created_at_ms) VALUES (?1, ?2, 1)",
            params![DEFAULT_PROFILE_ID, DEFAULT_PROFILE_NAME],
        )
        .expect("default profile should be inserted");
    connection
        .execute_batch(MIGRATION_2)
        .expect("message order migration should apply");
    connection
        .execute_batch(MIGRATION_3)
        .expect("provider run migration should apply");
    connection
        .execute_batch(MIGRATION_4)
        .expect("last-open migration should apply");
    connection
        .execute(
            "INSERT INTO conversations (id, profile_id, title, created_at_ms, updated_at_ms)
             VALUES ('conversation', ?1, 'Existing', 2, 2)",
            [DEFAULT_PROFILE_ID],
        )
        .expect("conversation should be inserted");
    connection
        .execute(
            "INSERT INTO branches (id, conversation_id, name, created_at_ms)
             VALUES ('main-branch', 'conversation', 'Main', 2)",
            [],
        )
        .expect("main branch should be inserted");
    connection
        .pragma_update(None, "user_version", 4)
        .expect("version should be set");
    drop(connection);

    let store = ConversationStore::initialize(path).expect("version four store should upgrade");
    let loaded = store
        .load_conversation("conversation")
        .expect("upgraded conversation should load");

    assert_eq!(
        store.status().expect("status should load").schema_version,
        6
    );
    assert_eq!(loaded.current_branch_id, "main-branch");
    assert_eq!(loaded.branches.len(), 1);
}
