//! Automatic backup rotation: at most one verified snapshot per day, seven kept, in an app-private folder.

use std::{
    fs,
    path::{Path, PathBuf},
};

use super::backup::remove_database_files;
use super::portable_backup::omitted_file_count;
use super::staging_sweep::{StagingSweep, remove_stale_staging, stale_staging_cutoff};
use super::{ConversationStore, StorageError, now_ms};

const AUTOMATIC_BACKUP_DIRECTORY: &str = "automatic-backups";
const AUTOMATIC_BACKUP_FILE_PREFIX: &str = "bottie-auto";
const AUTOMATIC_BACKUP_STAGING_PREFIX: &str = ".bottie-auto-staging";
const AUTOMATIC_BACKUP_INTERVAL_MS: i64 = 24 * 60 * 60 * 1_000;
const AUTOMATIC_BACKUP_RETENTION_COUNT: usize = 7;

/// Summary of one automatic-backup rotation without exposing native paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AutomaticBackupRotation {
    /// Whether this rotation created a new verified snapshot.
    pub(crate) created: bool,
    /// Number of managed automatic snapshots remaining after rotation.
    pub(crate) retained: usize,
    /// Number of expired managed snapshots removed by this rotation.
    pub(crate) pruned: usize,
    /// Expired snapshots that could not be removed; the new snapshot still exists, and the next rotation
    /// that creates a snapshot retries them (#230).
    pub(crate) prune_failed: usize,
    /// Catalog files left out of the new snapshot because they were missing from disk.
    pub(crate) skipped_files: usize,
}

impl AutomaticBackupRotation {
    /// Diagnostic level and detail for a rotation that created a snapshot; skipped files or a failed
    /// prune raise it to `warn`.
    pub(crate) fn created_report(&self) -> (&'static str, String) {
        let level = if self.skipped_files > 0 || self.prune_failed > 0 {
            "warn"
        } else {
            "info"
        };
        let prune_failed = if self.prune_failed > 0 {
            format!(
                " {} expired snapshot(s) could not be removed and are retried by the next new snapshot;",
                self.prune_failed
            )
        } else {
            String::new()
        };
        let detail = format!(
            "{} verified snapshot(s) retained; {} expired snapshot(s) removed;{prune_failed} \
             {} missing attachment or image file(s) left out",
            self.retained, self.pruned, self.skipped_files
        );
        (level, detail)
    }
}

/// One application-owned automatic snapshot discovered from its strict filename contract.
pub(super) struct ManagedBackup {
    pub(super) timestamp_ms: i64,
    pub(super) path: PathBuf,
}

/// The managed snapshots in a directory, plus what the listing could not see.
pub(super) struct ManagedBackupListing {
    pub(super) backups: Vec<ManagedBackup>,
    /// Entries whose file type failed to read; a snapshot among them is left on disk but invisible to
    /// retention and recovery, which `automatic_backup_folder_warning` reports at startup (#228).
    pub(super) unreadable: usize,
    /// The directory listing stopped on an error, so entries after it were never seen.
    pub(super) incomplete: bool,
}

impl ConversationStore {
    /// Removes abandoned automatic-backup staging; kept apart from rotation so its count survives a failed rotation.
    pub(crate) fn sweep_automatic_backup_staging(&self) -> StagingSweep {
        let Ok(directory) = automatic_backup_directory(&self.path) else {
            return StagingSweep::default();
        };
        remove_stale_staging(
            &directory,
            &[AUTOMATIC_BACKUP_STAGING_PREFIX],
            stale_staging_cutoff(),
        )
    }

    /// Checks the automatic-backup folder on its own, so a partly unreadable folder is reported even when
    /// rotation fails or is skipped for recovery (#228). `None` when every entry was read.
    pub(crate) fn automatic_backup_folder_warning(&self) -> Option<String> {
        let directory = automatic_backup_directory(&self.path).ok()?;
        if !directory.exists() {
            return None;
        }
        match managed_backups(&directory) {
            Ok(listing) => backup_folder_warning(listing.unreadable, listing.incomplete),
            Err(_) => backup_folder_warning(0, true),
        }
    }

