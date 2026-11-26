//! Unit tests for automatic-backup rotation helpers; a child module so it reaches private helpers.

use super::*;

#[test]
fn report_warns_and_names_failed_prunes_only_when_there_are_any() {
    let clean = AutomaticBackupRotation {
        created: true,
        retained: 7,
        pruned: 1,
        prune_failed: 0,
        prune_error: None,
        check_failed: 0,
        check_error: None,
        unreadable_expired: 0,
        unreadable_error: None,
        skipped_files: 0,
    };
    assert_eq!(
        clean.report(),
        (
            "info",
            "Automatic backup created",
            "7 snapshot(s) retained; 1 expired snapshot(s) removed; \
             0 missing or damaged attachment or image file(s) left out"
                .to_owned()
        )
    );
    let stuck = AutomaticBackupRotation {
        pruned: 0,
        prune_failed: 1,
        prune_error: Some(std::io::ErrorKind::PermissionDenied),
        ..clean
    };
    let (level, _, detail) = stuck.report();
    assert_eq!(level, "warn");
    assert!(detail.contains(
        "1 expired snapshot(s) could not be removed (permission denied); the next launch retries them"
    ));
}

#[test]
fn a_current_rotation_reports_info_unless_a_stuck_prune_remains() {
    let current = AutomaticBackupRotation {
        created: false,
        retained: 7,
        pruned: 0,
        prune_failed: 0,
        prune_error: None,
        check_failed: 0,
        check_error: None,
        unreadable_expired: 0,
        unreadable_error: None,
        skipped_files: 0,
    };
    assert_eq!(
        current.report(),
        (
            "info",
            "Automatic backups current",
            "7 snapshot(s) retained; snapshots run at most once every 24 hours".to_owned()
        )
    );
    let stuck = AutomaticBackupRotation {
        prune_failed: 1,
        prune_error: Some(std::io::ErrorKind::ResourceBusy),
        ..current
    };
    let (level, event, detail) = stuck.report();
    assert_eq!((level, event), ("warn", "Automatic backups current"));
    assert!(detail.ends_with(
        "1 expired snapshot(s) could not be removed (resource busy); the next launch retries them"
    ));
}

/// A SQLite failure with `code`, carrying free text that names a path the cause must never show.
fn sqlite_fault(code: std::ffi::c_int) -> PayloadFault {
    PayloadFault::Sqlite(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(code),
        Some("cannot read /Users/someone/bottie-auto.sqlite3".to_owned()),
    ))
}

#[test]
fn an_expired_snapshot_whose_check_fails_is_kept_counted_and_named() {
    // Newest first: an incomplete one, a corrupt one, one that is busy, one that is not a database,
    // one that cannot be opened, the newest complete one, and older snapshots it supersedes.
    let mut expired = Vec::from(
        [
            "partial",
            "corrupt",
            "busy",
            "not-a-database",
            "cannot-open",
            "complete",
            "older-busy",
            "oldest",
        ]
        .map(|name| ManagedBackup {
            timestamp_ms: 0,
            path: PathBuf::from(name),
        }),
    );

    let check = hold_back_from_expiry(&mut expired, |path| match path.to_str() {
        Some("partial") => Ok(2),
        Some("complete" | "oldest") => Ok(0),
        Some("corrupt") => Err(sqlite_fault(rusqlite::ffi::SQLITE_CORRUPT)),
        Some("not-a-database") => Err(sqlite_fault(rusqlite::ffi::SQLITE_NOTADB)),
        Some("cannot-open") => Err(sqlite_fault(rusqlite::ffi::SQLITE_CANTOPEN)),
        _ => Err(sqlite_fault(rusqlite::ffi::SQLITE_BUSY)),
    });

    let names = |backups: &[ManagedBackup]| -> Vec<String> {
        backups
            .iter()
            .map(|backup| backup.path.display().to_string())
            .collect()
    };
    // Corrupt and not-a-database snapshots can never be read or restored, so they expire, counted (#264).
    assert_eq!(names(&check.kept), ["busy", "cannot-open", "complete"]);
    assert_eq!(check.failed, 2);
    assert_eq!(
        check.failed_cause.as_deref(),
        Some("Error code 5: database is locked")
    );
    assert_eq!(check.unreadable, 2);
    assert_eq!(
        check.unreadable_cause.as_deref(),
        Some("Error code 11: database disk image is malformed")
    );
    assert_eq!(
        names(&expired),
        [
            "partial",
            "corrupt",
            "not-a-database",
            "older-busy",
            "oldest"
        ]
    );

    let report = AutomaticBackupRotation {
        created: false,
        retained: 9,
        pruned: 3,
        prune_failed: 0,
        prune_error: None,
        check_failed: check.failed,
        check_error: check.failed_cause,
        unreadable_expired: check.unreadable,
        unreadable_error: check.unreadable_cause,
        skipped_files: 0,
    }
    .report();
    assert_eq!(report.0, "warn");
    assert!(report.2.ends_with(
        "2 expired snapshot(s) kept because they could not be checked for missing files \
         (Error code 5: database is locked); later launches check them again until a retained \
         snapshot is complete; 2 expired snapshot(s) could not be read (Error code 11: database \
         disk image is malformed), so they expired unchecked; a damaged backup can mean a failing disk"
    ));
}

#[test]
fn a_payload_fault_names_sqlites_code_and_never_its_free_text() {
    // #263: backup sites keep the code, so a full disk reads differently from bad content.
    assert_eq!(
        sqlite_fault(rusqlite::ffi::SQLITE_FULL)
            .backup_error()
            .message,
        "Bottie could not save the local data backup (Error code 13: database or disk is full)."
    );
    assert_eq!(
        PayloadFault::Invalid.backup_error().message,
        "Bottie could not save the local data backup (the embedded files failed validation)."
    );
    assert_eq!(
        PayloadFault::Sqlite(rusqlite::Error::InvalidQuery).cause(),
        "an unexpected database error"
    );
    // Restore keeps its one stable message for every fault.
    assert_eq!(
        StorageError::from(sqlite_fault(rusqlite::ffi::SQLITE_BUSY)),
        StorageError::invalid_backup()
    );
}

#[test]
fn a_failed_prune_is_counted_and_the_rest_are_still_removed() {
    let expired = ["a", "b", "c", "gone"].map(|name| ManagedBackup {
        timestamp_ms: 0,
        path: PathBuf::from(name),
    });
    let mut attempted = Vec::new();

    let (failed, first_error) = remove_expired(&expired, |path| {
        attempted.push(path.to_owned());
        if path == Path::new("b") {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        } else if path == Path::new("c") {
            Err(std::io::Error::from(std::io::ErrorKind::ResourceBusy))
        } else if path == Path::new("gone") {
            Err(std::io::Error::from(std::io::ErrorKind::NotFound))
        } else {
            Ok(())
        }
    });

    assert_eq!(failed, 2);
    assert_eq!(first_error, Some(std::io::ErrorKind::PermissionDenied));
    assert_eq!(attempted.len(), 4);
}

#[test]
fn folder_warning_names_unreadable_items_or_an_incomplete_listing() {
    assert_eq!(backup_folder_warning(0, false), None);
    assert_eq!(
        backup_folder_warning(1, false).as_deref(),
        Some(
            "1 item(s) in the automatic-backup folder could not be read, so retention and \
             recovery skip them; later launches check again"
        )
    );
    assert_eq!(
        backup_folder_warning(1, true).as_deref(),
        Some(
            "The automatic-backup folder could not be read in full, so retention and recovery \
             may miss snapshots; later launches check again"
        )
    );
}
