//! Native SQLite FTS5 lexical-memory contract tests.

use std::fs;

use super::{
    ConversationStore, MessageState, NewProviderRun, NewStoredMessage, ProviderRunState,
    RunBlockKind, StoredReasoningEffort, StoredRole,
    memory_lexical::{MemoryLexicalFilters, MemorySourceKind},
    tests::{process_pending_attachments, test_database_path},
};

/// Appends one final fixture message without retained files.
fn append_message(
    store: &ConversationStore,
    conversation_id: &str,
    role: StoredRole,
    text: &str,
    reasoning: Option<&str>,
) -> super::StoredMessage {
    store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation_id.into(),
                role,
                text: text.into(),
                reasoning: reasoning.map(str::to_owned),
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[],
        )
        .expect("memory fixture message should append")
}

#[test]
fn migration_backfills_final_message_text_without_reasoning() {
    let path = test_database_path();
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Lexical migration")
        .expect("conversation should create");
    append_message(
        &store,
        &conversation.id,
        StoredRole::Assistant,
        "Visible migration marigold",
        Some("private migration chrysanthemum"),
    );
    let connection = store.open().expect("store should open");
    connection
        .execute_batch(super::memory_lexical::REMOVE_LEXICAL_SCHEMA_FOR_TEST)
        .expect("lexical schema should be removable in the fixture");
    connection
        .execute("DELETE FROM schema_migrations WHERE version = 16", [])
        .expect("lexical migration record should be removable");
    connection
        .pragma_update(None, "user_version", 15)
        .expect("fixture version should rewind");
    drop(connection);
    drop(store);

    let upgraded =
        ConversationStore::initialize(path).expect("version fifteen store should upgrade");
    let visible = upgraded
        .search_memory_lexically("marigold", MemoryLexicalFilters::default())
        .expect("visible text should search");
    let reasoning = upgraded
        .search_memory_lexically("chrysanthemum", MemoryLexicalFilters::default())
        .expect("reasoning query should search safely");

    assert_eq!(
        upgraded
            .status()
            .expect("status should load")
            .schema_version,
        16
    );
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].source_kind, MemorySourceKind::Message);
    assert!(reasoning.is_empty());
}

#[test]
fn indexes_only_terminal_message_answers_and_aggregates_streamed_deltas() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Streamed lexical source")
        .expect("conversation should create");
    let request = append_message(
        &store,
        &conversation.id,
        StoredRole::User,
        "Describe durable indexing",
        None,
    );
    let run_id = uuid::Uuid::new_v4().to_string();
    store
        .start_provider_run(NewProviderRun {
            id: run_id.clone(),
            conversation_id: conversation.id,
            request_message_id: request.id,
            provider_id: "ollama".into(),
            model_id: "fixture".into(),
            reasoning_effort: StoredReasoningEffort::Off,
            temperature: None,
            max_output_tokens: None,
        })
        .expect("provider run should start");
    store
        .checkpoint_provider_delta(&run_id, RunBlockKind::Text, "purple ")
        .expect("first delta should persist");
    store
        .checkpoint_provider_delta(&run_id, RunBlockKind::Text, "platypus")
        .expect("second delta should persist");

    assert!(
        store
            .search_memory_lexically("purple platypus", MemoryLexicalFilters::default())
            .expect("partial query should run")
            .is_empty()
    );

    store
        .finish_provider_run(&run_id, ProviderRunState::Completed, None, None)
        .expect("provider run should complete");
    let completed = store
        .search_memory_lexically("purple platypus", MemoryLexicalFilters::default())
        .expect("completed query should run");

    assert_eq!(completed.len(), 1);
    assert!(completed[0].snippet.contains("purple platypus"));
}

