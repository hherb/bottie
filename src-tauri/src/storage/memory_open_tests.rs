//! Native `open_memory` tool-contract tests.

use serde_json::json;

use super::{
    ConversationStore, MessageState, NewStoredMessage, StoredRole,
    memory_open::{
        MAX_OPEN_MEMORY_SURROUNDING_TURNS, MAX_OPEN_MEMORY_TURN_CHARACTERS, OPEN_MEMORY_TOOL_NAME,
        OpenMemoryArguments,
    },
    tests::test_database_path,
};

/// Appends one stored message and returns its durable identity.
fn append_message(
    store: &ConversationStore,
    conversation_id: &str,
    role: StoredRole,
    text: &str,
    state: MessageState,
) -> String {
    store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation_id.into(),
                role,
                text: text.into(),
                reasoning: (role == StoredRole::Assistant).then(|| "private reasoning".into()),
                state,
                provider_id: (role == StoredRole::Assistant).then(|| "ollama".into()),
                model_id: (role == StoredRole::Assistant).then(|| "fixture-model".into()),
            },
            &[],
        )
        .expect("open-memory fixture message should append")
        .id
}

/// Appends one final retained turn.
fn append_final(
    store: &ConversationStore,
    conversation_id: &str,
    role: StoredRole,
    text: &str,
) -> String {
    append_message(store, conversation_id, role, text, MessageState::Final)
}

#[test]
fn returns_bounded_surrounding_turns_with_path_free_provenance() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Archived architecture")
        .expect("conversation should create");
    let first_id = append_final(
        &store,
        &conversation.id,
        StoredRole::User,
        "Where should local files be handled?",
    );
    append_final(
        &store,
        &conversation.id,
        StoredRole::Assistant,
        "Keep filesystem access inside Rust.",
    );
    let target_id = append_final(
        &store,
        &conversation.id,
        StoredRole::User,
        "What crosses the WebView boundary?",
    );
    append_final(
        &store,
        &conversation.id,
        StoredRole::Assistant,
        "Only bounded path-free metadata crosses it.",
    );
    let last_id = append_final(
        &store,
        &conversation.id,
        StoredRole::User,
        "That keeps the boundary inspectable.",
    );
    store
        .set_conversation_archived(&conversation.id, true)
        .expect("conversation should archive");

    let result = store
        .execute_open_memory(OpenMemoryArguments {
            conversation_id: conversation.id.clone(),
            message_id: target_id.clone(),
            before: None,
            after: None,
        })
        .expect("open_memory should retain archived conversation context");

    assert_eq!(OPEN_MEMORY_TOOL_NAME, "open_memory");
    assert_eq!(result.provenance.source_kind, "message");
    assert_eq!(result.provenance.conversation_id, conversation.id);
    assert_eq!(
        result.provenance.conversation_title,
        "Archived architecture"
    );
    assert_eq!(result.provenance.message_id, target_id);
    assert_eq!(result.turns.len(), 5);
    assert_eq!(result.turns[0].message_id, first_id);
    assert_eq!(result.turns[4].message_id, last_id);
    assert_eq!(result.turns.iter().filter(|turn| turn.is_match).count(), 1);
    assert_eq!(result.turns[2].role, StoredRole::User);
    assert_eq!(result.turns[2].text, "What crosses the WebView boundary?");

    let serialized = serde_json::to_value(&result).expect("tool result should serialize");
    assert_eq!(serialized["provenance"]["sourceKind"], json!("message"));
    assert_eq!(serialized["turns"][2]["isMatch"], json!(true));
    let serialized_text = serialized.to_string();
    for forbidden in [
        "private reasoning",
        "fixture-model",
        "ollama",
        "filePath",
        "databasePath",
        "attachment",
        "providerId",
    ] {
        assert!(!serialized_text.contains(forbidden));
    }
}

#[test]
fn follows_the_matched_messages_own_branch_lineage_without_changing_selection() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Branch provenance")
        .expect("conversation should create");
    let main_branch_id = conversation.current_branch_id.clone();
    let root_user = append_final(
        &store,
        &conversation.id,
        StoredRole::User,
        "Shared root question",
    );
    append_final(
        &store,
        &conversation.id,
        StoredRole::Assistant,
        "Shared root answer",
    );
    let original_user = append_final(
        &store,
        &conversation.id,
        StoredRole::User,
        "Original follow-up",
    );
    let original_assistant = append_final(
        &store,
        &conversation.id,
        StoredRole::Assistant,
        "Original sibling answer",
    );

    let fork = store
        .fork_from_user_message(&conversation.id, &original_user, "Alternative follow-up")
        .expect("branch should fork");
    let fork_target = fork.request_message_id;
    let fork_assistant = append_final(
        &store,
        &conversation.id,
        StoredRole::Assistant,
        "Alternative branch answer",
    );
    store
        .select_branch(&conversation.id, &main_branch_id)
        .expect("main branch should become selected again");

    let result = store
        .execute_open_memory(OpenMemoryArguments {
            conversation_id: conversation.id.clone(),
            message_id: fork_target.clone(),
            before: Some(MAX_OPEN_MEMORY_SURROUNDING_TURNS),
            after: Some(MAX_OPEN_MEMORY_SURROUNDING_TURNS),
        })
        .expect("the matched branch should open independently of current selection");

    let ids = result
        .turns
        .iter()
        .map(|turn| turn.message_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids.first(), Some(&root_user.as_str()));
    assert!(ids.contains(&fork_target.as_str()));
    assert!(ids.contains(&fork_assistant.as_str()));
    assert!(!ids.contains(&original_user.as_str()));
    assert!(!ids.contains(&original_assistant.as_str()));
    assert_eq!(
        store
            .load_conversation(&conversation.id)
            .expect("conversation should remain selected on main")
            .current_branch_id,
        main_branch_id
    );
}

