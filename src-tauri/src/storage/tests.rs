//! Storage contract tests.

use std::fs;

use super::*;

impl ConversationStore {
    /// Appends one message without attachment associations through the storage test boundary.
    pub(super) fn append_message(
        &self,
        message: NewStoredMessage,
    ) -> Result<StoredMessage, StorageError> {
        self.append_message_with_attachments(message, &[])
    }
}

/// Creates an isolated database path for one storage test.
pub(super) fn test_database_path() -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!("bottie-storage-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).expect("test directory should be created");
    directory.join("bottie.sqlite3")
}

/// Drains durable attachment work synchronously at the explicit storage test boundary.
pub(super) fn process_pending_attachments(store: &ConversationStore) {
    while store
        .process_next_pending_attachment()
        .expect("pending attachment processing should succeed")
        .is_some()
    {}
}

/// Completes a newly ingested fixture while preserving its selection-specific duplicate flag.
pub(super) fn completed_ingestion(
    store: &ConversationStore,
    ingested: IngestedAttachment,
) -> IngestedAttachment {
    process_pending_attachments(store);
    let stored = store
        .stored_attachment_for_test(&ingested.id)
        .expect("processed attachment should load")
        .expect("processed attachment should exist");
    IngestedAttachment {
        id: stored.id,
        display_name: stored.display_name,
        mime_type: stored.mime_type,
        byte_size: stored.byte_size,
        sha256: stored.sha256,
        extraction: stored.extraction,
        indexing: stored.indexing,
        normalization: stored.normalization,
        duplicate: ingested.duplicate,
    }
}

#[test]
fn initializes_ordered_migrations_and_default_local_profile() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");

    let status = store.status().expect("storage status should load");

    assert_eq!(status.schema_version, 15);
    assert_eq!(status.profile_name, "Local profile");
    assert_eq!(status.integrity_check, "ok");
    assert!(status.foreign_keys_enabled);
    assert_eq!(status.journal_mode, "wal");
}

#[test]
fn upgrades_a_version_two_store_without_rewriting_existing_messages() {
    let path = test_database_path();
    let connection = Connection::open(&path).expect("version two database should open");
    connection
        .execute_batch(MIGRATION_1)
        .expect("foundation migration should apply");
    connection
        .execute(
            "INSERT INTO schema_migrations (version, name, applied_at_ms)
             VALUES (1, 'storage foundation', 1)",
            [],
        )
        .expect("first migration should be recorded");
    connection
        .execute(
            "INSERT INTO profiles (id, name, created_at_ms) VALUES (?1, ?2, 1)",
            params![DEFAULT_PROFILE_ID, DEFAULT_PROFILE_NAME],
        )
        .expect("default profile should be inserted");
    connection
        .execute_batch(MIGRATION_2)
        .expect("message-order migration should apply");
    connection
        .execute(
            "INSERT INTO schema_migrations (version, name, applied_at_ms)
             VALUES (2, 'branch-local message order', 2)",
            [],
        )
        .expect("second migration should be recorded");
    connection
        .pragma_update(None, "user_version", 2)
        .expect("version should be set");
    drop(connection);

    let store = ConversationStore::initialize(path).expect("version two store should upgrade");
    let status = store.status().expect("upgraded status should load");
    let connection = store.open().expect("upgraded database should open");
    let provider_run_table: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = 'provider_runs'",
            [],
            |row| row.get(0),
        )
        .expect("provider run table should be queryable");

    assert_eq!(status.schema_version, 15);
    assert_eq!(provider_run_table, 1);
}

