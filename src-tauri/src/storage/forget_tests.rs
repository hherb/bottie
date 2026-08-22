//! Permanent per-conversation forget contract tests.

use std::fs;

use super::{
    ConversationLifecycle, ConversationStore, MessageState, NewProviderRun, NewStoredMessage,
    ProviderRunState, StoredReasoningEffort, StoredRole,
    memory_chunks::MemoryChunkSourceKind,
    memory_lexical::{MemoryLexicalFilters, MemorySourceKind},
    tests::{process_pending_attachments, test_database_path},
};

/// Appends one final user message for a forget-policy fixture.
fn append_message(store: &ConversationStore, conversation_id: &str, text: &str) -> String {
    store
        .append_message(NewStoredMessage {
            conversation_id: conversation_id.into(),
            role: StoredRole::User,
            text: text.into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("forget fixture should append")
        .id
}

#[test]
fn permanently_forgets_only_trashed_conversations_without_active_generation() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let active = store
        .create_conversation("Active source")
        .expect("active conversation should create");
    let archived = store
        .create_conversation("Archived source")
        .expect("archived conversation should create");
    store
        .set_conversation_archived(&archived.id, true)
        .expect("conversation should archive");

    for conversation_id in [&active.id, &archived.id] {
        let error = store
            .forget_conversation(conversation_id)
            .expect_err("readable conversations must not be forgotten directly");
        assert_eq!(error.code, "invalid_request");
    }

    let running = store
        .create_conversation("Running source")
        .expect("running conversation should create");
    let request_message_id = append_message(&store, &running.id, "finish this response first");
    let run_id = uuid::Uuid::new_v4().to_string();
    store
        .start_provider_run(NewProviderRun {
            id: run_id.clone(),
            conversation_id: running.id.clone(),
            request_message_id,
            provider_id: "ollama".into(),
            model_id: "fixture".into(),
            reasoning_effort: StoredReasoningEffort::Off,
            temperature: None,
            max_output_tokens: None,
        })
        .expect("provider run should start");
    store
        .delete_conversation(&running.id)
        .expect("running fixture should move to trash through the storage boundary");
    let error = store
        .forget_conversation(&running.id)
        .expect_err("active native generation must block permanent deletion");
    assert_eq!(error.code, "invalid_request");
    store
        .finish_provider_run(&run_id, ProviderRunState::Cancelled, None, None)
        .expect("provider run should finish");
    store
        .forget_conversation(&running.id)
        .expect("finished trashed conversation should be forgotten");

    let connection = store.open().expect("fixture database should open");
    let retained_run_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM provider_runs WHERE conversation_id = ?1",
            [&running.id],
            |row| row.get(0),
        )
        .expect("provider-run ownership should be queryable");
    assert_eq!(retained_run_count, 0);
    drop(connection);
    assert!(
        store
            .list_conversations()
            .expect("list should load")
            .iter()
            .all(|item| item.id != running.id)
    );
    assert_eq!(
        store
            .forget_conversation("missing-conversation")
            .expect_err("missing conversation should reject")
            .code,
        "not_found"
    );
}

#[test]
fn forget_removes_owned_sources_and_derived_rows_but_respects_attachment_retention() {
    let path = test_database_path();
    let exclusive_path = path.with_file_name("exclusive-forget.txt");
    let shared_path = path.with_file_name("shared-forget.txt");
    fs::write(&exclusive_path, "exclusive violet forget content")
        .expect("exclusive fixture should write");
    fs::write(&shared_path, "shared violet forget content").expect("shared fixture should write");
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let forgotten = store
        .create_conversation("Forget this")
        .expect("forgotten conversation should create");
    let retained = store
        .create_conversation("Keep this")
        .expect("retained conversation should create");
    let message_id = append_message(&store, &forgotten.id, "violet private conversation memory");
    let exclusive = store
        .ingest_attachment(&exclusive_path)
        .expect("exclusive attachment should ingest");
    let shared = store
        .ingest_attachment(&shared_path)
        .expect("shared attachment should ingest");
    process_pending_attachments(&store);
    store
        .add_conversation_attachments(&forgotten.id, &[exclusive.id.clone(), shared.id.clone()])
        .expect("forgotten associations should persist");
    store
        .add_conversation_attachments(&retained.id, std::slice::from_ref(&shared.id))
        .expect("shared retained association should persist");
    assert!(
        !store
            .memory_chunks_for_source_for_test(MemoryChunkSourceKind::Message, &message_id)
            .expect("message chunks should load before forget")
            .is_empty()
    );
    store
        .delete_conversation(&forgotten.id)
        .expect("conversation should move to trash");

    store
        .forget_conversation(&forgotten.id)
        .expect("trashed conversation should be forgotten");

    assert!(store.load_conversation(&forgotten.id).is_err());
    assert!(store.restore_conversation(&forgotten.id).is_err());
    assert!(
        store
            .list_conversations()
            .expect("list should load")
            .iter()
            .all(|item| item.id != forgotten.id)
    );
    assert!(
        store
            .memory_chunks_for_source_for_test(MemoryChunkSourceKind::Message, &message_id)
            .expect("message chunks should load")
            .is_empty()
    );
    assert!(
        store
            .search_memory_lexically(
                "violet private",
                MemoryLexicalFilters {
                    source_kind: Some(MemorySourceKind::Message),
                    ..MemoryLexicalFilters::default()
                },
            )
            .expect("lexical search should succeed")
            .is_empty()
    );
    assert!(
        store
            .stored_attachment_for_test(&exclusive.id)
            .expect("exclusive catalog should load")
            .is_some(),
        "unshared bytes remain inside the existing safety window"
    );
    assert!(
        store
            .stored_attachment_for_test(&shared.id)
            .expect("shared catalog should load")
            .is_some()
    );
    assert!(
        !store
            .memory_chunks_for_source_for_test(MemoryChunkSourceKind::Attachment, &exclusive.id)
            .expect("exclusive attachment chunks should remain during the safety window")
            .is_empty()
    );

    let collection = store
        .collect_all_unreferenced_attachments_for_test()
        .expect("explicit test collection should succeed");
    assert_eq!(collection.catalog_entries_removed, 1);
    assert!(
        store
            .stored_attachment_for_test(&exclusive.id)
            .expect("exclusive catalog should reload")
            .is_none()
    );
    assert!(
        store
            .memory_chunks_for_source_for_test(MemoryChunkSourceKind::Attachment, &exclusive.id)
            .expect("exclusive attachment chunks should reload")
            .is_empty()
    );
    assert!(
        store
            .stored_attachment_for_test(&shared.id)
            .expect("shared catalog should reload")
            .is_some()
    );
    assert!(
        !store
            .memory_chunks_for_source_for_test(MemoryChunkSourceKind::Attachment, &shared.id)
            .expect("shared attachment chunks should reload")
            .is_empty()
    );
    drop(store);

    let reopened = ConversationStore::initialize(path).expect("storage should reopen");
    let listed = reopened
        .list_conversations()
        .expect("reopened list should load");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, retained.id);
    assert_eq!(listed[0].lifecycle, ConversationLifecycle::Active);
}
