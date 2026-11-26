//! Backups and re-ingestion when an attachment blob has gone missing from disk or cannot be used.
//!
//! Blobs can disappear outside Bottie's control (antivirus quarantine, a legacy restore that kept
//! database rows but not files). One missing file must not block every backup, and attaching the
//! same file again must put the missing blob back.

use std::fs;

use rusqlite::Connection;

use super::{
    portable_backup::{SkippedFiles, omitted_file_count},
    *,
};

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
    assert_eq!(
        rotation.skipped_files,
        SkippedFiles {
            missing: 1,
            damaged: 0
        }
    );
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

    assert_eq!(
        skipped,
        SkippedFiles {
            missing: 1,
            damaged: 0
        }
    );
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
    // A same-day relaunch creates nothing but still prunes, so it must keep the rescued snapshot too (#235).
    let relaunch = store
        .rotate_automatic_backups_at(15 * DAY_MS + 1)
        .expect("a same-day relaunch should succeed");
    assert!(!relaunch.created);
    assert_eq!((relaunch.retained, relaunch.pruned), (8, 0));

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

const DAY_MS: i64 = 24 * 60 * 60 * 1_000;

/// Builds seven complete daily snapshots, then seven that leave out a missing file, and returns the
/// store and the day-8 snapshot. Day 15 expires day 8 (incomplete) and day 7 (the newest complete).
fn store_whose_day_eight_snapshot_expires_next() -> (ConversationStore, std::path::PathBuf) {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let missing = ingest(&store, "missing.txt", b"bytes only old snapshots hold");
    for day in 1..=7 {
        store
            .rotate_automatic_backups_at(day * DAY_MS)
            .expect("complete rotation should succeed");
    }
    fs::remove_file(store.attachment_blob_path(&missing.sha256)).expect("blob should delete");
    for day in 8..=14 {
        store
            .rotate_automatic_backups_at(day * DAY_MS)
            .expect("rotation with a missing blob should succeed");
    }
    let day_eight = fs::read_dir(store.path.with_file_name("automatic-backups"))
        .expect("automatic backup directory should list")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&format!("bottie-auto-{}-", 8 * DAY_MS)))
        })
        .expect("the day-8 snapshot should exist");
    (store, day_eight)
}

#[cfg(unix)]
#[test]
fn rotation_keeps_and_names_an_expired_snapshot_it_cannot_open() {
    use std::os::unix::fs::PermissionsExt;

    let (store, day_eight) = store_whose_day_eight_snapshot_expires_next();
    fs::set_permissions(&day_eight, fs::Permissions::from_mode(0o000)).expect("lock snapshot");
    if fs::read(&day_eight).is_ok() {
        // Root ignores file permissions, so the open cannot be made to fail.
        return;
    }

    let rotation = store
        .rotate_automatic_backups_at(15 * DAY_MS)
        .expect("rotation with an unopenable snapshot should succeed");

    fs::set_permissions(&day_eight, fs::Permissions::from_mode(0o644)).expect("unlock snapshot");
    // A snapshot that cannot be opened now might open later and be the complete one (#259).
    assert_eq!(
        (rotation.retained, rotation.pruned, rotation.check_failed),
        (9, 0, 1)
    );
    assert_eq!(
        rotation.check_error.as_deref(),
        Some("Error code 14: unable to open database file")
    );
    assert_eq!(rotation.report().0, "warn");
    assert!(
        day_eight.exists(),
        "an unchecked snapshot must not be pruned"
    );
}

#[test]
fn rotation_prunes_an_expired_snapshot_that_is_not_a_database() {
    let (store, day_eight) = store_whose_day_eight_snapshot_expires_next();
    fs::write(&day_eight, b"not a sqlite database").expect("snapshot should be overwritten");

    let rotation = store
        .rotate_automatic_backups_at(15 * DAY_MS)
        .expect("rotation with an unreadable snapshot should succeed");

    // No check can ever read it, and restore would reject it, so it expires, but is named (#264).
    assert_eq!(
        (rotation.retained, rotation.pruned, rotation.check_failed),
        (8, 1, 0)
    );
    assert_eq!(rotation.unreadable_expired, 1);
    assert_eq!(
        rotation.unreadable_error.as_deref(),
        Some("Error code 26: file is not a database")
    );
    let (level, _, detail) = rotation.report();
    assert_eq!(level, "warn");
    assert!(!detail.contains("could not be checked"));
    assert!(detail.contains("1 expired snapshot(s) could not be read"));
    assert!(!day_eight.exists());
}

