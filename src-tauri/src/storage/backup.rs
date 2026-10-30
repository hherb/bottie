//! Consistent manual backup and restore for Bottie's live SQLite store; rotation lives in `automatic_backup`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, MAIN_DB, OpenFlags};

use super::portable_backup::{
    embed_portable_payload, extract_portable_payload, missing_content_count, omitted_file_count,
    strip_portable_payload, strip_portable_payload_from_connection, validate_portable_payload,
};
use super::staging_sweep::{StagingSweep, remove_stale_staging_files, stale_staging_cutoff};
use super::{CURRENT_SCHEMA_VERSION, ConversationStore, StorageError, now_ms};

const PRE_RESTORE_FILE_PREFIX: &str = "bottie-pre-restore";
pub(super) const RESTORE_STAGING_FILE_PREFIX: &str = ".bottie-restore-staging";
pub(super) const ATTACHMENT_RESTORE_STAGING_PREFIX: &str = ".bottie-attachment-restore";
pub(super) const GENERATED_ASSET_RESTORE_STAGING_PREFIX: &str = ".bottie-generated-asset-restore";
pub(super) const MANUAL_BACKUP_STAGING_PREFIX: &str = ".bottie-backup";
const REQUIRED_BACKUP_TABLES: &[&str] = &[
    "schema_migrations",
    "profiles",
    "conversations",
    "branches",
    "messages",
    "message_blocks",
];

/// Removes hour-old staging a killed manual backup left beside `destination`; call before
/// [`ConversationStore::backup_to`].
///
/// The user's folder has no startup sweep, so each backup clears it. Kept separate from the backup so its failures
/// are reported even when the backup itself then fails (#225); anything left is retried by the next backup there.
pub(crate) fn sweep_manual_backup_staging(destination: &Path) -> StagingSweep {
    remove_stale_staging_files(
        backup_directory(destination),
        MANUAL_BACKUP_STAGING_PREFIX,
        &[".part", ".part-journal", ".part-wal", ".part-shm"],
        stale_staging_cutoff(),
    )
}

/// The folder a backup to `destination` stages in; a bare filename stages in the working directory.
fn backup_directory(destination: &Path) -> &Path {
    destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

impl ConversationStore {
    /// Copies every committed page to an independently readable database through SQLite's online backup API.
    ///
    /// Returns how many attachment or image files were left out because they were missing.
    pub(crate) fn backup_to(&self, destination: &Path) -> Result<usize, StorageError> {
        if paths_refer_to_same_file(&self.path, destination) {
            return Err(StorageError::invalid(
                "Choose a different location for the Bottie backup.",
            ));
        }
        // Build and verify beside the destination, then rename over it, so a failed backup
        // never destroys or replaces the user's previous backup.
        let staging = backup_directory(destination).join(format!(
            "{MANUAL_BACKUP_STAGING_PREFIX}-{}.part",
            uuid::Uuid::new_v4()
        ));
        let result = self.write_verified_backup(&staging).and_then(|skipped| {
            remove_database_sidecars(destination);
            fs::rename(&staging, destination).map_err(|_| StorageError::backup())?;
            Ok(skipped)
        });
        remove_database_files(&staging);
        result
    }

    /// Writes a complete snapshot with its portable payload to `path` and verifies it.
    ///
    /// On failure `path` may hold partial bytes; callers write to a staging path and clean up.
    pub(super) fn write_verified_backup(&self, path: &Path) -> Result<usize, StorageError> {
        remove_database_sidecars(path);
        let source = self.open()?;
        source
            .backup(MAIN_DB, path, None)
            .map_err(|_| StorageError::backup())?;
        let skipped =
            embed_portable_payload(path, &self.attachment_root(), &self.generated_asset_root())
                .map_err(|_| StorageError::backup())?;
        verify_backup(path)?;
        // Every row the snapshot lacks must be a file this backup actually found missing.
        if omitted_file_count(path).map_err(|_| StorageError::backup())? != skipped {
            return Err(StorageError::backup());
        }
        Ok(skipped)
    }

    /// Counts catalogued attachment and image files that are absent from the live store's disk.
    pub(crate) fn missing_content_files(&self) -> Result<usize, StorageError> {
        missing_content_count(
            &self.open()?,
            &self.attachment_root(),
            &self.generated_asset_root(),
        )
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
        if self
            .recovery_required
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return self.recover_corrupt_store(source, safety_copy);
        }
        if paths_refer_to_same_file(&self.path, source) {
            return Err(StorageError::invalid_backup());
        }
        validate_restore_source(source)?;
        let staging = restore_staging_path(&self.path)?;
        let attachment_staging = attachment_restore_staging_path(&self.path)?;
        let generated_asset_staging = generated_asset_restore_staging_path(&self.path)?;
        let restore_result = self.restore_through_staging(
            source,
            safety_copy,
            &staging,
            &attachment_staging,
            &generated_asset_staging,
        );
        remove_database_files(&staging);
        let _ = fs::remove_dir_all(&attachment_staging);
        let _ = fs::remove_dir_all(&generated_asset_staging);
        restore_result
    }

    /// Migrates an isolated copy before changing the live store, then restores it through SQLite's backup API.
    fn restore_through_staging(
        &self,
        source: &Path,
        safety_copy: &Path,
        staging: &Path,
        attachment_staging: &Path,
        generated_asset_staging: &Path,
    ) -> Result<(), StorageError> {
        copy_database(source, staging).map_err(|_| StorageError::invalid_backup())?;
        ConversationStore::initialize(staging.to_path_buf())
            .map_err(|_| StorageError::invalid_backup())?;
        validate_restore_source(staging)?;
        let has_portable_payload =
            extract_portable_payload(staging, attachment_staging, generated_asset_staging)?;
        strip_portable_payload(staging)?;
        self.backup_to(safety_copy)
            .map_err(|_| StorageError::restore_safety_copy())?;
        let attachment_rollback = if has_portable_payload {
            Some(install_attachment_staging(
                &attachment_staging,
                &self.attachment_root(),
            )?)
        } else {
            None
        };
        let generated_asset_rollback = if generated_asset_staging.exists() {
            match install_attachment_staging(generated_asset_staging, &self.generated_asset_root())
            {
                Ok(rollback) => Some(rollback),
                Err(error) => {
                    if let Some(rollback) = attachment_rollback.as_deref() {
                        rollback_attachment_install(&self.attachment_root(), rollback);
                    }
                    return Err(error);
                }
            }
        } else {
            None
        };
        let mut live = match self.open() {
            Ok(live) => live,
            Err(_) => {
                if let Some(rollback) = generated_asset_rollback.as_deref() {
                    rollback_attachment_install(&self.generated_asset_root(), rollback);
                }
                if let Some(rollback) = attachment_rollback.as_deref() {
                    rollback_attachment_install(&self.attachment_root(), rollback);
                }
                return Err(StorageError::restore());
            }
        };
        if live.restore(MAIN_DB, staging, None::<fn(_)>).is_err()
            || validate_live_restore(&live).is_err()
        {
            let _ = live.restore(MAIN_DB, safety_copy, None::<fn(_)>);
            let _ = strip_portable_payload_from_connection(&live);
            if let Some(rollback) = generated_asset_rollback.as_deref() {
                rollback_attachment_install(&self.generated_asset_root(), rollback);
            }
            if let Some(rollback) = attachment_rollback.as_deref() {
                rollback_attachment_install(&self.attachment_root(), rollback);
            }
            return Err(StorageError::restore());
        }
        if let Some(rollback) = attachment_rollback {
            let _ = fs::remove_dir_all(rollback);
        }
        if let Some(rollback) = generated_asset_rollback {
            let _ = fs::remove_dir_all(rollback);
        }
        Ok(())
    }
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
    if integrity != "ok" {
        Err(StorageError::backup())
    } else {
        validate_portable_payload(&connection)
            .map(|_| ())
            .map_err(|_| StorageError::backup())
    }
}