#[test]
fn searches_only_associated_ready_attachment_text() {
    let path = test_database_path();
    let source = path.with_file_name("field-notes.md");
    fs::write(&source, "# Field notes\nAmber kingfisher habitat").expect("fixture should write");
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let attachment = store
        .ingest_attachment(&source)
        .expect("attachment should ingest");
    process_pending_attachments(&store);

    assert!(
        store
            .search_memory_lexically("kingfisher", MemoryLexicalFilters::default())
            .expect("unassociated query should run")
            .is_empty()
    );

    let conversation = store
        .create_conversation("Field research")
        .expect("conversation should create");
    store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation.id.clone(),
                role: StoredRole::User,
                text: "Keep these notes".into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            std::slice::from_ref(&attachment.id),
        )
        .expect("attachment should associate");
    drop(store);

    let reopened = ConversationStore::initialize(path).expect("storage should reopen");
    let hits = reopened
        .search_memory_lexically(
            "amber kingfisher",
            MemoryLexicalFilters {
                source_kind: Some(MemorySourceKind::Attachment),
                conversation_id: Some(conversation.id),
                ..MemoryLexicalFilters::default()
            },
        )
        .expect("associated attachment should search");

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].source_kind, MemorySourceKind::Attachment);
    assert_eq!(hits[0].source_id, attachment.id);
}

#[test]
fn ranks_with_bm25_and_applies_lifecycle_source_conversation_and_date_filters() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let strong = store
        .create_conversation("Strong lexical match")
        .expect("conversation should create");
    let strong_message = append_message(
        &store,
        &strong.id,
        StoredRole::User,
        "orchard orchard orchard orchard harvest",
        None,
    );
    let weak = store
        .create_conversation("Weak lexical match")
        .expect("conversation should create");
    append_message(
        &store,
        &weak.id,
        StoredRole::User,
        "orchard harvest notes with many unrelated words added here",
        None,
    );
    let trashed = store
        .create_conversation("Trashed lexical match")
        .expect("conversation should create");
    append_message(
        &store,
        &trashed.id,
        StoredRole::User,
        "orchard orchard orchard orchard orchard harvest",
        None,
    );
    store
        .delete_conversation(&trashed.id)
        .expect("conversation should move to trash");

    let ranked = store
        .search_memory_lexically("orchard harvest", MemoryLexicalFilters::default())
        .expect("ranked query should run");
    let filtered = store
        .search_memory_lexically(
            "orchard harvest",
            MemoryLexicalFilters {
                source_kind: Some(MemorySourceKind::Message),
                conversation_id: Some(weak.id.clone()),
                created_after_ms: Some(0),
                created_before_ms: Some(i64::MAX),
                limit: 5,
            },
        )
        .expect("filtered query should run");

    assert_eq!(ranked.len(), 2);
    assert_eq!(ranked[0].source_id, strong_message.id);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].source_kind, MemorySourceKind::Message);
}

#[test]
fn normalizes_fts_syntax_and_enforces_native_query_and_result_bounds() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    for index in 0..55 {
        let conversation = store
            .create_conversation(&format!("Bounded memory {index}"))
            .expect("conversation should create");
        append_message(
            &store,
            &conversation.id,
            StoredRole::User,
            &format!("shared lexical token item {index}"),
            None,
        );
    }

    let empty = store
        .search_memory_lexically(" \n ", MemoryLexicalFilters::default())
        .expect("empty query should succeed");
    let syntax = store
        .search_memory_lexically("shared OR \"unterminated", MemoryLexicalFilters::default())
        .expect("operator-shaped text should be escaped");
    let bounded = store
        .search_memory_lexically("shared lexical", MemoryLexicalFilters::default())
        .expect("bounded query should succeed");
    let too_long = store.search_memory_lexically(&"x".repeat(201), MemoryLexicalFilters::default());

    assert!(empty.is_empty());
    assert!(syntax.is_empty());
    assert_eq!(bounded.len(), 50);
    assert_eq!(
        too_long.expect_err("long query should fail").code,
        "invalid_request"
    );
}
