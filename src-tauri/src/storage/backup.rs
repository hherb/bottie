//! Consistent manual backup, automatic rotation, and restore for Bottie's live SQLite store.

use std::{
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, MAIN_DB, OpenFlags};

use super::{CURRENT_SCHEMA_VERSION, ConversationStore, StorageError, now_ms};

const PRE_RESTORE_FILE_PREFIX: &str = "bottie-pre-restore";
const RESTORE_STAGING_FILE_PREFIX: &str = ".bottie-restore-staging";
const AUTOMATIC_BACKUP_DIRECTORY: &str = "automatic-backups";
const AUTOMATIC_BACKUP_FILE_PREFIX: &str = "bottie-auto";
const AUTOMATIC_BACKUP_STAGING_PREFIX: &str = ".bottie-auto-staging";
const AUTOMATIC_BACKUP_INTERVAL_MS: i64 = 24 * 60 * 60 * 1_000;
const AUTOMATIC_BACKUP_RETENTION_COUNT: usize = 7;
const REQUIRED_BACKUP_TABLES: &[&str] = &[
    "schema_migrations",
    "profiles",
    "conversations",
    "branches",
    "messages",
    "message_blocks",
];

/// Summary of one automatic-backup rotation without exposing native paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AutomaticBackupRotation {
    /// Whether this rotation created a new verified snapshot.
    pub(crate) created: bool,
    /// Number of managed automatic snapshots remaining after rotation.
    pub(crate) retained: usize,
    /// Number of expired managed snapshots removed by this rotation.
    pub(crate) pruned: usize,
}

/// One application-owned automatic snapshot discovered from its strict filename contract.
struct ManagedBackup {
    timestamp_ms: i64,
    path: PathBuf,
}

impl ConversationStore {
    /// Copies every committed page to an independently readable database through SQLite's online backup API.
    pub(crate) fn backup_to(&self, destination: &Path) -> Result<(), StorageError> {
        if paths_refer_to_same_file(&self.path, destination) {
            return Err(StorageError::invalid(
                "Choose a different location for the Bottie backup.",
            ));
        }
        let source = self.open()?;
        source
            .backup(MAIN_DB, destination, None)
            .map_err(|_| StorageError::backup())?;
        verify_backup(destination)
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
        let mut backups = managed_backups(&directory)?;
        let freshness_threshold = timestamp_ms.saturating_sub(AUTOMATIC_BACKUP_INTERVAL_MS);
        if backups
            .iter()
            .any(|backup| backup.timestamp_ms > freshness_threshold)
        {
            return Ok(AutomaticBackupRotation {
                created: false,
                retained: backups.len(),
                pruned: 0,
            });
        }

        let id = uuid::Uuid::new_v4();
        let staging = directory.join(format!("{AUTOMATIC_BACKUP_STAGING_PREFIX}-{id}.sqlite3"));
        let destination = directory.join(format!(
            "{AUTOMATIC_BACKUP_FILE_PREFIX}-{timestamp_ms}-{id}.sqlite3"
        ));
        if self.backup_to(&staging).is_err() {
            remove_database_files(&staging);
            return Err(StorageError::automatic_backup());
        }
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
        let expired = backups.split_off(backups.len().min(AUTOMATIC_BACKUP_RETENTION_COUNT));
        for backup in &expired {
            fs::remove_file(&backup.path).map_err(|_| StorageError::automatic_backup())?;
        }
        Ok(AutomaticBackupRotation {
            created: true,
            retained: backups.len(),
            pruned: expired.len(),
        })
    }

    /// Chooses an application-private filename for the safety snapshot created before restore.
    pub(crate) fn pre_restore_backup_path(&self) -> Result<PathBuf, StorageError> {
        let parent = self
            .path
            .parent()
            .ok_or_else(StorageError::restore_safety_copy)?;
        Ok(parent.join(format!(
            "{PRE_RESTORE_FILE_PREFIX}-{}-{}.sqlite3",
            now_ms().map_err(|_| StorageError::restore_safety_copy())?,
            uuid::Uuid::new_v4()
        )))
    }

    /// Validates and restores one Bottie backup after preserving the current live store.
    pub(crate) fn restore_from(
        &self,
        source: &Path,
        safety_copy: &Path,
    ) -> Result<(), StorageError> {
        if paths_refer_to_same_file(&self.path, source) {
            return Err(StorageError::invalid_backup());
        }
        validate_restore_source(source)?;
        let staging = restore_staging_path(&self.path)?;
        let restore_result = self.restore_through_staging(source, safety_copy, &staging);
        remove_database_files(&staging);
        restore_result
    }

