//! Removes staging files and directories left behind when the app was killed mid-backup, mid-export or mid-restore.
//!
//! Each staging entry can hold a full copy of the database plus every blob, so a crash would otherwise leak that
//! space forever. Only entries older than [`STALE_STAGING_AGE`] are removed, so a backup, export or restore that
//! another process started within that hour keeps its files.
//!
//! Folders the user chose (manual backups, exports) get the narrower [`remove_stale_staging_files`], which only
//! touches plain files named exactly as Bottie names its staging, so nothing the user put there is deleted.
//!
//! `.bottie-attachment-rollback-*` directories are deliberately not swept: a crash between moving the live
//! attachment root aside and installing the restored one leaves the user's only copy of those files there.

use std::{
    fs, io,
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

/// Counts from one staging sweep; `failed` entries were abandoned (or could not be inspected) but were not deleted.
#[derive(Debug, Default, PartialEq, Eq)]
#[must_use]
pub(crate) struct StagingSweep {
    pub(crate) removed: usize,
    pub(crate) failed: usize,
}

/// Removes entries in an app-owned `directory` named `<prefix>-…` for any of `prefixes` and last modified before
/// `cutoff`; directories are removed with their contents.
///
/// Unreadable directories and entries are counted as failed, never fatal: a sweep that cannot run must not block
/// the backup or startup that triggered it.
pub(super) fn remove_stale_staging(
    directory: &Path,
    prefixes: &[&str],
    cutoff: SystemTime,
) -> StagingSweep {
    sweep_matching(directory, cutoff, false, |name| {
        prefixes
            .iter()
            .any(|prefix| name.starts_with(&format!("{prefix}-")))
    })
}

/// Removes plain files in a user-chosen `directory` named `<prefix>-<uuid><suffix>` for one of `suffixes` and last
/// modified before `cutoff`.
///
/// Directories, links and any other name are left alone: the folder belongs to the user.
pub(super) fn remove_stale_staging_files(
    directory: &Path,
    prefix: &str,
    suffixes: &[&str],
    cutoff: SystemTime,
) -> StagingSweep {
    sweep_matching(directory, cutoff, true, |name| {
        is_staging_file_name(name, prefix, suffixes)
    })
}

/// Whether `name` is `<prefix>-<uuid><suffix>`, the shape Bottie gives every staging file it creates.
fn is_staging_file_name(name: &str, prefix: &str, suffixes: &[&str]) -> bool {
    const UUID_LENGTH: usize = 36;
    let Some(rest) = name
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('-'))
    else {
        return false;
    };
    let (Some(id), Some(suffix)) = (rest.get(..UUID_LENGTH), rest.get(UUID_LENGTH..)) else {
        return false;
    };
    uuid::Uuid::try_parse(id).is_ok() && suffixes.contains(&suffix)
}

/// Removes stale entries whose names satisfy `matches`; `files_only` skips everything but regular files.
fn sweep_matching(
    directory: &Path,
    cutoff: SystemTime,
    files_only: bool,
    matches: impl Fn(&str) -> bool,
) -> StagingSweep {
    let mut sweep = StagingSweep::default();
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return sweep,
        Err(_) => {
            sweep.failed += 1;
            return sweep;
        }
    };
    for entry in entries {
        let Ok(entry) = entry else {
            sweep.failed += 1;
            continue;
        };
        if !matches(&entry.file_name().to_string_lossy()) {
            continue;
        }
        // `file_type` does not follow symlinks, so a link is never followed into its target.
        let Ok(kind) = entry.file_type() else {
            sweep.failed += 1;
            continue;
        };
        if files_only && !kind.is_file() {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|metadata| metadata.modified()) else {
            sweep.failed += 1;
            continue;
        };
        if modified >= cutoff {
            continue;
        }
        let removed = if kind.is_dir() {
            fs::remove_dir_all(entry.path())
        } else {
            fs::remove_file(entry.path())
        };
        if removed.is_ok() {
            sweep.removed += 1;
        } else {
            sweep.failed += 1;
        }
    }
    sweep
}