#[test]
fn caps_requested_window_and_each_turn_without_splitting_unicode() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Bounded window")
        .expect("conversation should create");
    let mut message_ids = Vec::new();
    for index in 0..11 {
        message_ids.push(append_final(
            &store,
            &conversation.id,
            if index % 2 == 0 {
                StoredRole::User
            } else {
                StoredRole::Assistant
            },
            &format!(
                "turn {index} {}",
                "🦀".repeat(MAX_OPEN_MEMORY_TURN_CHARACTERS + 20)
            ),
        ));
    }

    let result = store
        .execute_open_memory(OpenMemoryArguments {
            conversation_id: conversation.id,
            message_id: message_ids[5].clone(),
            before: Some(usize::MAX),
            after: Some(usize::MAX),
        })
        .expect("oversized windows should cap to native policy");

    assert_eq!(
        result.turns.len(),
        (MAX_OPEN_MEMORY_SURROUNDING_TURNS * 2) + 1
    );
    assert_eq!(result.turns[0].message_id, message_ids[2]);
    assert_eq!(result.turns.last().unwrap().message_id, message_ids[8]);
    assert!(
        result
            .turns
            .iter()
            .all(|turn| turn.text.chars().count() <= MAX_OPEN_MEMORY_TURN_CHARACTERS)
    );
    assert!(result.turns.iter().all(|turn| turn.text.ends_with('…')));
}

#[test]
fn rejects_invalid_or_unretained_provenance_and_unknown_arguments() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Validation")
        .expect("conversation should create");
    let final_id = append_final(
        &store,
        &conversation.id,
        StoredRole::User,
        "Retained final message",
    );
    let failed_id = append_message(
        &store,
        &conversation.id,
        StoredRole::Assistant,
        "Unretained failed response",
        MessageState::Failed,
    );
    let other = store
        .create_conversation("Other conversation")
        .expect("second conversation should create");

    for arguments in [
        OpenMemoryArguments {
            conversation_id: " ".into(),
            message_id: final_id.clone(),
            before: None,
            after: None,
        },
        OpenMemoryArguments {
            conversation_id: conversation.id.clone(),
            message_id: " ".into(),
            before: None,
            after: None,
        },
        OpenMemoryArguments {
            conversation_id: "x".repeat(129),
            message_id: final_id.clone(),
            before: None,
            after: None,
        },
    ] {
        assert_eq!(
            store
                .execute_open_memory(arguments)
                .expect_err("blank identities should fail")
                .code,
            "invalid_request"
        );
    }

    for arguments in [
        OpenMemoryArguments {
            conversation_id: other.id,
            message_id: final_id,
            before: None,
            after: None,
        },
        OpenMemoryArguments {
            conversation_id: conversation.id.clone(),
            message_id: failed_id,
            before: None,
            after: None,
        },
        OpenMemoryArguments {
            conversation_id: conversation.id.clone(),
            message_id: "missing-message".into(),
            before: None,
            after: None,
        },
    ] {
        assert_eq!(
            store
                .execute_open_memory(arguments)
                .expect_err("unavailable provenance should not open")
                .code,
            "not_found"
        );
    }

    let unknown = serde_json::from_value::<OpenMemoryArguments>(json!({
        "conversationId": conversation.id,
        "messageId": "message-id",
        "includeReasoning": true
    }));
    assert!(unknown.is_err());
}

#[test]
fn excludes_trash_and_skips_nonfinal_surrounding_messages() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Lifecycle")
        .expect("conversation should create");
    let first_id = append_final(
        &store,
        &conversation.id,
        StoredRole::User,
        "First retained turn",
    );
    append_message(
        &store,
        &conversation.id,
        StoredRole::Assistant,
        "Failed response",
        MessageState::Failed,
    );
    let target_id = append_final(
        &store,
        &conversation.id,
        StoredRole::User,
        "Second retained turn",
    );

    let result = store
        .execute_open_memory(OpenMemoryArguments {
            conversation_id: conversation.id.clone(),
            message_id: target_id.clone(),
            before: Some(2),
            after: Some(2),
        })
        .expect("final turns should open around failed siblings");
    assert_eq!(
        result
            .turns
            .iter()
            .map(|turn| turn.message_id.as_str())
            .collect::<Vec<_>>(),
        vec![first_id.as_str(), target_id.as_str()]
    );

    store
        .delete_conversation(&conversation.id)
        .expect("conversation should move to trash");
    assert_eq!(
        store
            .execute_open_memory(OpenMemoryArguments {
                conversation_id: conversation.id,
                message_id: target_id,
                before: None,
                after: None,
            })
            .expect_err("trashed memory should not open")
            .code,
        "not_found"
    );
}
