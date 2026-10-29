//! Backups and re-ingestion when an attachment blob has gone missing from disk.
//!
//! Blobs can disappear outside Bottie's control (antivirus quarantine, a legacy restore that kept
//! database rows but not files). One missing file must not block every backup, and attaching the
//! same file again must put the missing blob back.

use std::fs;

use rusqlite::Connection;

use super::*;

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
        .expect("a missing blob must not block a manual backup");

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
