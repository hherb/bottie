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
        skipped_files: 0,
    };
    assert_eq!(
        clean.report(),
        (
            "info",
            "Automatic backup created",
            "7 snapshot(s) retained; 1 expired snapshot(s) removed; \
             0 missing attachment or image file(s) left out"
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

#[test]
fn an_expired_snapshot_whose_check_fails_is_kept_and_counted() {
    // Newest first: an incomplete one, one that cannot be read, the newest complete one, and
    // older snapshots that the complete one supersedes.
    let mut expired = Vec::from(
        [
            "partial",
            "unreadable",
            "complete",
            "older-unreadable",
            "oldest",
        ]
        .map(|name| ManagedBackup {
            timestamp_ms: 0,
            path: PathBuf::from(name),
        }),
    );

    let (kept, check_failed) = hold_back_from_expiry(&mut expired, |path| match path.to_str() {
        Some("partial") => Ok(2),
        Some("complete" | "oldest") => Ok(0),
        _ => Err(()),
    });

    let names = |backups: &[ManagedBackup]| -> Vec<String> {
        backups
            .iter()
            .map(|backup| backup.path.display().to_string())
            .collect()
    };
    assert_eq!(names(&kept), ["unreadable", "complete"]);
    assert_eq!(check_failed, 1);
    assert_eq!(names(&expired), ["partial", "older-unreadable", "oldest"]);

    let report = AutomaticBackupRotation {
        created: false,
        retained: 9,
        pruned: 3,
        prune_failed: 0,
        prune_error: None,
        check_failed,
        skipped_files: 0,
    }
    .report();
    assert_eq!(report.0, "warn");
    assert!(report.2.ends_with(
        "1 expired snapshot(s) kept because they could not be checked for missing files; \
         later launches check them again until a retained snapshot is complete"
    ));
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