    /// Creates a verified snapshot when the newest automatic backup is at least 24 hours old.
    pub(crate) fn rotate_automatic_backups(&self) -> Result<AutomaticBackupRotation, StorageError> {
        let timestamp_ms = now_ms().map_err(|_| StorageError::automatic_backup())?;
        self.rotate_automatic_backups_at(timestamp_ms)
    }

    /// Applies the automatic-backup interval and retention contract for a supplied timestamp.
    pub(crate) fn rotate_automatic_backups_at(
        &self,
        timestamp_ms: i64,
    ) -> Result<AutomaticBackupRotation, StorageError> {
        self.rotate_automatic_backups_with(timestamp_ms, |path| fs::remove_file(path))
    }

    /// Rotation with an injectable snapshot remover, so tests can fail a prune after the snapshot exists.
    pub(super) fn rotate_automatic_backups_with(
        &self,
        timestamp_ms: i64,
        remove: impl FnMut(&Path) -> std::io::Result<()>,
    ) -> Result<AutomaticBackupRotation, StorageError> {
        let directory = automatic_backup_directory(&self.path)?;
        fs::create_dir_all(&directory).map_err(|_| StorageError::automatic_backup())?;
        let mut backups = managed_backups(&directory)?.backups;
        let freshness_threshold = timestamp_ms.saturating_sub(AUTOMATIC_BACKUP_INTERVAL_MS);
        if backups
            .iter()
            .any(|backup| backup.timestamp_ms > freshness_threshold)
        {
            return Ok(AutomaticBackupRotation {
                created: false,
                retained: backups.len(),
                pruned: 0,
                prune_failed: 0,
                skipped_files: 0,
            });
        }

        let id = uuid::Uuid::new_v4();
        let staging = directory.join(format!("{AUTOMATIC_BACKUP_STAGING_PREFIX}-{id}.sqlite3"));
        let destination = directory.join(format!(
            "{AUTOMATIC_BACKUP_FILE_PREFIX}-{timestamp_ms}-{id}.sqlite3"
        ));
        let Ok(skipped_files) = self.write_verified_backup(&staging) else {
            remove_database_files(&staging);
            return Err(StorageError::automatic_backup());
        };
        if fs::rename(&staging, &destination).is_err() {
            remove_database_files(&staging);
            return Err(StorageError::automatic_backup());
        }
        backups.push(ManagedBackup {
            timestamp_ms,
            path: destination,
        });
        backups.sort_by(|left, right| {
            right
                .timestamp_ms
                .cmp(&left.timestamp_ms)
                .then_with(|| right.path.cmp(&left.path))
        });
        let mut expired = backups.split_off(backups.len().min(AUTOMATIC_BACKUP_RETENTION_COUNT));
        // A skipped file's bytes survive only in older snapshots, so keep the newest complete one.
        let is_complete = |backup: &ManagedBackup| omitted_file_count(&backup.path) == Ok(0);
        if skipped_files > 0
            && !backups.iter().any(is_complete)
            && let Some(index) = expired.iter().position(is_complete)
        {
            backups.push(expired.remove(index));
        }
        // The new snapshot is already in place, so a failed removal is a retention problem, not a failed backup.
        let prune_failed = remove_expired(&expired, remove);
        Ok(AutomaticBackupRotation {
            created: true,
            retained: backups.len(),
            pruned: expired.len() - prune_failed,
            prune_failed,
            skipped_files,
        })
    }
}

/// Resolves the app-private rotation directory beside the live database.
pub(super) fn automatic_backup_directory(live: &Path) -> Result<PathBuf, StorageError> {
    live.parent()
        .map(|parent| parent.join(AUTOMATIC_BACKUP_DIRECTORY))
        .ok_or_else(StorageError::automatic_backup)
}

