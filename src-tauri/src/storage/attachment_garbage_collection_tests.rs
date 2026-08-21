//! Native attachment garbage-collection contract tests.

use std::fs;

use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};

use super::{
    ConversationStore, MessageState, NewStoredMessage, StoredRole,
    tests::{completed_ingestion, test_database_path},
};

/// Writes and fully processes one attachment fixture beside the isolated test store.
fn ingest_fixture(
    store: &ConversationStore,
    name: &str,
    bytes: &[u8],
) -> super::IngestedAttachment {
    let source_path = store.path.with_file_name(name);
    fs::write(&source_path, bytes).expect("attachment fixture should be written");
    let ingested = store
        .ingest_attachment(&source_path)
        .expect("attachment fixture should ingest");
    completed_ingestion(store, ingested)
}

/// Encodes a small valid PNG so garbage collection must account for a ready derivative.
fn png_fixture(value: u8) -> Vec<u8> {
    let pixels = vec![value; 2 * 2 * 4];
    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(&pixels, 2, 2, ExtendedColorType::Rgba8)
        .expect("PNG fixture should encode");
    encoded
}

#[test]
fn collects_unreferenced_catalog_content_and_restart_debris() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let orphan = ingest_fixture(&store, "orphan.png", &png_fixture(127));
    let retained = ingest_fixture(&store, "retained.txt", b"Durable retained context");
    let conversation = store
        .create_conversation("Retained attachment")
        .expect("conversation should be created");
    store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation.id,
                role: StoredRole::User,
                text: "Keep this file".into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[retained.id.clone()],
        )
        .expect("retained attachment should be associated");

    let orphan_blob = store.attachment_blob_path(&orphan.sha256);
    let orphan_derivative = store
        .normalized_image_path(
            &store
                .normalized_sha256_for_test(&orphan.id)
                .expect("derivative identity should load")
                .expect("orphan should have a derivative"),
            orphan
                .normalization
                .format
                .expect("orphan format should exist"),
        )
        .expect("derivative path should resolve");
    let retained_blob = store.attachment_blob_path(&retained.sha256);
    let stale_hash = "a".repeat(64);
    let stale_blob = store.attachment_blob_path(&stale_hash);
    fs::create_dir_all(
        stale_blob
            .parent()
            .expect("stale blob should have a parent"),
    )
    .expect("stale shard should be created");
    fs::write(&stale_blob, b"restart debris").expect("stale blob should be written");
    let temporary = store.attachment_root().join("temporary/interrupted.part");
    fs::create_dir_all(
        temporary
            .parent()
            .expect("temporary file should have a parent"),
    )
    .expect("temporary directory should be created");
    fs::write(&temporary, b"partial").expect("temporary file should be written");
    let unmanaged = store.attachment_root().join("blobs/README.txt");
    fs::write(&unmanaged, b"leave unexpected files alone")
        .expect("unmanaged fixture should be written");

    let outcome = store
        .collect_all_unreferenced_attachments_for_test()
        .expect("garbage collection should succeed");

    assert_eq!(outcome.catalog_entries_removed, 1);
    assert_eq!(outcome.original_files_removed, 2);
    assert_eq!(outcome.derivative_files_removed, 1);
    assert_eq!(outcome.temporary_files_removed, 1);
    assert!(
        store
            .stored_attachment_for_test(&orphan.id)
            .expect("catalog should remain readable")
            .is_none()
    );
    assert!(
        store
            .stored_attachment_for_test(&retained.id)
            .expect("retained catalog should remain readable")
            .is_some()
    );
    assert!(!orphan_blob.exists());
    assert!(!orphan_derivative.exists());
    assert!(!stale_blob.exists());
    assert!(!temporary.exists());
    assert!(retained_blob.is_file());
    assert!(unmanaged.is_file());
}