#[test]
fn rotation_checks_a_version_one_snapshot_older_than_the_generated_image_catalog() {
    let (store, day_eight) = store_whose_day_eight_snapshot_expires_next();
    // A snapshot from before 2026-09-13 has neither generated-image catalog table (#264).
    Connection::open(&day_eight)
        .expect("snapshot should open")
        .execute_batch(
            "UPDATE bottie_portable_manifest SET version = 1;
             DROP TABLE bottie_portable_generated_asset_blobs;
             DROP TABLE generated_asset_sources;
             DROP TABLE generated_assets;",
        )
        .expect("snapshot should be converted to the old schema");

    let rotation = store
        .rotate_automatic_backups_at(15 * DAY_MS)
        .expect("rotation with an old snapshot should succeed");

    // Its catalog still lists the missing file, so it is incomplete and expires: an answer, not a failed check.
    assert_eq!(
        (rotation.retained, rotation.pruned, rotation.check_failed),
        (8, 1, 0)
    );
    assert!(!day_eight.exists());
}

#[test]
fn an_empty_or_foreign_database_never_counts_as_a_complete_snapshot() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let empty = store.path.with_file_name("bottie-auto-empty.sqlite3");
    fs::write(&empty, b"").expect("empty snapshot should be written");
    let foreign = store.path.with_file_name("bottie-auto-foreign.sqlite3");
    Connection::open(&foreign)
        .expect("foreign database should open")
        .execute_batch("CREATE TABLE notes (body TEXT);")
        .expect("foreign table should be created");

    // With no catalog tables they list no files, but restore rejects them, so they are never "complete".
    for path in [empty, foreign] {
        let fault = omitted_file_count(&path).expect_err("a non-Bottie database must not pass");
        assert!(fault.is_permanent());
        assert_eq!(fault.cause(), "not a Bottie database");
    }
}

#[test]
fn a_blob_that_fails_its_checksum_is_left_out_and_counted() {
    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let damaged = ingest(&store, "damaged.txt", b"original bytes");
    let kept = ingest(&store, "kept.txt", b"bytes still intact");
    // Same length, different bytes: only the hash check notices the damage.
    fs::write(
        store.attachment_blob_path(&damaged.sha256),
        b"ORIGINAL BYTES",
    )
    .expect("blob should be overwritten");
    let destination = store.path.with_file_name("bottie-checksum.sqlite3");

    // One damaged file must not block every backup (#265), and it is counted apart from missing ones (#267).
    assert_eq!(
        store.backup_to(&destination).expect("backup should save"),
        SkippedFiles {
            missing: 0,
            damaged: 1
        }
    );
    let embedded: Vec<String> = Connection::open(&destination)
        .expect("backup should open")
        .prepare("SELECT sha256 FROM bottie_portable_attachment_blobs")
        .expect("payload should query")
        .query_map([], |row| row.get(0))
        .expect("payload rows should read")
        .collect::<Result<_, _>>()
        .expect("payload rows should decode");
    assert_eq!(embedded, vec![kept.sha256]);
    assert_eq!(omitted_file_count(&destination), Ok(1));
}

#[cfg(unix)]
#[test]
fn an_unreadable_blob_names_its_io_cause_and_leaves_no_staging() {
    use std::os::unix::fs::PermissionsExt;

    let store = ConversationStore::initialize(tests::test_database_path())
        .expect("storage should initialize");
    let locked = ingest(&store, "locked.txt", b"bytes behind a lock");
    let blob = store.attachment_blob_path(&locked.sha256);
    fs::set_permissions(&blob, fs::Permissions::from_mode(0o000)).expect("lock blob");
    if fs::read(&blob).is_ok() {
        // Root ignores file permissions, so the read cannot be made to fail.
        return;
    }
    let destination = store.path.with_file_name("bottie-locked.sqlite3");

    let error = store
        .backup_to(&destination)
        .expect_err("an unreadable blob must fail the backup");

    fs::set_permissions(&blob, fs::Permissions::from_mode(0o644)).expect("unlock blob");
    assert_eq!(
        error.message,
        "Bottie could not save the local data backup (permission denied)."
    );
    assert!(!destination.exists());
    let leftovers = fs::read_dir(destination.parent().expect("backup directory"))
        .expect("backup directory should list")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".part"))
        .count();
    assert_eq!(leftovers, 0, "a failed backup must not leave staging files");
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
        source.backup_to(&backup_path).expect("backup should save"),
        SkippedFiles {
            missing: 1,
            damaged: 0
        }
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

#[test]
fn a_snapshot_must_omit_exactly_the_files_the_backup_skipped() {
    let skipped = SkippedFiles {
        missing: 1,
        damaged: 2,
    };
    assert!(skipped.check_omitted(3).is_ok());
    // A row lost in the payload copy, or a skip the snapshot did not reflect, fails the backup.
    for omitted in [2, 4] {
        let error = skipped
            .check_omitted(omitted)
            .expect_err("a count mismatch must fail the backup");
        assert!(
            error
                .message
                .contains("the snapshot left out files the backup did not skip")
        );
    }
}