/// Finds only regular files whose names exactly match Bottie's automatic-backup contract.
pub(super) fn managed_backups(directory: &Path) -> Result<ManagedBackupListing, StorageError> {
    let mut listing = ManagedBackupListing {
        backups: Vec::new(),
        unreadable: 0,
        incomplete: false,
    };
    for entry in fs::read_dir(directory).map_err(|_| StorageError::automatic_backup())? {
        // std ends a unix directory stream after its first read error, so this is not one skipped item.
        let Ok(entry) = entry else {
            listing.incomplete = true;
            break;
        };
        let Ok(file_type) = entry.file_type() else {
            listing.unreadable += 1;
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        if let Some(timestamp_ms) = automatic_backup_timestamp(&entry.file_name()) {
            listing.backups.push(ManagedBackup {
                timestamp_ms,
                path: entry.path(),
            });
        }
    }
    Ok(listing)
}

/// Tries to remove every expired snapshot with `remove`, returning how many removals failed.
/// A snapshot already gone (deleted by the user, say) counts as removed, not as a failure.
fn remove_expired(
    expired: &[ManagedBackup],
    mut remove: impl FnMut(&Path) -> std::io::Result<()>,
) -> usize {
    expired
        .iter()
        .filter(|backup| {
            remove(&backup.path).is_err_and(|error| error.kind() != std::io::ErrorKind::NotFound)
        })
        .count()
}

/// Diagnostic text for a backup folder that could not be fully read, or `None` when every entry was read.
fn backup_folder_warning(unreadable: usize, incomplete: bool) -> Option<String> {
    if incomplete {
        return Some(
            "The automatic-backup folder could not be read in full, so retention and recovery may miss \
             snapshots; later launches check again"
                .to_owned(),
        );
    }
    (unreadable > 0).then(|| {
        format!(
            "{unreadable} item(s) in the automatic-backup folder could not be read, so retention and \
             recovery skip them; later launches check again"
        )
    })
}

/// Parses the timestamp from one strictly formatted managed automatic-backup filename.
fn automatic_backup_timestamp(name: &std::ffi::OsStr) -> Option<i64> {
    let name = name.to_str()?;
    let body = name
        .strip_prefix(&format!("{AUTOMATIC_BACKUP_FILE_PREFIX}-"))?
        .strip_suffix(".sqlite3")?;
    let (timestamp, id) = body.split_once('-')?;
    uuid::Uuid::parse_str(id).ok()?;
    timestamp.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn created_report_warns_and_names_failed_prunes_only_when_there_are_any() {
        let clean = AutomaticBackupRotation {
            created: true,
            retained: 7,
            pruned: 1,
            prune_failed: 0,
            skipped_files: 0,
        };
        assert_eq!(
            clean.created_report(),
            (
                "info",
                "7 verified snapshot(s) retained; 1 expired snapshot(s) removed; \
                 0 missing attachment or image file(s) left out"
                    .to_owned()
            )
        );
        let stuck = AutomaticBackupRotation {
            pruned: 0,
            prune_failed: 1,
            ..clean
        };
        let (level, detail) = stuck.created_report();
        assert_eq!(level, "warn");
        assert!(detail.contains(
            "1 expired snapshot(s) could not be removed and are retried by the next new snapshot;"
        ));
    }

    #[test]
    fn a_failed_prune_is_counted_and_the_rest_are_still_removed() {
        let expired = ["a", "b", "c", "gone"].map(|name| ManagedBackup {
            timestamp_ms: 0,
            path: PathBuf::from(name),
        });
        let mut attempted = Vec::new();

        let failed = remove_expired(&expired, |path| {
            attempted.push(path.to_owned());
            if path == Path::new("b") {
                Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
            } else if path == Path::new("gone") {
                Err(std::io::Error::from(std::io::ErrorKind::NotFound))
            } else {
                Ok(())
            }
        });

        assert_eq!(failed, 1);
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
}