    /// Migrates an isolated copy before changing the live store, then restores it through SQLite's backup API.
    fn restore_through_staging(
        &self,
        source: &Path,
        safety_copy: &Path,
        staging: &Path,
    ) -> Result<(), StorageError> {
        copy_database(source, staging).map_err(|_| StorageError::invalid_backup())?;
        ConversationStore::initialize(staging.to_path_buf())
            .map_err(|_| StorageError::invalid_backup())?;
        validate_restore_source(staging)?;
        self.backup_to(safety_copy)
            .map_err(|_| StorageError::restore_safety_copy())?;
        let mut live = self.open().map_err(|_| StorageError::restore())?;
        if live.restore(MAIN_DB, staging, None::<fn(_)>).is_err()
            || validate_live_restore(&live).is_err()
        {
            let _ = live.restore(MAIN_DB, safety_copy, None::<fn(_)>);
            return Err(StorageError::restore());
        }
        Ok(())
    }
}

/// Resolves the app-private rotation directory beside the live database.
fn automatic_backup_directory(live: &Path) -> Result<PathBuf, StorageError> {
    live.parent()
        .map(|parent| parent.join(AUTOMATIC_BACKUP_DIRECTORY))
        .ok_or_else(StorageError::automatic_backup)
}

/// Finds only regular files whose names exactly match Bottie's automatic-backup contract.
fn managed_backups(directory: &Path) -> Result<Vec<ManagedBackup>, StorageError> {
    let backups = fs::read_dir(directory)
        .map_err(|_| StorageError::automatic_backup())?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_file()))
        .filter_map(|entry| {
            let timestamp_ms = automatic_backup_timestamp(&entry.file_name())?;
            Some(ManagedBackup {
                timestamp_ms,
                path: entry.path(),
            })
        })
        .collect();
    Ok(backups)
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

/// Returns whether two existing paths resolve to the same filesystem entry.
fn paths_refer_to_same_file(source: &Path, destination: &Path) -> bool {
    if source == destination {
        return true;
    }
    match (
        std::fs::canonicalize(source),
        std::fs::canonicalize(destination),
    ) {
        (Ok(source), Ok(destination)) => source == destination,
        _ => false,
    }
}

/// Reopens the completed snapshot and rejects output that fails SQLite's quick integrity check.
fn verify_backup(path: &Path) -> Result<(), StorageError> {
    let connection = Connection::open(path).map_err(|_| StorageError::backup())?;
    let integrity: String = connection
        .pragma_query_value(None, "quick_check", |row| row.get(0))
        .map_err(|_| StorageError::backup())?;
    if integrity == "ok" {
        Ok(())
    } else {
        Err(StorageError::backup())
    }
}

/// Rejects corrupt, unrelated, empty, and newer-schema databases before live data can change.
fn validate_restore_source(path: &Path) -> Result<(), StorageError> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| StorageError::invalid_backup())?;
    let integrity: String = connection
        .pragma_query_value(None, "quick_check", |row| row.get(0))
        .map_err(|_| StorageError::invalid_backup())?;
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| StorageError::invalid_backup())?;
    if integrity != "ok" || !(1..=CURRENT_SCHEMA_VERSION).contains(&version) {
        return Err(StorageError::invalid_backup());
    }
    for table in REQUIRED_BACKUP_TABLES {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
                [table],
                |row| row.get(0),
            )
            .map_err(|_| StorageError::invalid_backup())?;
        if !exists {
            return Err(StorageError::invalid_backup());
        }
    }
    let has_local_profile: bool = connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM profiles WHERE id = 'local')",
            [],
            |row| row.get(0),
        )
        .map_err(|_| StorageError::invalid_backup())?;
    if has_local_profile {
        Ok(())
    } else {
        Err(StorageError::invalid_backup())
    }
}

/// Copies a candidate into an isolated database while including any committed WAL content.
fn copy_database(source: &Path, destination: &Path) -> Result<(), rusqlite::Error> {
    let connection = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    connection.backup(MAIN_DB, destination, None)
}

/// Confirms the restored destination still satisfies current Bottie integrity and schema policy.
fn validate_live_restore(connection: &Connection) -> Result<(), StorageError> {
    let integrity: String = connection
        .pragma_query_value(None, "quick_check", |row| row.get(0))
        .map_err(|_| StorageError::restore())?;
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| StorageError::restore())?;
    if integrity == "ok" && version == CURRENT_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(StorageError::restore())
    }
}

/// Chooses a unique same-directory staging file so restore never mutates the selected backup.
fn restore_staging_path(live: &Path) -> Result<PathBuf, StorageError> {
    let parent = live.parent().ok_or_else(StorageError::restore)?;
    Ok(parent.join(format!(
        "{RESTORE_STAGING_FILE_PREFIX}-{}.sqlite3",
        uuid::Uuid::new_v4()
    )))
}

/// Removes the exact temporary database and any SQLite sidecars left by validation.
fn remove_database_files(path: &Path) {
    let _ = std::fs::remove_file(path);
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        let sidecar = PathBuf::from(sidecar);
        let _ = std::fs::remove_file(sidecar);
    }
}
