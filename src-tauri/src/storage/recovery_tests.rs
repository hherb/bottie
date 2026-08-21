//! Corrupt-store detection and guided-recovery contract tests.

use std::fs;

use super::*;

#[test]
fn corrupt_startup_enters_recovery_without_exposing_the_store_to_commands() {
    let path = tests::test_database_path();
    fs::write(&path, b"not a sqlite database").expect("corrupt fixture should be written");

    let startup = ConversationStore::initialize_for_app(path)
        .expect("corruption should become a recoverable startup state");
    let status = startup
        .store
        .recovery_status()
        .expect("recovery status should load");
    let error = startup
        .store
        .list_conversations()
        .expect_err("conversation access must remain paused");

    assert!(startup.recovery_required);
    assert_eq!(
        status.state,
        recovery::StorageRecoveryState::RecoveryRequired
    );
    assert_eq!(status.automatic_backup_count, 0);
    assert_eq!(status.latest_automatic_backup_at_ms, None);
    assert_eq!(error.code, "recovery_required");
}

#[test]
fn recovery_status_counts_only_verified_managed_automatic_backups() {
    const SNAPSHOT_AT_MS: i64 = 86_400_000;

    let live_path = tests::test_database_path();
    let store =
        ConversationStore::initialize(live_path.clone()).expect("storage should initialize");
    store
        .create_conversation("Verified recovery point")
        .expect("conversation should be created");
    store
        .rotate_automatic_backups_at(SNAPSHOT_AT_MS)
        .expect("automatic backup should be created");
    let automatic_directory = live_path
        .parent()
        .expect("live database should have a parent")
        .join("automatic-backups");
    fs::write(
        automatic_directory.join(format!(
            "bottie-auto-{}-{}.sqlite3",
            SNAPSHOT_AT_MS + 1,
            uuid::Uuid::new_v4()
        )),
        b"not sqlite",
    )
    .expect("invalid managed-looking backup should be written");
    drop(store);
    super::backup::remove_database_files(&live_path);
    fs::write(&live_path, b"damaged live store").expect("live store should be corrupted");

    let startup = ConversationStore::initialize_for_app(live_path)
        .expect("corruption should become a recoverable startup state");
    let status = startup
        .store
        .recovery_status()
        .expect("recovery status should load");

    assert_eq!(status.automatic_backup_count, 1);
    assert_eq!(status.latest_automatic_backup_at_ms, Some(SNAPSHOT_AT_MS));
}

#[test]
fn latest_automatic_backup_recovers_corrupt_store_and_preserves_damaged_bytes() {
    const SNAPSHOT_AT_MS: i64 = 86_400_000;

    let live_path = tests::test_database_path();
    let store =
        ConversationStore::initialize(live_path.clone()).expect("storage should initialize");
    let conversation = store
        .create_conversation("Return after recovery")
        .expect("conversation should be created");
    let attachment_source = live_path.with_file_name("recovery-notes.txt");
    fs::write(&attachment_source, b"portable recovery attachment")
        .expect("attachment source should be written");
    let attachment = tests::completed_ingestion(
        &store,
        store
            .ingest_attachment(&attachment_source)
            .expect("recovery attachment should ingest"),
    );
    store
        .add_conversation_attachments(&conversation.id, &[attachment.id.clone()])
        .expect("recovery attachment should enter conversation scope");
    store
        .rotate_automatic_backups_at(SNAPSHOT_AT_MS)
        .expect("automatic backup should be created");
    fs::write(
        store.attachment_blob_path(&attachment.sha256),
        b"damaged attachment bytes",
    )
    .expect("live attachment bytes should be damaged after the snapshot");
    drop(store);
    super::backup::remove_database_files(&live_path);
    let damaged_bytes = b"damaged live store";
    let damaged_wal = b"damaged wal";
    let damaged_shm = b"damaged shared memory";
    fs::write(&live_path, damaged_bytes).expect("live store should be corrupted");
    fs::write(sidecar_path(&live_path, "-wal"), damaged_wal)
        .expect("damaged WAL should be written");
    fs::write(sidecar_path(&live_path, "-shm"), damaged_shm)
        .expect("damaged shared memory should be written");

    let startup = ConversationStore::initialize_for_app(live_path)
        .expect("corruption should become a recoverable startup state");
    let observed_wal = fs::read(sidecar_path(&startup.store.path, "-wal"))
        .expect("startup WAL should remain available");
    let observed_shm = fs::read(sidecar_path(&startup.store.path, "-shm"))
        .expect("startup shared memory should remain available");
    let preservation = startup
        .store
        .restore_preservation_path()
        .expect("a private preservation target should be available");
    let restored_at_ms = startup
        .store
        .restore_latest_automatic_backup(&preservation)
        .expect("the verified automatic backup should restore");
    let status = startup
        .store
        .recovery_status()
        .expect("recovered status should load");
    let conversations = startup
        .store
        .list_conversations()
        .expect("conversation access should resume");
    let recovered_conversation = startup
        .store
        .open_conversation(&conversation.id)
        .expect("recovered conversation should reopen");
    let preserved_live = preservation.join("bottie.sqlite3");
    let restored_attachment = startup.store.attachment_blob_path(&attachment.sha256);
    let preserved_attachment = preservation
        .join("attachments")
        .join("blobs")
        .join(&attachment.sha256[..2])
        .join(&attachment.sha256);

    assert_eq!(restored_at_ms, SNAPSHOT_AT_MS);
    assert_eq!(status.state, recovery::StorageRecoveryState::Ready);
    assert_eq!(conversations[0].id, conversation.id);
    assert_eq!(recovered_conversation.attachments[0].id, attachment.id);
    assert_eq!(
        fs::read(restored_attachment).expect("portable attachment should recover"),
        b"portable recovery attachment"
    );
    assert_eq!(
        fs::read(preserved_attachment).expect("damaged attachment should be preserved"),
        b"damaged attachment bytes"
    );
    assert_eq!(
        fs::read(preserved_live).expect("damaged database should be preserved"),
        damaged_bytes
    );
    assert_eq!(
        fs::read(preservation.join("bottie.sqlite3-wal")).expect("damaged WAL should be preserved"),
        observed_wal
    );
    assert_eq!(
        fs::read(preservation.join("bottie.sqlite3-shm"))
            .expect("damaged shared memory should be preserved"),
        observed_shm
    );
}

/// Resolves one SQLite sidecar path without assuming the database path is Unicode.
fn sidecar_path(database: &std::path::Path, suffix: &str) -> std::path::PathBuf {
    let mut sidecar = database.as_os_str().to_os_string();
    sidecar.push(suffix);
    sidecar.into()
}
