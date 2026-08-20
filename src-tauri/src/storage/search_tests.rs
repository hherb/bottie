//! Conversation-search contract tests.

use super::*;

/// Appends one final message used as searchable fixture content.
fn append_search_message(
    store: &ConversationStore,
    conversation_id: &str,
    role: StoredRole,
    text: &str,
    reasoning: Option<&str>,
) -> StoredMessage {
    store
        .append_message(NewStoredMessage {
            conversation_id: conversation_id.into(),
            role,
            text: text.into(),
            reasoning: reasoning.map(str::to_owned),
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("search fixture message should be stored")
}

#[test]
fn searches_titles_and_visible_text_with_literal_case_insensitive_queries() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let active = store
        .create_conversation("Rust-owned storage")
        .expect("active conversation should be created");
    append_search_message(
        &store,
        &active.id,
        StoredRole::Assistant,
        "Literal 100% coverage",
        Some("private reasoning marker"),
    );
    append_search_message(
        &store,
        &active.id,
        StoredRole::User,
        &format!("{} Unicode needle", "İ".repeat(100)),
        None,
    );
    let archived = store
        .create_conversation("Archived notes")
        .expect("archived conversation should be created");
    append_search_message(
        &store,
        &archived.id,
        StoredRole::User,
        "A durable search needle",
        None,
    );
    store
        .set_conversation_archived(&archived.id, true)
        .expect("conversation should archive");
    let deleted = store
        .create_conversation("Deleted search needle")
        .expect("deleted conversation should be created");
    store
        .delete_conversation(&deleted.id)
        .expect("conversation should move to trash");

    let title_results = store
        .search_conversations("RUST-OWNED")
        .expect("title search should succeed");
    let literal_results = store
        .search_conversations("100%")
        .expect("literal search should succeed");
    let archived_results = store
        .search_conversations("durable search needle")
        .expect("archived search should succeed");
    let reasoning_results = store
        .search_conversations("private reasoning marker")
        .expect("reasoning search should succeed");
    let unicode_results = store
        .search_conversations("unicode needle")
        .expect("Unicode-prefix search should succeed");

    assert_eq!(title_results.len(), 1);
    assert_eq!(title_results[0].conversation_id, active.id);
    assert_eq!(title_results[0].snippet, "Rust-owned storage");
    assert_eq!(literal_results.len(), 1);
    assert!(literal_results[0].snippet.contains("100%"));
    assert_eq!(archived_results.len(), 1);
    assert_eq!(archived_results[0].conversation_id, archived.id);
    assert_eq!(
        archived_results[0].lifecycle,
        ConversationLifecycle::Archived
    );
    assert!(reasoning_results.is_empty());
    assert_eq!(unicode_results.len(), 1);
    assert!(unicode_results[0].snippet.contains("Unicode needle"));
}

#[test]
fn returns_the_branch_that_contains_a_preserved_alternative() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let conversation = store
        .create_conversation("Branch search")
        .expect("conversation should be created");
    let first_request = append_search_message(
        &store,
        &conversation.id,
        StoredRole::User,
        "Original request",
        None,
    );
    append_search_message(
        &store,
        &conversation.id,
        StoredRole::Assistant,
        "Original answer",
        None,
    );
    let fork = store
        .fork_from_user_message(
            &conversation.id,
            &first_request.id,
            "Preserved marmalade branch",
        )
        .expect("alternative branch should be created");
    store
        .select_branch(&conversation.id, &conversation.current_branch_id)
        .expect("main branch should be reselected");

    let results = store
        .search_conversations("marmalade")
        .expect("branch search should succeed");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].conversation_id, conversation.id);
    assert_eq!(results[0].branch_id, fork.conversation.current_branch_id);
}

#[test]
fn normalizes_empty_queries_and_enforces_native_search_bounds() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    for index in 0..55 {
        store
            .create_conversation(&format!("Matching conversation {index}"))
            .expect("conversation should be created");
    }

    let empty = store
        .search_conversations("  \n ")
        .expect("empty search should succeed");
    let bounded = store
        .search_conversations("matching")
        .expect("bounded search should succeed");
    let too_long = store.search_conversations(&"x".repeat(201));

    assert!(empty.is_empty());
    assert_eq!(bounded.len(), 50);
    assert_eq!(
        too_long.expect_err("long query should fail").code,
        "invalid_request"
    );
}
