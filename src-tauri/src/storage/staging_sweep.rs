//! Removes staging files and directories left behind when the app was killed mid-backup or mid-restore.
//!
//! Each staging entry can hold a full copy of the database plus every blob, so a crash would otherwise leak that
//! space forever. Only entries older than [`STALE_STAGING_AGE`] are removed, so a backup or restore still running in
//! another process keeps its files.
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
pub(super) const STALE_STAGING_AGE: Duration = Duration::from_secs(60 * 60);

/// Returns the modification time before which a staging entry is considered abandoned.
pub(super) fn stale_staging_cutoff() -> SystemTime {
    SystemTime::now()
        .checked_sub(STALE_STAGING_AGE)
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

/// Removes entries in `directory` named `<prefix>-…` for any of `prefixes` and last modified before `cutoff`.
///
/// Returns how many entries were removed. Unreadable directories and entries are skipped, never fatal: a sweep
/// that cannot run must not block the backup or startup that triggered it.
pub(super) fn remove_stale_staging(
    directory: &Path,
    prefixes: &[&str],
    cutoff: SystemTime,
) -> usize {
    let Ok(entries) = fs::read_dir(directory) else {
        return 0;
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
        .filter(|entry| {
            // `file_type` does not follow symlinks, so a link is removed itself rather than its target.
            let removed = match entry.file_type() {
                Ok(kind) if kind.is_dir() => fs::remove_dir_all(entry.path()),
                Ok(_) => fs::remove_file(entry.path()),
                Err(error) => Err(error),
            };
            removed.is_ok()
        })
        .count()
}

impl ConversationStore {
    /// Removes abandoned restore staging beside the live database; call at startup before workers start.
    pub(crate) fn sweep_interrupted_restore_staging(&self) -> usize {
        let Some(directory) = self.path.parent() else {
            return 0;
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
mod tests {
    use super::*;

    /// Creates a staging file whose modification time is `age` in the past.
    fn aged_file(path: &Path, age: Duration) {
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

        assert_eq!(removed, 2);
        assert!(!old.exists());
        assert!(!old_dir.exists());
        assert!(fresh.exists());
        assert!(unrelated.exists());
        let _ = fs::remove_dir_all(directory);
    }
}
