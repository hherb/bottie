//! Removes staging files and directories left behind when the app was killed mid-automatic-backup or mid-restore.
//!
//! Each staging entry can hold a full copy of the database plus every blob, so a crash would otherwise leak that
//! space forever. Only entries older than [`STALE_STAGING_AGE`] are removed, so a backup or restore that another
//! process started within that hour keeps its files.
//!
//! `.bottie-attachment-rollback-*` directories are deliberately not swept: a crash between moving the live
//! attachment root aside and installing the restored one leaves the user's only copy of those files there.

use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime},
};

use super::{
    ConversationStore,
    backup::{
        ATTACHMENT_RESTORE_STAGING_PREFIX, GENERATED_ASSET_RESTORE_STAGING_PREFIX,
        RESTORE_STAGING_FILE_PREFIX,
    },
};

/// Minimum age before a staging entry counts as abandoned.
// shortcut: age is the top-level entry's mtime, so a second instance's restore running past an hour could be swept;
// track the newest nested mtime or add a single-instance lock if concurrent instances become supported.
pub(super) const STALE_STAGING_AGE: Duration = Duration::from_secs(60 * 60);

/// Returns the modification time before which a staging entry is considered abandoned.
pub(super) fn stale_staging_cutoff() -> SystemTime {
    SystemTime::now()
        .checked_sub(STALE_STAGING_AGE)
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

/// Counts from one staging sweep; `failed` entries were abandoned but could not be deleted.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct StagingSweep {
    pub(crate) removed: usize,
    pub(crate) failed: usize,
}

/// Removes entries in `directory` named `<prefix>-…` for any of `prefixes` and last modified before `cutoff`.
///
/// Unreadable directories and entries are skipped, never fatal: a sweep that cannot run must not block the backup
/// or startup that triggered it.
pub(super) fn remove_stale_staging(
    directory: &Path,
    prefixes: &[&str],
    cutoff: SystemTime,
) -> StagingSweep {
    let Ok(entries) = fs::read_dir(directory) else {
        return StagingSweep::default();
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            prefixes
                .iter()
                .any(|prefix| name.starts_with(&format!("{prefix}-")))
        })
        .filter(|entry| {
            entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .is_ok_and(|modified| modified < cutoff)
        })
        .fold(StagingSweep::default(), |mut sweep, entry| {
            // `file_type` does not follow symlinks, so a link is removed itself rather than its target.
            let removed = match entry.file_type() {
                Ok(kind) if kind.is_dir() => fs::remove_dir_all(entry.path()),
                Ok(_) => fs::remove_file(entry.path()),
                Err(error) => Err(error),
            };
            if removed.is_ok() {
                sweep.removed += 1;
            } else {
                sweep.failed += 1;
            }
            sweep
        })
}

impl ConversationStore {
    /// Removes abandoned restore staging beside the live database; call at startup before workers start.
    pub(crate) fn sweep_interrupted_restore_staging(&self) -> StagingSweep {
        let Some(directory) = self.path.parent() else {
            return StagingSweep::default();
        };
        remove_stale_staging(
            directory,
            &[
                RESTORE_STAGING_FILE_PREFIX,
                ATTACHMENT_RESTORE_STAGING_PREFIX,
                GENERATED_ASSET_RESTORE_STAGING_PREFIX,
            ],
            stale_staging_cutoff(),
        )
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// Creates a staging file whose modification time is `age` in the past.
    pub(in crate::storage) fn aged_file(path: &Path, age: Duration) {
        fs::write(path, b"staging").unwrap();
        let file = fs::File::options().write(true).open(path).unwrap();
        file.set_modified(SystemTime::now() - age).unwrap();
    }

    #[test]
    fn removes_only_old_entries_with_a_matching_prefix() {
        let directory = std::env::temp_dir().join(format!("bottie-sweep-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let old = directory.join(".bottie-auto-staging-old.sqlite3");
        let fresh = directory.join(".bottie-auto-staging-fresh.sqlite3");
        let unrelated = directory.join("bottie-auto-1-keep.sqlite3");
        let old_dir = directory.join(".bottie-attachment-restore-old");
        aged_file(&old, Duration::from_secs(2 * 60 * 60));
        aged_file(&fresh, Duration::from_secs(60));
        aged_file(&unrelated, Duration::from_secs(2 * 60 * 60));
        fs::create_dir_all(old_dir.join("blobs")).unwrap();
        aged_file(&old_dir.join("blobs/a"), Duration::from_secs(2 * 60 * 60));
        fs::File::open(&old_dir)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(2 * 60 * 60))
            .unwrap();

        let removed = remove_stale_staging(
            &directory,
            &[".bottie-auto-staging", ".bottie-attachment-restore"],
            stale_staging_cutoff(),
        );

        assert_eq!(
            removed,
            StagingSweep {
                removed: 2,
                failed: 0
            }
        );
        assert!(!old.exists());
        assert!(!old_dir.exists());
        assert!(fresh.exists());
        assert!(unrelated.exists());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn startup_sweep_removes_restore_staging_but_keeps_rollback_and_safety_copies() {
        let path = crate::storage::tests::test_database_path();
        let directory = path.parent().unwrap().to_path_buf();
        let store = ConversationStore::initialize(path).unwrap();
        let hours_ago = Duration::from_secs(2 * 60 * 60);
        let swept = [
            ".bottie-restore-staging-x.sqlite3",
            ".bottie-attachment-restore-x",
            ".bottie-generated-asset-restore-x",
        ];
        let kept = [
            ".bottie-attachment-rollback-x",
            "bottie-pre-restore-x.sqlite3",
        ];
        for name in swept.iter().chain(&kept) {
            aged_file(&directory.join(name), hours_ago);
        }

        assert_eq!(
            store.sweep_interrupted_restore_staging(),
            StagingSweep {
                removed: 3,
                failed: 0
            }
        );
        assert!(swept.iter().all(|name| !directory.join(name).exists()));
        assert!(kept.iter().all(|name| directory.join(name).exists()));
        let _ = fs::remove_dir_all(directory);
    }
}
