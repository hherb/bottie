//! Storage contract tests.

use std::fs;

use super::*;

/// Creates an isolated database path for one storage test.
fn test_database_path() -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!("bottie-storage-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).expect("test directory should be created");
    directory.join("bottie.sqlite3")
}

#[test]
fn initializes_ordered_migrations_and_default_local_profile() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");

    let status = store.status().expect("storage status should load");

    assert_eq!(status.schema_version, 2);
    assert_eq!(status.profile_name, "Local profile");
    assert_eq!(status.integrity_check, "ok");
    assert!(status.foreign_keys_enabled);
    assert_eq!(status.journal_mode, "wal");
}

#[test]
fn creates_lists_and_reopens_an_ordered_conversation() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("A durable first chat")
        .expect("conversation should be created");

    store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "Keep this after restart".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("user message should be stored");
    store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::Assistant,
            text: "It is safely in SQLite.".into(),
            reasoning: Some("Checked the local store.".into()),
            state: MessageState::Final,
            provider_id: Some("ollama".into()),
            model_id: Some("qwen3:latest".into()),
        })
        .expect("assistant message should be stored");
    let connection = store.open().expect("test connection should open");
    connection
        .execute("UPDATE messages SET created_at_ms = 1", [])
        .expect("test timestamps should be equalized");

    let reopened = ConversationStore::initialize(path)
        .expect("storage should reopen")
        .load_conversation(&conversation.id)
        .expect("conversation should load");
    let listed = store
        .list_conversations()
        .expect("conversations should list");

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].title, "A durable first chat");
    assert_eq!(reopened.messages.len(), 2);
    assert_eq!(reopened.messages[0].role, StoredRole::User);
    assert_eq!(reopened.messages[0].text, "Keep this after restart");
    assert_eq!(
        reopened.messages[1].reasoning.as_deref(),
        Some("Checked the local store.")
    );
    assert_eq!(reopened.messages[1].provider_id.as_deref(), Some("ollama"));
    assert_eq!(
        reopened.messages[1].model_id.as_deref(),
        Some("qwen3:latest")
    );
}

#[test]
fn rejects_empty_messages_and_unknown_conversations() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");

    let empty = store.append_message(NewStoredMessage {
        conversation_id: "missing".into(),
        role: StoredRole::User,
        text: "  ".into(),
        reasoning: None,
        state: MessageState::Final,
        provider_id: None,
        model_id: None,
    });
    let missing = store.load_conversation("missing");

    assert!(empty.is_err());
    assert!(missing.is_err());
}
