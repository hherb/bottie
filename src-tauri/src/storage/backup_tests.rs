//! Manual and automatic SQLite-backup contract tests.

use std::fs;

use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

use super::*;

/// Writes and ingests one attachment beside an isolated backup fixture.
fn ingest_backup_fixture(
    store: &ConversationStore,
    name: &str,
    bytes: &[u8],
) -> IngestedAttachment {
    let source = store.path.with_file_name(name);
    fs::write(&source, bytes).expect("attachment fixture should be written");
    let ingested = store
        .ingest_attachment(&source)
        .expect("attachment fixture should ingest");
    tests::completed_ingestion(store, ingested)
}

/// Encodes a small valid PNG so backup coverage includes a ready derivative.
fn backup_png_fixture() -> Vec<u8> {
    let pixels = vec![127_u8; 2 * 2 * 4];
    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(&pixels, 2, 2, ExtendedColorType::Rgba8)
        .expect("PNG fixture should encode");
    encoded
}

#[test]
fn creates_a_consistent_independently_readable_backup_without_changing_live_state() {
    let source_path = tests::test_database_path();
    let backup_path = source_path.with_file_name("bottie-backup.sqlite3");
    let store =
        ConversationStore::initialize(source_path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Backup snapshot")
        .expect("conversation should be created");
    store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id.clone(),
            role: StoredRole::User,
            text: "Retain committed WAL content".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("message should append");

    store
        .backup_to(&backup_path)
        .expect("online backup should complete");

    let backup = Connection::open(&backup_path).expect("backup should open independently");
    let integrity: String = backup
        .pragma_query_value(None, "quick_check", |row| row.get(0))
        .expect("backup integrity should be readable");
    let backed_up_text: String = backup
        .query_row(
            "SELECT message_blocks.text_content
             FROM message_blocks JOIN messages ON messages.id = message_blocks.message_id
             WHERE messages.conversation_id = ?1 AND message_blocks.block_type = 'text'",
            [&conversation.id],
            |row| row.get(0),
        )
        .expect("committed message should exist in the backup");
    let selected = store
        .load_last_open_conversation()
        .expect("live selection should load")
        .expect("live conversation should remain selected");

    assert_ne!(source_path, backup_path);
    assert_eq!(integrity, "ok");
    assert_eq!(backed_up_text, "Retain committed WAL content");
    assert_eq!(selected.id, conversation.id);
}

#[test]
fn embeds_and_verifies_original_attachment_blobs_and_ready_derivatives() {
    let source_path = tests::test_database_path();
    let backup_path = source_path.with_file_name("bottie-portable-backup.sqlite3");
    let store = ConversationStore::initialize(source_path).expect("storage should initialize");
    let document = ingest_backup_fixture(&store, "notes.txt", b"portable attachment notes");
    let image_bytes = backup_png_fixture();
    let image = ingest_backup_fixture(&store, "diagram.png", &image_bytes);
    let mut image_variant_bytes = image_bytes;
    image_variant_bytes.extend_from_slice(b"trailing source variation");
    let image_variant = ingest_backup_fixture(&store, "diagram-variant.png", &image_variant_bytes);
    let connection = store.open().expect("store should open");
    let normalized_sha256 = |attachment_id: &str| {
        connection
            .query_row(
                "SELECT normalized_sha256 FROM attachment_image_normalizations
                 WHERE attachment_id = ?1",
                [attachment_id],
                |row| row.get::<_, String>(0),
            )
            .expect("ready normalization should have an identity")
    };

    assert_ne!(image.id, image_variant.id);
    assert_eq!(
        normalized_sha256(&image.id),
        normalized_sha256(&image_variant.id)
    );
    drop(connection);

    store
        .backup_to(&backup_path)
        .expect("portable backup should complete");
    let copied_backup = backup_path
        .parent()
        .expect("backup should have a parent")
        .join("copied")
        .join("portable.sqlite3");
    fs::create_dir_all(copied_backup.parent().expect("copy should have a parent"))
        .expect("copy directory should be created");
    fs::copy(&backup_path, &copied_backup).expect("single backup file should copy independently");

    let backup = Connection::open(&copied_backup).expect("copied portable backup should open");
    let manifest: (String, i64) = backup
        .query_row(
            "SELECT format, version FROM bottie_portable_manifest",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("portable manifest should exist");
    let document_bytes: Vec<u8> = backup
        .query_row(
            "SELECT content FROM bottie_portable_attachment_blobs WHERE sha256 = ?1",
            [&document.sha256],
            |row| row.get(0),
        )
        .expect("document bytes should be embedded");
    let derivative: (String, Vec<u8>) = backup
        .query_row(
            "SELECT sha256, content FROM bottie_portable_image_derivatives",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("ready normalized derivative should be embedded");
    let derivative_count: i64 = backup
        .query_row(
            "SELECT COUNT(*) FROM bottie_portable_image_derivatives",
            [],
            |row| row.get(0),
        )
        .expect("deduplicated derivative count should be readable");

    assert_eq!(manifest, ("bottie-portable-backup".into(), 1));
    assert!(!sidecar_path(&backup_path, "-wal").exists());
    assert!(!sidecar_path(&backup_path, "-shm").exists());
    assert_eq!(document_bytes, b"portable attachment notes");
    assert_eq!(derivative_count, 1);
    assert_eq!(format!("{:x}", Sha256::digest(&derivative.1)), derivative.0);
    assert_eq!(
        image.normalization.byte_size,
        Some(derivative.1.len() as u64)
    );
}

/// Resolves one SQLite sidecar path without assuming a Unicode database path.
fn sidecar_path(database: &std::path::Path, suffix: &str) -> std::path::PathBuf {
    let mut sidecar = database.as_os_str().to_os_string();
    sidecar.push(suffix);
    sidecar.into()
}

#[test]
fn rejects_the_live_database_as_its_own_backup_destination() {
    let source_path = tests::test_database_path();
    let store =
        ConversationStore::initialize(source_path.clone()).expect("storage should initialize");

    let error = store
        .backup_to(&source_path)
        .expect_err("the live database path must be rejected");

    assert_eq!(error.code, "invalid_request");
    assert_eq!(
        error.message,
        "Choose a different location for the Bottie backup."
    );
}

#[test]
fn restores_a_valid_backup_after_preserving_the_live_store() {
    let live_path = tests::test_database_path();
    let backup_path = live_path.with_file_name("selected-backup.sqlite3");
    let safety_path = live_path.with_file_name("bottie-pre-restore.sqlite3");
    let live = ConversationStore::initialize(live_path).expect("live storage should initialize");
    let original = live
        .create_conversation("Current local conversation")
        .expect("live conversation should be created");
    let backup_source_path = live
        .path
        .with_file_name("backup-source")
        .join("bottie.sqlite3");
    let backup_source =
        ConversationStore::initialize(backup_source_path).expect("backup source should initialize");
    let restored = backup_source
        .create_conversation("Conversation from backup")
        .expect("backup conversation should be created");
    backup_source
        .backup_to(&backup_path)
        .expect("selected backup should be created");

    live.restore_from(&backup_path, &safety_path)
        .expect("valid backup should restore");

    let restored_conversations = live
        .list_conversations()
        .expect("restored conversations should list");
    let safety = ConversationStore::initialize(safety_path).expect("safety copy should reopen");
    let preserved_conversations = safety
        .list_conversations()
        .expect("preserved conversations should list");
    let restored_selection = live
        .load_last_open_conversation()
        .expect("restored selection should load")
        .expect("backup selection should be preserved");
    let preserved_selection = safety
        .load_last_open_conversation()
        .expect("safety selection should load")
        .expect("live selection should be preserved");
    assert_eq!(restored_conversations.len(), 1);
    assert_eq!(restored_conversations[0].id, restored.id);
    assert_eq!(restored_selection.id, restored.id);
    assert_eq!(preserved_conversations.len(), 1);
    assert_eq!(preserved_conversations[0].id, original.id);
    assert_eq!(preserved_selection.id, original.id);
}

#[test]
fn restores_portable_attachment_bytes_and_preserves_the_previous_bytes_in_the_safety_copy() {
    let live_path = tests::test_database_path();
    let backup_path = live_path.with_file_name("portable-selected-backup.sqlite3");
    let safety_path = live_path.with_file_name("portable-pre-restore.sqlite3");
    let live = ConversationStore::initialize(live_path).expect("live storage should initialize");
    let previous = ingest_backup_fixture(&live, "previous.txt", b"previous local bytes");
    let backup_source_path = live
        .path
        .with_file_name("portable-backup-source")
        .join("bottie.sqlite3");
    let backup_source =
        ConversationStore::initialize(backup_source_path).expect("backup source should initialize");
    let restored_attachment =
        ingest_backup_fixture(&backup_source, "restored.txt", b"restored portable bytes");
    let restored_conversation = backup_source
        .create_conversation("Portable restore")
        .expect("backup conversation should be created");
    backup_source
        .add_conversation_attachments(&restored_conversation.id, &[restored_attachment.id.clone()])
        .expect("restored attachment should enter conversation scope");
    backup_source
        .backup_to(&backup_path)
        .expect("portable backup should be created");

    live.restore_from(&backup_path, &safety_path)
        .expect("portable backup should restore");

    let restored = live
        .load_conversation(&restored_conversation.id)
        .expect("restored conversation should load");
    let safety = Connection::open(&safety_path).expect("portable safety copy should open");
    let preserved_bytes: Vec<u8> = safety
        .query_row(
            "SELECT content FROM bottie_portable_attachment_blobs WHERE sha256 = ?1",
            [&previous.sha256],
            |row| row.get(0),
        )
        .expect("previous attachment bytes should be preserved");

    assert_eq!(restored.attachments[0].id, restored_attachment.id);
    assert_eq!(
        fs::read(live.attachment_blob_path(&restored_attachment.sha256))
            .expect("restored attachment bytes should exist"),
        b"restored portable bytes"
    );
    assert_eq!(preserved_bytes, b"previous local bytes");
    assert!(!live.attachment_blob_path(&previous.sha256).exists());
}

#[test]
fn rejects_a_backup_with_tampered_portable_bytes_without_changing_live_state() {
    let live_path = tests::test_database_path();
    let backup_path = live_path.with_file_name("tampered-portable-backup.sqlite3");
    let safety_path = live_path.with_file_name("unused-pre-restore.sqlite3");
    let live = ConversationStore::initialize(live_path).expect("live storage should initialize");
    let original = live
        .create_conversation("Keep live state")
        .expect("live conversation should be created");
    let source_path = live
        .path
        .with_file_name("tampered-source")
        .join("bottie.sqlite3");
    let source = ConversationStore::initialize(source_path).expect("source should initialize");
    ingest_backup_fixture(&source, "tampered.txt", b"original portable bytes");
    source
        .backup_to(&backup_path)
        .expect("portable backup should complete");
    Connection::open(&backup_path)
        .expect("portable backup should open")
        .execute(
            "UPDATE bottie_portable_attachment_blobs SET content = X'00'",
            [],
        )
        .expect("portable payload should be tampered");

    let error = live
        .restore_from(&backup_path, &safety_path)
        .expect_err("tampered portable bytes must be rejected");

    assert_eq!(error.code, "invalid_request");
    assert_eq!(
        live.list_conversations()
            .expect("live conversations should list")[0]
            .id,
        original.id
    );
    assert!(!safety_path.exists());
}

#[test]
fn rejects_a_non_bottie_database_without_changing_the_live_store() {
    let live_path = tests::test_database_path();
    let invalid_path = live_path.with_file_name("unrelated.sqlite3");
    let safety_path = live_path.with_file_name("bottie-pre-restore.sqlite3");
    let live = ConversationStore::initialize(live_path).expect("live storage should initialize");
    let original = live
        .create_conversation("Keep this conversation")
        .expect("live conversation should be created");
    let unrelated = Connection::open(&invalid_path).expect("unrelated SQLite database should open");
    unrelated
        .execute("CREATE TABLE unrelated (value TEXT NOT NULL)", [])
        .expect("unrelated schema should be created");
    drop(unrelated);

    let error = live
        .restore_from(&invalid_path, &safety_path)
        .expect_err("an unrelated SQLite database must be rejected");

    let conversations = live
        .list_conversations()
        .expect("live conversations should still list");
    assert_eq!(error.code, "invalid_request");
    assert_eq!(error.message, "Choose a valid Bottie backup.");
    assert_eq!(conversations.len(), 1);
    assert_eq!(conversations[0].id, original.id);
    assert!(!safety_path.exists());
}

#[test]
fn creates_one_verified_automatic_backup_per_rolling_day() {
    const DAY_MS: i64 = 24 * 60 * 60 * 1_000;

    let live_path = tests::test_database_path();
    let store =
        ConversationStore::initialize(live_path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Automatic backup")
        .expect("conversation should be created");
    store
        .append_message(NewStoredMessage {
            conversation_id: conversation.id,
            role: StoredRole::User,
            text: "Keep this daily snapshot".into(),
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        })
        .expect("message should append");

    let first = store
        .rotate_automatic_backups_at(DAY_MS)
        .expect("first automatic backup should complete");
    let skipped = store
        .rotate_automatic_backups_at(DAY_MS * 2 - 1)
        .expect("a recent automatic backup should be reused");
    let second = store
        .rotate_automatic_backups_at(DAY_MS * 2)
        .expect("a full day should create another automatic backup");
    let paths = automatic_backup_paths(&live_path);
    let newest = Connection::open(paths.last().expect("a newest backup should exist"))
        .expect("automatic backup should open independently");
    let backed_up_text: String = newest
        .query_row(
            "SELECT text_content FROM message_blocks WHERE text_content = ?1",
            ["Keep this daily snapshot"],
            |row| row.get(0),
        )
        .expect("committed content should exist in the automatic backup");

    assert!(first.created);
    assert_eq!(first.retained, 1);
    assert!(!skipped.created);
    assert_eq!(skipped.retained, 1);
    assert!(second.created);
    assert_eq!(second.retained, 2);
    assert_eq!(paths.len(), 2);
    assert_eq!(backed_up_text, "Keep this daily snapshot");
}

#[test]
fn retains_seven_automatic_backups_without_pruning_other_files() {
    const DAY_MS: i64 = 24 * 60 * 60 * 1_000;
    const ROTATIONS: i64 = 9;

    let live_path = tests::test_database_path();
    let store =
        ConversationStore::initialize(live_path.clone()).expect("storage should initialize");
    let backup_directory = live_path
        .parent()
        .expect("test database should have a parent")
        .join("automatic-backups");
    fs::create_dir_all(&backup_directory).expect("automatic backup directory should exist");
    let unrelated = backup_directory.join("keep-me.sqlite3");
    let safety_copy = live_path.with_file_name("bottie-pre-restore-keep.sqlite3");
    fs::write(&unrelated, b"not managed by rotation").expect("unrelated file should be created");
    fs::write(&safety_copy, b"separate safety copy").expect("safety copy should be created");

    let mut final_outcome = None;
    for day in 1..=ROTATIONS {
        final_outcome = Some(
            store
                .rotate_automatic_backups_at(day * DAY_MS)
                .expect("automatic rotation should complete"),
        );
    }

    let outcome = final_outcome.expect("rotation should produce an outcome");
    let paths = automatic_backup_paths(&live_path);
    assert!(outcome.created);
    assert_eq!(outcome.retained, 7);
    assert_eq!(outcome.pruned, 1);
    assert_eq!(paths.len(), 7);
    assert!(unrelated.exists());
    assert!(safety_copy.exists());
}

/// Lists only automatic snapshots recognized by Bottie's filename contract.
fn automatic_backup_paths(live_path: &std::path::Path) -> Vec<std::path::PathBuf> {
    let directory = live_path
        .parent()
        .expect("test database should have a parent")
        .join("automatic-backups");
    let mut paths = fs::read_dir(directory)
        .expect("automatic backup directory should be readable")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("bottie-auto-") && name.ends_with(".sqlite3"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}
