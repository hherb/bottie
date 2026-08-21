//! Storage contract tests for durable attachment indexing readiness.

use std::fs;

use super::{
    AttachmentIndexingState, ConversationStore, extraction::MAX_EXTRACTED_TEXT_BYTES,
    migrations::MIGRATION_14, tests::test_database_path,
};

#[test]
fn background_processing_marks_extracted_text_indexable() {
    let database_path = test_database_path();
    let source_path = database_path.with_file_name("notes.md");
    fs::write(&source_path, "# Durable indexing readiness\n").expect("write source");
    let store = ConversationStore::initialize(database_path).expect("initialize store");

    let ingested = store
        .ingest_attachment(&source_path)
        .expect("ingest attachment");
    assert_eq!(
        ingested.indexing.state,
        AttachmentIndexingState::WaitingForExtraction
    );

    let completed = store
        .process_next_pending_attachment()
        .expect("process pending attachment")
        .expect("one pending attachment");

    assert_eq!(completed.indexing.state, AttachmentIndexingState::Indexable);
    assert!(
        store
            .process_next_pending_attachment()
            .expect("check drained queue")
            .is_none()
    );
}

#[test]
fn background_processing_records_terminal_non_indexable_states() {
    let database_path = test_database_path();
    let image_path = database_path.with_file_name("pixel.png");
    let oversized_text_path = database_path.with_file_name("oversized.txt");
    fs::write(
        &image_path,
        [
            0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, b'n', b'o', b't', b'-', b'a', b'-',
            b'p', b'n', b'g',
        ],
    )
    .expect("write image-shaped source");
    fs::write(
        &oversized_text_path,
        vec![b'a'; MAX_EXTRACTED_TEXT_BYTES + 1],
    )
    .expect("write oversized text");
    let store = ConversationStore::initialize(database_path).expect("initialize store");
    let image = store.ingest_attachment(&image_path).expect("ingest image");
    let oversized_text = store
        .ingest_attachment(&oversized_text_path)
        .expect("ingest oversized text");

    while store
        .process_next_pending_attachment()
        .expect("process pending attachment")
        .is_some()
    {}

    let image = store
        .stored_attachment_for_test(&image.id)
        .expect("load image")
        .expect("stored image");
    let oversized_text = store
        .stored_attachment_for_test(&oversized_text.id)
        .expect("load oversized text")
        .expect("stored oversized text");
    assert_eq!(image.indexing.state, AttachmentIndexingState::Unsupported);
    assert_eq!(
        oversized_text.indexing.state,
        AttachmentIndexingState::Blocked
    );
}

#[test]
fn waiting_indexing_state_resumes_after_an_interrupted_worker_pass() {
    let database_path = test_database_path();
    let source_path = database_path.with_file_name("resume.txt");
    fs::write(&source_path, "Resume indexing readiness").expect("write source");
    let store = ConversationStore::initialize(database_path.clone()).expect("initialize store");
    let attachment_id = store
        .ingest_attachment(&source_path)
        .expect("ingest attachment")
        .id;
    store
        .process_attachment_extraction(&attachment_id)
        .expect("simulate extraction before interruption");
    drop(store);

    let reopened = ConversationStore::initialize(database_path).expect("reopen store");
    let completed = reopened
        .process_next_pending_attachment()
        .expect("resume waiting indexing state")
        .expect("waiting indexing row remains queued");

    assert_eq!(completed.indexing.state, AttachmentIndexingState::Indexable);
}

#[test]
fn indexing_readiness_survives_restart() {
    let database_path = test_database_path();
    let source_path = database_path.with_file_name("notes.txt");
    fs::write(&source_path, "Retained indexable text").expect("write source");
    let store = ConversationStore::initialize(database_path.clone()).expect("initialize store");
    let attachment_id = store
        .ingest_attachment(&source_path)
        .expect("ingest attachment")
        .id;
    store
        .process_next_pending_attachment()
        .expect("process pending attachment")
        .expect("one pending attachment");
    drop(store);

    let reopened = ConversationStore::initialize(database_path).expect("reopen store");
    let stored = reopened
        .stored_attachment_for_test(&attachment_id)
        .expect("load attachment")
        .expect("stored attachment");

    assert_eq!(stored.indexing.state, AttachmentIndexingState::Indexable);
}

#[test]
fn migration_maps_existing_extraction_outcomes_without_exposing_text() {
    let path = test_database_path();
    let ready_path = path.with_file_name("ready.txt");
    let pending_path = path.with_file_name("pending.txt");
    let unsupported_path = path.with_file_name("unsupported.png");
    let blocked_path = path.with_file_name("blocked.txt");
    fs::write(&ready_path, "Ready text").expect("write ready source");
    fs::write(&pending_path, "Pending text").expect("write pending source");
    fs::write(
        &unsupported_path,
        [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, b'x'],
    )
    .expect("write unsupported source");
    fs::write(&blocked_path, vec![b'b'; MAX_EXTRACTED_TEXT_BYTES + 1])
        .expect("write blocked source");
    let store = ConversationStore::initialize(path).expect("initialize store");
    let ready = store
        .ingest_attachment(&ready_path)
        .expect("ingest ready source");
    let pending = store
        .ingest_attachment(&pending_path)
        .expect("ingest pending source");
    let unsupported = store
        .ingest_attachment(&unsupported_path)
        .expect("ingest unsupported source");
    let blocked = store
        .ingest_attachment(&blocked_path)
        .expect("ingest blocked source");
    for attachment_id in [&ready.id, &unsupported.id, &blocked.id] {
        store
            .process_attachment_extraction(attachment_id)
            .expect("prepare terminal extraction state");
    }
    let connection = store.open().expect("open store");
    connection
        .execute_batch("DROP TABLE attachment_text_indexing;")
        .expect("remove current indexing table");
    connection
        .execute("DELETE FROM schema_migrations WHERE version = 14", [])
        .expect("remove migration record");
    connection
        .pragma_update(None, "user_version", 13)
        .expect("set prior schema version");
    connection
        .execute_batch(MIGRATION_14)
        .expect("apply indexing migration directly");

    for (attachment_id, expected) in [
        (&ready.id, "indexable"),
        (&pending.id, "waiting_for_extraction"),
        (&unsupported.id, "unsupported"),
        (&blocked.id, "blocked"),
    ] {
        let state: String = connection
            .query_row(
                "SELECT state FROM attachment_text_indexing WHERE attachment_id = ?1",
                [attachment_id],
                |row| row.get(0),
            )
            .expect("load migrated indexing state");
        assert_eq!(state, expected);
    }

    let columns = connection
        .prepare("PRAGMA table_info(attachment_text_indexing)")
        .expect("prepare table inspection")
        .query_map([], |row| row.get::<_, String>(1))
        .expect("inspect indexing table")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect indexing columns");

    assert_eq!(columns, vec!["attachment_id", "state", "updated_at_ms"]);
}