/// Rejects corrupt, unrelated, empty, and newer-schema databases before live data can change.
pub(super) fn validate_restore_source(path: &Path) -> Result<(), StorageError> {
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
        validate_portable_payload(&connection).map(|_| ())
    } else {
        Err(StorageError::invalid_backup())
    }
}

/// Chooses a unique same-parent directory for rehydrating a portable attachment payload.
fn attachment_restore_staging_path(live: &Path) -> Result<PathBuf, StorageError> {
    let parent = live.parent().ok_or_else(StorageError::restore)?;
    Ok(parent.join(format!(
        "{ATTACHMENT_RESTORE_STAGING_PREFIX}-{}",
        uuid::Uuid::new_v4()
    )))
}

/// Chooses a unique same-parent directory for rehydrating generated PNG payloads.
fn generated_asset_restore_staging_path(live: &Path) -> Result<PathBuf, StorageError> {
    let parent = live.parent().ok_or_else(StorageError::restore)?;
    Ok(parent.join(format!(
        "{GENERATED_ASSET_RESTORE_STAGING_PREFIX}-{}",
        uuid::Uuid::new_v4()
    )))
}

/// Installs staged attachment bytes while retaining the previous root for rollback.
fn install_attachment_staging(staging: &Path, live: &Path) -> Result<PathBuf, StorageError> {
    let parent = live.parent().ok_or_else(StorageError::restore)?;
    let rollback = parent.join(format!(
        ".bottie-attachment-rollback-{}",
        uuid::Uuid::new_v4()
    ));
    if live.exists() && fs::rename(live, &rollback).is_err() {
        let _ = fs::remove_dir_all(staging);
        return Err(StorageError::restore());
    }
    if fs::rename(staging, live).is_err() {
        if rollback.exists() {
            let _ = fs::rename(&rollback, live);
        }
        let _ = fs::remove_dir_all(staging);
        return Err(StorageError::restore());
    }
    Ok(rollback)
}

/// Restores the previous attachment root after a database-installation failure.
fn rollback_attachment_install(live: &Path, rollback: &Path) {
    let _ = fs::remove_dir_all(live);
    if rollback.exists() {
        let _ = fs::rename(rollback, live);
    }
}

/// Copies a candidate into an isolated database while including any committed WAL content.
pub(super) fn copy_database(source: &Path, destination: &Path) -> Result<(), rusqlite::Error> {
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
pub(super) fn restore_staging_path(live: &Path) -> Result<PathBuf, StorageError> {
    let parent = live.parent().ok_or_else(StorageError::restore)?;
    Ok(parent.join(format!(
        "{RESTORE_STAGING_FILE_PREFIX}-{}.sqlite3",
        uuid::Uuid::new_v4()
    )))
}

/// Removes the exact temporary database and any SQLite sidecars left by validation.
pub(super) fn remove_database_files(path: &Path) {
    let _ = std::fs::remove_file(path);
    remove_database_sidecars(path);
}

/// Removes only SQLite WAL/shared-memory sidecars for one exact database path.
fn remove_database_sidecars(path: &Path) {
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        let sidecar = PathBuf::from(sidecar);
        let _ = std::fs::remove_file(sidecar);
    }
}
