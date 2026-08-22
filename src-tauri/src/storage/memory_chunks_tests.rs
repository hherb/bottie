//! Native deterministic memory-chunk catalog contract tests.

use std::fs;

use rusqlite::params;

use super::{
    ConversationStore, MessageState, NewProviderRun, NewStoredMessage, ProviderRunState,
    RunBlockKind, StoredReasoningEffort, StoredRole,
    memory_chunks::{
        CHUNK_OVERLAP_CHARACTERS, CHUNKING_ALGORITHM, CHUNKING_VERSION, MAX_CHUNK_CHARACTERS,
        MIN_CHUNK_SPLIT_CHARACTERS, MemoryChunkSourceKind, chunks_for_text,
    },
    tests::{process_pending_attachments, test_database_path},
};

/// Appends one final message used by catalog fixtures.
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
        .expect("memory chunk fixture should append")
}

#[test]
fn chunks_unicode_deterministically_with_bounded_overlap_and_exact_offsets() {
    let text = format!(
        "Intro 🦜 {} middle {} conclusion",
        "alpha beta gamma ".repeat(90),
        "delta epsilon zeta ".repeat(90)
    );
    let first = chunks_for_text("message", "source-1", &text);
    let second = chunks_for_text("message", "source-1", &text);
    let characters = text.chars().collect::<Vec<_>>();

    assert_eq!(first, second);
    assert!(first.len() > 1);
    for (ordinal, chunk) in first.iter().enumerate() {
        assert_eq!(chunk.ordinal, ordinal);
        assert_eq!(chunk.chunking_version, CHUNKING_VERSION);
        assert!(chunk.text.chars().count() <= MAX_CHUNK_CHARACTERS);
        assert_eq!(
            chunk.text,
            characters[chunk.start_character..chunk.end_character]
                .iter()
                .collect::<String>()
        );
        assert_eq!(chunk.id.len(), 64);
    }
    for pair in first.windows(2) {
        assert!(pair[1].start_character < pair[0].end_character);
        assert!(pair[1].start_character > pair[0].start_character);
    }
}

