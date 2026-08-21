use std::fs;

use super::{
    AttachmentExtractionFormat, AttachmentExtractionState, ConversationStore,
    image_normalization::ImageNormalizationState, tests::test_database_path,
};

#[test]
fn ingestion_stays_pending_until_one_background_pass_completes() {
    let database_path = test_database_path();
    let source_path = database_path.with_file_name("notes.md");
    fs::write(&source_path, "# Durable background work\n").expect("write source");
    let store = ConversationStore::initialize(database_path).expect("initialize store");

    let ingested = store
        .ingest_attachment(&source_path)
        .expect("ingest attachment");

    assert_eq!(
        ingested.extraction.state,
        AttachmentExtractionState::Pending
    );
    assert_eq!(
        ingested.normalization.state,
        ImageNormalizationState::Unsupported
    );

    let completed = store
        .process_next_pending_attachment()
        .expect("process pending attachment")
        .expect("one pending attachment");

    assert_eq!(completed.id, ingested.id);
    assert_eq!(completed.extraction.state, AttachmentExtractionState::Ready);
    assert_eq!(
        completed.extraction.format,
        Some(AttachmentExtractionFormat::Markdown)
    );
    assert!(
        store
            .process_next_pending_attachment()
            .expect("check drained queue")
            .is_none()
    );
}

#[test]
fn pending_work_survives_restart_without_blocking_initialization() {
    let database_path = test_database_path();
    let source_path = database_path.with_file_name("notes.txt");
    fs::write(&source_path, "Resume after restart").expect("write source");
    let store = ConversationStore::initialize(database_path.clone()).expect("initialize store");
    let attachment_id = store
        .ingest_attachment(&source_path)
        .expect("ingest attachment")
        .id;
    drop(store);

    let reopened = ConversationStore::initialize(database_path).expect("reopen store");
    let pending = reopened
        .stored_attachment_for_test(&attachment_id)
        .expect("load attachment")
        .expect("stored attachment");

    assert_eq!(pending.extraction.state, AttachmentExtractionState::Pending);
    let completed = reopened
        .process_next_pending_attachment()
        .expect("resume pending attachment")
        .expect("pending attachment remains queued");
    assert_eq!(completed.extraction.state, AttachmentExtractionState::Ready);
}
