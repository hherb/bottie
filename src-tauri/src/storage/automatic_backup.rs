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
    /// Catalog files left out of the new snapshot because they were missing from disk.
    pub(crate) skipped_files: usize,
    /// Directory entries that could not be read, so they were neither kept nor pruned (#228).
    pub(crate) unreadable_entries: usize,
}

/// One application-owned automatic snapshot discovered from its strict filename contract.
pub(super) struct ManagedBackup {
    pub(super) timestamp_ms: i64,
    pub(super) path: PathBuf,
}

/// The managed snapshots in a directory, plus how many entries could not be read to decide.
pub(super) struct ManagedBackupListing {
    pub(super) backups: Vec<ManagedBackup>,
    /// Entries whose directory record or file type failed to read; a snapshot among them is invisible to
    /// retention and recovery, so callers report this count instead of dropping it silently.
    pub(super) unreadable: usize,
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
        let directory = automatic_backup_directory(&self.path)?;
        fs::create_dir_all(&directory).map_err(|_| StorageError::automatic_backup())?;
        let ManagedBackupListing {
            mut backups,
            unreadable: unreadable_entries,
        } = managed_backups(&directory)?;
        let freshness_threshold = timestamp_ms.saturating_sub(AUTOMATIC_BACKUP_INTERVAL_MS);
        if backups
            .iter()
            .any(|backup| backup.timestamp_ms > freshness_threshold)
        {
            return Ok(AutomaticBackupRotation {
                created: false,
                retained: backups.len(),
                pruned: 0,
                skipped_files: 0,
                unreadable_entries,
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
        for backup in &expired {
            fs::remove_file(&backup.path).map_err(|_| StorageError::automatic_backup())?;
        }
        Ok(AutomaticBackupRotation {
            created: true,
            retained: backups.len(),
            pruned: expired.len(),
            skipped_files,
            unreadable_entries,
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
    };
    for entry in fs::read_dir(directory).map_err(|_| StorageError::automatic_backup())? {
        let Ok(entry) = entry else {
            listing.unreadable += 1;
            continue;
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

/// Diagnostic text for backup-folder entries rotation could not read, or `None` when every entry was read.
pub(crate) fn unreadable_backup_warning(unreadable: usize) -> Option<String> {
    (unreadable > 0).then(|| {
        format!(
            "{unreadable} item(s) in the automatic-backup folder could not be read, so retention and \
             recovery skip them; the next launch checks again"
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
    fn unreadable_warning_names_the_count_only_when_entries_were_skipped() {
        assert_eq!(unreadable_backup_warning(0), None);
        assert_eq!(
            unreadable_backup_warning(1).as_deref(),
            Some(
                "1 item(s) in the automatic-backup folder could not be read, so retention and \
                 recovery skip them; the next launch checks again"
            )
        );
    }
}