#[test]
fn preserves_a_derivative_still_shared_by_retained_content() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let retained = ingest_fixture(&store, "retained.png", &png_fixture(63));
    let orphan = ingest_fixture(&store, "orphan.png", &png_fixture(191));
    let conversation = store
        .create_conversation("Shared derivative")
        .expect("conversation should be created");
    store
        .add_conversation_attachments(&conversation.id, &[retained.id.clone()])
        .expect("retained image should be associated");
    let retained_sha256 = store
        .normalized_sha256_for_test(&retained.id)
        .expect("retained derivative identity should load")
        .expect("retained derivative should exist");
    let orphan_sha256 = store
        .normalized_sha256_for_test(&orphan.id)
        .expect("orphan derivative identity should load")
        .expect("orphan derivative should exist");
    let retained_path = store
        .normalized_image_path(
            &retained_sha256,
            retained
                .normalization
                .format
                .expect("retained format should exist"),
        )
        .expect("retained derivative path should resolve");
    let orphan_path = store
        .normalized_image_path(
            &orphan_sha256,
            orphan
                .normalization
                .format
                .expect("orphan format should exist"),
        )
        .expect("orphan derivative path should resolve");
    let connection = store.open().expect("database should open");
    connection
        .execute(
            "UPDATE attachment_image_normalizations
             SET normalized_sha256 = ?1 WHERE attachment_id = ?2",
            rusqlite::params![retained_sha256, orphan.id],
        )
        .expect("fixture should share the retained derivative identity");
    drop(connection);

    let outcome = store
        .collect_all_unreferenced_attachments_for_test()
        .expect("garbage collection should succeed");

    assert_eq!(outcome.catalog_entries_removed, 1);
    assert!(retained_path.is_file());
    assert!(!orphan_path.exists());
    assert!(
        store
            .normalized_image_bytes_for_test(&retained.id)
            .expect("retained derivative should remain readable")
            .is_some()
    );
}

#[test]
fn preserves_attachments_referenced_only_by_recoverable_trash_or_conversation_scope() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let message_attachment = ingest_fixture(&store, "trashed.txt", b"Recoverable message context");
    let conversation_attachment =
        ingest_fixture(&store, "shared.txt", b"Recoverable conversation context");
    let conversation = store
        .create_conversation("Recoverable attachment context")
        .expect("conversation should be created");
    store
        .add_conversation_attachments(&conversation.id, &[conversation_attachment.id.clone()])
        .expect("conversation attachment should be associated");
    store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation.id.clone(),
                role: StoredRole::User,
                text: "Keep this in Trash".into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[message_attachment.id.clone()],
        )
        .expect("message attachment should be associated");
    store
        .delete_conversation(&conversation.id)
        .expect("conversation should move to Trash");

    let outcome = store
        .collect_all_unreferenced_attachments_for_test()
        .expect("garbage collection should succeed");

    assert_eq!(outcome.catalog_entries_removed, 0);
    assert!(
        store
            .attachment_blob_path(&message_attachment.sha256)
            .is_file()
    );
    assert!(
        store
            .attachment_blob_path(&conversation_attachment.sha256)
            .is_file()
    );
    store
        .restore_conversation(&conversation.id)
        .expect("conversation should remain restorable");
    let restored = store
        .load_conversation(&conversation.id)
        .expect("restored conversation should load");
    assert_eq!(restored.attachments[0].id, conversation_attachment.id);
    assert_eq!(
        restored.messages[0].attachments[0].id,
        message_attachment.id
    );
}

#[test]
fn preserves_recent_unreferenced_content_for_the_cross_process_safety_window() {
    let store =
        ConversationStore::initialize(test_database_path()).expect("storage should initialize");
    let recent = ingest_fixture(&store, "recent.txt", b"Potential draft in another process");
    let blob = store.attachment_blob_path(&recent.sha256);

    let outcome = store
        .collect_unreferenced_attachments()
        .expect("garbage collection should succeed");

    assert_eq!(outcome.catalog_entries_removed, 0);
    assert!(blob.is_file());
    assert!(
        store
            .stored_attachment_for_test(&recent.id)
            .expect("recent catalog should remain readable")
            .is_some()
    );
}
