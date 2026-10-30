//! Backups and re-ingestion when an attachment blob has gone missing from disk.
//!
//! Blobs can disappear outside Bottie's control (antivirus quarantine, a legacy restore that kept
//! database rows but not files). One missing file must not block every backup, and attaching the
//! same file again must put the missing blob back.

use std::fs;

use rusqlite::Connection;

use super::{portable_backup::omitted_file_count, *};

/// Writes and ingests one attachment beside an isolated store.
fn ingest(store: &ConversationStore, name: &str, bytes: &[u8]) -> IngestedAttachment {
    let source = store.path.with_file_name(name);
    fs::write(&source, bytes).expect("attachment fixture should be written");
    let ingested = store
        .ingest_attachment(&source)
        .expect("attachment fixture should ingest");
    tests::completed_ingestion(store, ingested)
}

#[test]
fn automatic_backup_skips_a_missing_blob_and_reports_it() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let missing = ingest(&store, "missing.txt", b"quarantined bytes");
    let kept = ingest(&store, "kept.txt", b"bytes still on disk");
    fs::remove_file(store.attachment_blob_path(&missing.sha256)).expect("blob should delete");

    let rotation = store
        .rotate_automatic_backups_at(24 * 60 * 60 * 1_000)
        .expect("a missing blob must not block the automatic backup");

    assert!(rotation.created);
    assert_eq!(rotation.skipped_files, 1);
    let backup_path = fs::read_dir(store.path.with_file_name("automatic-backups"))
        .expect("automatic backup directory should list")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "sqlite3"))
        .expect("automatic backup should exist");
    let embedded: Vec<String> = Connection::open(backup_path)
        .expect("backup should open")
        .prepare("SELECT sha256 FROM bottie_portable_attachment_blobs")
        .expect("payload should query")
        .query_map([], |row| row.get(0))
        .expect("payload rows should read")
        .collect::<Result<_, _>>()
        .expect("payload rows should decode");
    assert_eq!(embedded, vec![kept.sha256]);
}

#[test]
fn manual_backup_reports_skipped_blobs() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let missing = ingest(&store, "missing.txt", b"quarantined bytes");
    fs::remove_file(store.attachment_blob_path(&missing.sha256)).expect("blob should delete");

    let skipped = store
        .backup_to(&store.path.with_file_name("manual.sqlite3"))
        .expect("a missing blob must not block a manual backup")
        .skipped_files;

    assert_eq!(skipped, 1);
}

#[test]
fn reattaching_identical_bytes_restores_a_missing_blob() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let first = ingest(&store, "notes.txt", b"bytes to repair");
    let blob = store.attachment_blob_path(&first.sha256);
    fs::remove_file(&blob).expect("blob should delete");

    let source = store.path.with_file_name("notes-again.txt");
    fs::write(&source, b"bytes to repair").expect("source should be written");
    let again = store
        .ingest_attachment(&source)
        .expect("identical bytes should ingest");

    assert!(again.duplicate);
    assert_eq!(
        fs::read(&blob).expect("blob should exist again"),
        b"bytes to repair"
    );
}

#[test]
fn rotation_keeps_the_newest_complete_snapshot_while_files_are_missing() {
    const DAY_MS: i64 = 24 * 60 * 60 * 1_000;
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let missing = ingest(&store, "missing.txt", b"bytes only old snapshots hold");
    for day in 1..=7 {
        store
            .rotate_automatic_backups_at(day * DAY_MS)
            .expect("complete rotation should succeed");
    }
    fs::remove_file(store.attachment_blob_path(&missing.sha256)).expect("blob should delete");

    for day in 8..=15 {
        store
            .rotate_automatic_backups_at(day * DAY_MS)
            .expect("rotation with a missing blob should succeed");
    }

    let snapshots: Vec<_> = fs::read_dir(store.path.with_file_name("automatic-backups"))
        .expect("automatic backup directory should list")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    assert_eq!(
        snapshots.len(),
        8,
        "seven newest plus the last complete snapshot"
    );
    let complete = snapshots
        .iter()
        .filter(|path| omitted_file_count(path) == Ok(0))
        .count();
    assert_eq!(complete, 1);
}

#[test]
fn restoring_a_backup_that_left_out_a_file_succeeds_and_counts_it() {
    let source = ConversationStore::initialize(tests::test_database_path())
        .expect("source storage should initialize");
    let missing = ingest(&source, "missing.txt", b"quarantined bytes");
    let kept = ingest(&source, "kept.txt", b"bytes still on disk");
    let conversation = source
        .create_conversation("Partial backup")
        .expect("conversation should be created");
    source
        .add_conversation_attachments(&conversation.id, &[missing.id.clone(), kept.id.clone()])
        .expect("attachments should enter conversation scope");
    fs::remove_file(source.attachment_blob_path(&missing.sha256)).expect("blob should delete");
    let backup_path = source.path.with_file_name("partial.sqlite3");
    assert_eq!(
        source
            .backup_to(&backup_path)
            .expect("backup should save")
            .skipped_files,
        1
    );

    let live = ConversationStore::initialize(tests::test_database_path())
        .expect("live storage should initialize");
    live.restore_from(
        &backup_path,
        &live.path.with_file_name("pre-restore.sqlite3"),
    )
    .expect("a backup that left out a missing file should restore");

    assert_eq!(
        live.load_conversation(&conversation.id)
            .expect("restored conversation should load")
            .attachments
            .len(),
        2
    );
    assert_eq!(
        fs::read(live.attachment_blob_path(&kept.sha256)).expect("kept blob should restore"),
        b"bytes still on disk"
    );
    assert_eq!(
        live.missing_content_files()
            .expect("missing files should count"),
        1
    );
}

#[test]
fn reattaching_repairs_a_damaged_blob_and_requeues_its_failed_extraction() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let source = store.path.with_file_name("notes.txt");
    fs::write(&source, b"bytes to repair").expect("source should be written");
    let first = store
        .ingest_attachment(&source)
        .expect("attachment should ingest");
    let blob = store.attachment_blob_path(&first.sha256);
    fs::remove_file(&blob).expect("blob should delete");
    let failed = tests::completed_ingestion(&store, first);
    assert_eq!(
        failed.extraction.error_code.as_deref(),
        Some("missing_content")
    );
    // Same length, different bytes: only a hash check notices the damage.
    fs::write(&blob, b"BYTES TO REPAIR").expect("damaged blob should be written");

    let again = store
        .ingest_attachment(&source)
        .expect("identical bytes should ingest");
    assert!(again.duplicate);
    assert_eq!(again.extraction.state, AttachmentExtractionState::Pending);
    assert_eq!(
        fs::read(&blob).expect("blob should be repaired"),
        b"bytes to repair"
    );

    let processed = tests::completed_ingestion(&store, again);
    assert_eq!(processed.extraction.state, AttachmentExtractionState::Ready);
}