#[test]
fn persists_provider_run_provenance_and_terminal_usage() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Measured generation")
        .expect("conversation should be created");
    let request = store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "Count this request".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("request should be stored");
    let run_id = uuid::Uuid::new_v4().to_string();

    store
        .start_provider_run(NewProviderRun {
            id: run_id.clone(),
            conversation_id: conversation.id.clone(),
            request_message_id: request.id,
            provider_id: "ollama".into(),
            model_id: "qwen3:latest".into(),
            reasoning_effort: StoredReasoningEffort::Low,
            temperature: Some(0.25),
            max_output_tokens: Some(1_024),
        })
        .expect("provider run should start");
    store
        .finish_provider_run(
            &run_id,
            ProviderRunState::Completed,
            None,
            Some(StoredUsage {
                input_tokens: Some(23),
                output_tokens: Some(41),
                cost_usd: Some(0.0012),
            }),
        )
        .expect("provider run should complete");
    drop(store);

    let reopened = ConversationStore::initialize(path)
        .expect("storage should reopen")
        .load_conversation(&conversation.id)
        .expect("conversation should load");
    let stored_run = reopened.messages[1]
        .provider_run
        .as_ref()
        .expect("assistant response should include run provenance");

    assert_eq!(stored_run.id, run_id);
    assert_eq!(stored_run.state, ProviderRunState::Completed);
    assert_eq!(stored_run.reasoning_effort, StoredReasoningEffort::Low);
    assert!(
        stored_run
            .completed_at_ms
            .is_some_and(|completed_at_ms| completed_at_ms >= stored_run.started_at_ms)
    );
    assert_eq!(
        stored_run.usage,
        Some(StoredUsage {
            input_tokens: Some(23),
            output_tokens: Some(41),
            cost_usd: Some(0.0012),
        })
    );
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

#[test]
fn renames_archives_and_reactivates_conversations_on_append() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Lifecycle draft")
        .expect("conversation should be created");

    let renamed = store
        .rename_conversation(&conversation.id, "  Renamed   conversation  ")
        .expect("conversation should be renamed");
    let archived = store
        .set_conversation_archived(&conversation.id, true)
        .expect("conversation should be archived");
    drop(store);
    let store = ConversationStore::initialize(path).expect("storage should reopen");

    assert_eq!(renamed.title, "Renamed conversation");
    assert_eq!(renamed.lifecycle, ConversationLifecycle::Active);
    assert_eq!(archived.lifecycle, ConversationLifecycle::Archived);
    assert_eq!(
        store
            .load_conversation(&conversation.id)
            .expect("archived conversation should remain readable")
            .title,
        "Renamed conversation"
    );

    store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "Bring this conversation back".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("appending should reactivate the conversation");

    assert_eq!(
        store
            .list_conversations()
            .expect("conversations should list")[0]
            .lifecycle,
        ConversationLifecycle::Active
    );
}

#[test]
fn moves_conversations_to_trash_and_restores_without_data_loss() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Recoverable conversation")
        .expect("conversation should be created");
    store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "Keep this message".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("message should be stored");

    let deleted = store
        .delete_conversation(&conversation.id)
        .expect("conversation should move to trash");
    drop(store);
    let store = ConversationStore::initialize(path).expect("storage should reopen");

    assert_eq!(deleted.lifecycle, ConversationLifecycle::Deleted);
    assert!(store.load_conversation(&conversation.id).is_err());
    assert!(
        store
            .append_message(NewStoredMessage {
                conversation_id: conversation.id.clone(),
                role: StoredRole::User,
                text: "Do not append while deleted".into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            })
            .is_err()
    );

    let restored = store
        .restore_conversation(&conversation.id)
        .expect("conversation should restore");
    let reopened = store
        .load_conversation(&conversation.id)
        .expect("restored conversation should load");

    assert_eq!(restored.lifecycle, ConversationLifecycle::Active);
    assert_eq!(reopened.messages.len(), 1);
    assert_eq!(reopened.messages[0].text, "Keep this message");
}

#[test]
fn lists_active_archived_and_deleted_lifecycle_states() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let active = store
        .create_conversation("Active")
        .expect("active conversation should be created");
    let archived = store
        .create_conversation("Archived")
        .expect("archived conversation should be created");
    let deleted = store
        .create_conversation("Deleted")
        .expect("deleted conversation should be created");
    store
        .set_conversation_archived(&archived.id, true)
        .expect("conversation should archive");
    store
        .delete_conversation(&deleted.id)
        .expect("conversation should move to trash");

    let listed = store
        .list_conversations()
        .expect("all conversation states should list");

    assert_eq!(listed.len(), 3);
    assert_eq!(listed[0].id, active.id);
    assert_eq!(listed[0].lifecycle, ConversationLifecycle::Active);
    assert_eq!(listed[1].lifecycle, ConversationLifecycle::Archived);
    assert_eq!(listed[2].lifecycle, ConversationLifecycle::Deleted);
}