/// Diagnostic text for staging a sweep could not delete, or `None` when nothing was left behind.
///
/// `staging` names whose staging it was (for example "automatic-backup") and `retry` says when it is tried again.
pub(crate) fn staging_cleanup_warning(failed: usize, staging: &str, retry: &str) -> Option<String> {
    (failed > 0).then(|| {
        format!("{failed} interrupted {staging} staging item(s) could not be removed; {retry}")
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

    #[test]
    fn cleanup_warning_names_the_count_and_retry_only_when_something_was_left() {
        assert_eq!(staging_cleanup_warning(0, "export", "retried later"), None);
        assert_eq!(
            staging_cleanup_warning(2, "export", "the next export into that folder retries them")
                .as_deref(),
            Some(
                "2 interrupted export staging item(s) could not be removed; \
                 the next export into that folder retries them"
            )
        );
    }

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

    #[test]
    fn a_user_folder_sweep_removes_only_bottie_named_staging_files() {
        let directory = std::env::temp_dir().join(format!("bottie-sweep-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let hours_ago = Duration::from_secs(2 * 60 * 60);
        let id = uuid::Uuid::new_v4();
        let swept = [
            format!(".bottie-backup-{id}.part"),
            format!(".bottie-backup-{id}.part-wal"),
        ];
        let kept = [
            ".bottie-backup-weekly.sqlite3".to_owned(),
            format!(".bottie-backup-{id}.sqlite3"),
            format!(".bottie-backup-{id}.part.bak"),
        ];
        for name in swept.iter().chain(&kept) {
            aged_file(&directory.join(name), hours_ago);
        }
        // Even a directory with a staging-shaped name is the user's: Bottie only ever stages plain files here.
        let user_folder = directory.join(format!(".bottie-backup-{}.part", uuid::Uuid::new_v4()));
        fs::create_dir_all(&user_folder).unwrap();
        aged_file(&user_folder.join("keep"), hours_ago);

        let sweep = remove_stale_staging_files(
            &directory,
            ".bottie-backup",
            &[".part", ".part-wal"],
            stale_staging_cutoff(),
        );

        assert_eq!(
            sweep,
            StagingSweep {
                removed: 2,
                failed: 0
            }
        );
        assert!(swept.iter().all(|name| !directory.join(name).exists()));
        assert!(kept.iter().all(|name| directory.join(name).exists()));
        assert!(user_folder.join("keep").exists());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn staging_names_need_the_exact_prefix_uuid_and_suffix() {
        let id = "67e55044-10b1-426f-9247-bb680e5fe0c8";
        let suffixes = [".tmp"];
        assert!(is_staging_file_name(
            &format!(".bottie-export-{id}.tmp"),
            ".bottie-export",
            &suffixes
        ));
        for name in [
            ".bottie-export-archive".to_owned(),
            format!(".bottie-export-{id}"),
            format!(".bottie-export-{id}.zip"),
            format!(".bottie-exports-{id}.tmp"),
            format!(".bottie-export-{}.tmp", &id[..35]),
            ".bottie-export-ü".to_owned(),
        ] {
            assert!(
                !is_staging_file_name(&name, ".bottie-export", &suffixes),
                "{name}"
            );
        }
    }

    #[test]
    fn an_unreadable_directory_counts_as_a_failed_sweep() {
        let file = std::env::temp_dir().join(format!("bottie-sweep-{}", uuid::Uuid::new_v4()));
        fs::write(&file, b"not a directory").unwrap();
        let missing = file.with_extension("missing");

        assert_eq!(
            remove_stale_staging(&file, &[".bottie-auto-staging"], stale_staging_cutoff()).failed,
            1
        );
        assert_eq!(
            remove_stale_staging(&missing, &[".bottie-auto-staging"], stale_staging_cutoff()),
            StagingSweep::default()
        );
        let _ = fs::remove_file(file);
    }
}