#[test]
fn migration_backfills_final_messages_and_ready_documents_without_reasoning() {
    let path = test_database_path();
    let document_path = path.with_file_name("chunk-catalog-notes.md");
    fs::write(
        &document_path,
        format!("catalog kingfisher {}", "document words ".repeat(120)),
    )
    .expect("document fixture should write");
    let store = ConversationStore::initialize(path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Chunk migration")
        .expect("conversation should create");
    let message = append_message(
        &store,
        &conversation.id,
        StoredRole::Assistant,
        &format!("visible marigold {}", "message words ".repeat(120)),
        Some("private chrysanthemum reasoning"),
    );
    let attachment = store
        .ingest_attachment(&document_path)
        .expect("attachment should ingest");
    process_pending_attachments(&store);
    let connection = store.open().expect("store should open");
    connection
        .execute_batch(super::memory_semantic::REMOVE_MEMORY_SEMANTIC_SCHEMA_FOR_TEST)
        .expect("semantic schema should be removable in the fixture");
    connection
        .execute_batch(super::memory_chunks::REMOVE_MEMORY_CHUNK_SCHEMA_FOR_TEST)
        .expect("chunk schema should be removable in the fixture");
    connection
        .execute_batch("DROP TABLE conversation_memory_preferences")
        .expect("later memory preference schema should be removable in the fixture");
    connection
        .execute("DELETE FROM schema_migrations WHERE version >= 17", [])
        .expect("chunk and later migration records should be removable");
    connection
        .pragma_update(None, "user_version", 16)
        .expect("fixture version should rewind");
    drop(connection);
    drop(store);

    let upgraded =
        ConversationStore::initialize(path).expect("version sixteen store should upgrade");
    let message_chunks = upgraded
        .memory_chunks_for_source_for_test(MemoryChunkSourceKind::Message, &message.id)
        .expect("message chunks should load");
    let attachment_chunks = upgraded
        .memory_chunks_for_source_for_test(MemoryChunkSourceKind::Attachment, &attachment.id)
        .expect("attachment chunks should load");
    let metadata: (i64, String, i64, i64, i64) = upgraded
        .open()
        .expect("store should open")
        .query_row(
            "SELECT chunking_version, algorithm, max_characters,
                    min_split_characters, overlap_characters
             FROM memory_chunk_metadata WHERE singleton = 1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("chunk metadata should load");

    assert_eq!(
        upgraded
            .status()
            .expect("status should load")
            .schema_version,
        19
    );
    assert!(message_chunks.len() > 1);
    assert!(attachment_chunks.len() > 1);
    assert!(
        message_chunks
            .iter()
            .any(|chunk| chunk.text.contains("marigold"))
    );
    assert!(
        message_chunks
            .iter()
            .all(|chunk| !chunk.text.contains("chrysanthemum"))
    );
    assert!(
        attachment_chunks
            .iter()
            .any(|chunk| chunk.text.contains("kingfisher"))
    );
    assert_eq!(
        metadata,
        (
            CHUNKING_VERSION,
            CHUNKING_ALGORITHM.into(),
            MAX_CHUNK_CHARACTERS as i64,
            MIN_CHUNK_SPLIT_CHARACTERS as i64,
            CHUNK_OVERLAP_CHARACTERS as i64,
        )
    );
}

#[test]
fn catalogs_new_ready_documents_and_removes_rows_when_extraction_is_invalidated() {
    let path = test_database_path();
    let document_path = path.with_file_name("runtime-chunk-notes.md");
    fs::write(
        &document_path,
        format!("runtime rosella {}", "document words ".repeat(120)),
    )
    .expect("document fixture should write");
    let store = ConversationStore::initialize(path).expect("storage should initialize");
    let attachment = store
        .ingest_attachment(&document_path)
        .expect("attachment should ingest");

    assert!(
        store
            .memory_chunks_for_source_for_test(MemoryChunkSourceKind::Attachment, &attachment.id)
            .expect("pending chunks should load")
            .is_empty()
    );

    process_pending_attachments(&store);
    let ready = store
        .memory_chunks_for_source_for_test(MemoryChunkSourceKind::Attachment, &attachment.id)
        .expect("ready chunks should load");
    assert!(ready.len() > 1);
    assert!(ready.iter().any(|chunk| chunk.text.contains("rosella")));

    store
        .open()
        .expect("store should open")
        .execute(
            "UPDATE attachment_extractions
             SET state = 'failed', format = NULL, text_content = NULL,
                 character_count = NULL, page_count = NULL, error_code = 'fixture_failure'
             WHERE attachment_id = ?1",
            [&attachment.id],
        )
        .expect("fixture extraction should invalidate");
    assert!(
        store
            .memory_chunks_for_source_for_test(MemoryChunkSourceKind::Attachment, &attachment.id)
            .expect("invalidated chunks should load")
            .is_empty()
    );
}

#[test]
fn catalogs_only_completed_answer_text_and_replaces_stale_source_rows() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Chunk lifecycle")
        .expect("conversation should create");
    let request = append_message(
        &store,
        &conversation.id,
        StoredRole::User,
        "Describe deterministic chunks",
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
        .checkpoint_provider_delta(&run_id, RunBlockKind::Reasoning, "hidden analysis")
        .expect("reasoning should persist");
    store
        .checkpoint_provider_delta(
            &run_id,
            RunBlockKind::Text,
            &format!("answer albatross {}", "streamed words ".repeat(120)),
        )
        .expect("answer should persist");
    let response_id: String = store
        .open()
        .expect("store should open")
        .query_row(
            "SELECT id FROM messages WHERE provider_run_id = ?1",
            [&run_id],
            |row| row.get(0),
        )
        .expect("response should exist");

    assert!(
        store
            .memory_chunks_for_source_for_test(MemoryChunkSourceKind::Message, &response_id)
            .expect("partial chunks should load")
            .is_empty()
    );

    store
        .finish_provider_run(&run_id, ProviderRunState::Completed, None, None)
        .expect("provider run should finish");
    let completed = store
        .memory_chunks_for_source_for_test(MemoryChunkSourceKind::Message, &response_id)
        .expect("completed chunks should load");
    assert!(completed.len() > 1);
    assert!(
        completed
            .iter()
            .any(|chunk| chunk.text.contains("albatross"))
    );
    assert!(
        completed
            .iter()
            .all(|chunk| !chunk.text.contains("analysis"))
    );

    let connection = store.open().expect("store should open");
    connection
        .execute(
            "UPDATE messages SET state = 'failed' WHERE id = ?1",
            params![response_id],
        )
        .expect("fixture state should update");
    drop(connection);
    assert!(
        store
            .memory_chunks_for_source_for_test(MemoryChunkSourceKind::Message, &response_id)
            .expect("failed chunks should load")
            .is_empty()
    );
}
