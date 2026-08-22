//! Response-rating storage contract tests.

use super::*;

/// Appends one final message used by response-rating tests.
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
fn persists_changes_and_clears_one_assistant_response_rating() {
    let path = tests::test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Durable response rating")
        .expect("conversation should be created");
    append_final(
        &store,
        &conversation.id,
        StoredRole::User,
        "Was this useful?",
    );
    let response = append_final(
        &store,
        &conversation.id,
        StoredRole::Assistant,
        "A useful response.",
    );

    assert_eq!(
        store
            .rate_response(&conversation.id, &response.id, Some(ResponseRating::Good))
            .expect("good rating should persist"),
        Some(ResponseRating::Good)
    );
    assert_eq!(
        store
            .rate_response(&conversation.id, &response.id, Some(ResponseRating::Poor))
            .expect("poor rating should replace good"),
        Some(ResponseRating::Poor)
    );
    drop(store);

    let store = ConversationStore::initialize(path).expect("storage should reopen");
    assert_eq!(
        store
            .status()
            .expect("storage status should load")
            .schema_version,
        20
    );
    let reopened = store
        .load_conversation(&conversation.id)
        .expect("conversation should reopen");
    assert_eq!(reopened.messages[1].rating, Some(ResponseRating::Poor));
    assert_eq!(
        store
            .rate_response(&conversation.id, &response.id, None)
            .expect("rating should clear"),
        None
    );
    assert_eq!(
        store
            .load_conversation(&conversation.id)
            .expect("conversation should reload")
            .messages[1]
            .rating,
        None
    );
}

#[test]
fn rejects_ratings_for_user_foreign_and_deleted_conversation_messages() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let first = store
        .create_conversation("First conversation")
        .expect("first conversation should be created");
    let user = append_final(&store, &first.id, StoredRole::User, "User prompt");
    let response = append_final(
        &store,
        &first.id,
        StoredRole::Assistant,
        "Assistant response",
    );
    let second = store
        .create_conversation("Second conversation")
        .expect("second conversation should be created");

    let user_error = store
        .rate_response(&first.id, &user.id, Some(ResponseRating::Good))
        .expect_err("user messages must not be rateable");
    let foreign_error = store
        .rate_response(&second.id, &response.id, Some(ResponseRating::Good))
        .expect_err("foreign messages must not be rateable");
    store
        .fork_from_user_message(&first.id, &user.id, "Alternative prompt")
        .expect("an alternative branch should be selected");
    let hidden_error = store
        .rate_response(&first.id, &response.id, Some(ResponseRating::Good))
        .expect_err("hidden sibling responses must not be rateable");
    store
        .delete_conversation(&first.id)
        .expect("first conversation should move to trash");
    let deleted_error = store
        .rate_response(&first.id, &response.id, Some(ResponseRating::Good))
        .expect_err("deleted conversation responses must not be rateable");

    assert_eq!(user_error.code, "invalid_request");
    assert_eq!(foreign_error.code, "not_found");
    assert_eq!(hidden_error.code, "not_found");
    assert_eq!(deleted_error.code, "not_found");
}
