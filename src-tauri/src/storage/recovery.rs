//! Startup corruption detection and guided restoration from verified recovery points.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use rusqlite::{Connection, ErrorCode, OpenFlags};
use serde::Serialize;

use super::{
    ConversationStore, StorageError,
    backup::{
        automatic_backup_directory, copy_database, managed_backups, remove_database_files,
        restore_staging_path, validate_restore_source,
    },
    now_ms,
    portable_backup::{extract_portable_payload, strip_portable_payload},
};

const DAMAGED_STORE_DIRECTORY_PREFIX: &str = "bottie-damaged-data";
const RECOVERY_REPLACEMENT_PREFIX: &str = ".bottie-recovery-replacement";
const RECOVERY_ATTACHMENT_STAGING_PREFIX: &str = ".bottie-recovery-attachments";
const SQLITE_SIDECAR_SUFFIXES: &[&str] = &["-wal", "-shm"];

/// Startup result that keeps the native app available when SQLite reports corruption.
pub(crate) struct StorageStartup {
    /// Path-backed store shared by normal commands and recovery actions.
    pub(crate) store: ConversationStore,
    /// Whether conversation access must remain paused until a verified restore completes.
    pub(crate) recovery_required: bool,
}

/// Stable local-data availability returned to the WebView without filesystem paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StorageRecoveryState {
    /// The conversation store passed startup integrity and is available normally.
    Ready,
    /// SQLite reported corruption and only native recovery actions are available.
    RecoveryRequired,
}

/// Path-redacted recovery guidance derived from verified app-private snapshots.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StorageRecoveryStatus {
    /// Current conversation-store availability.
    pub(crate) state: StorageRecoveryState,
    /// Number of valid automatic snapshots Bottie can restore without a file picker.
    pub(crate) automatic_backup_count: usize,
    /// Creation timestamp of the newest valid automatic snapshot, when one exists.
    pub(crate) latest_automatic_backup_at_ms: Option<i64>,
}

impl ConversationStore {
    /// Initializes healthy data or returns a restricted store when SQLite identifies corruption.
    pub(crate) fn initialize_for_app(path: PathBuf) -> Result<StorageStartup, StorageError> {
        if existing_store_is_corrupt(&path)? {
            return Ok(StorageStartup {
                store: Self {
                    path,
                    recovery_required: Arc::new(AtomicBool::new(true)),
                },
                recovery_required: true,
            });
        }
        Ok(StorageStartup {
            store: Self::initialize(path)?,
            recovery_required: false,
        })
    }

    /// Returns recovery availability and only counts automatic snapshots that pass validation.
    pub(crate) fn recovery_status(&self) -> Result<StorageRecoveryStatus, StorageError> {
        if !self.recovery_required.load(Ordering::Acquire) {
            return Ok(StorageRecoveryStatus {
                state: StorageRecoveryState::Ready,
                automatic_backup_count: 0,
                latest_automatic_backup_at_ms: None,
            });
        }
        let backups = self.valid_automatic_backups().unwrap_or_default();
        Ok(StorageRecoveryStatus {
            state: StorageRecoveryState::RecoveryRequired,
            automatic_backup_count: backups.len(),
            latest_automatic_backup_at_ms: backups.first().map(|backup| backup.0),
        })
    }

    /// Returns whether normal conversation access is currently paused for recovery.
    pub(crate) fn is_recovery_required(&self) -> bool {
        self.recovery_required.load(Ordering::Acquire)
    }

    /// Chooses an application-private preservation target appropriate to the store state.
    pub(crate) fn restore_preservation_path(&self) -> Result<PathBuf, StorageError> {
        if !self.recovery_required.load(Ordering::Acquire) {
            return self.pre_restore_backup_path();
        }
        let parent = self
            .path
            .parent()
            .ok_or_else(StorageError::recovery_preservation)?;
        Ok(parent.join(format!(
            "{DAMAGED_STORE_DIRECTORY_PREFIX}-{}-{}",
            now_ms().map_err(|_| StorageError::recovery_preservation())?,
            uuid::Uuid::new_v4()
        )))
    }

    /// Restores the newest verified automatic snapshot and returns its native creation timestamp.
    pub(crate) fn restore_latest_automatic_backup(
        &self,
        preservation: &Path,
    ) -> Result<i64, StorageError> {
        if !self.recovery_required.load(Ordering::Acquire) {
            return Err(StorageError::invalid(
                "Automatic recovery is available only when local data needs recovery.",
            ));
        }
        let (timestamp_ms, source) = self
            .valid_automatic_backups()?
            .into_iter()
            .next()
            .ok_or_else(|| StorageError::invalid("No verified automatic backup is available."))?;
        self.recover_corrupt_store(&source, preservation)?;
        Ok(timestamp_ms)
    }

    /// Replaces a corrupt live database only after staging a migrated verified replacement.
    pub(super) fn recover_corrupt_store(
        &self,
        source: &Path,
        preservation: &Path,
    ) -> Result<(), StorageError> {
        validate_restore_source(source)?;
        let staging = restore_staging_path(&self.path)?;
        let replacement = recovery_replacement_path(&self.path)?;
        let attachment_staging = recovery_attachment_staging_path(&self.path)?;
        let result = self.recover_through_replacement(
            source,
            preservation,
            &staging,
            &replacement,
            &attachment_staging,
        );
        remove_database_files(&staging);
        remove_database_files(&replacement);
        let _ = fs::remove_dir_all(&attachment_staging);
        result
    }

    /// Builds the replacement independently, preserves the damaged bundle, and installs atomically by rename.
    fn recover_through_replacement(
        &self,
        source: &Path,
        preservation: &Path,
        staging: &Path,
        replacement: &Path,
        attachment_staging: &Path,
    ) -> Result<(), StorageError> {
        copy_database(source, staging).map_err(|_| StorageError::invalid_backup())?;
        ConversationStore::initialize(staging.to_path_buf())
            .map_err(|_| StorageError::invalid_backup())?;
        let has_portable_payload = extract_portable_payload(staging, attachment_staging)?;
        strip_portable_payload(staging)?;
        copy_database(staging, replacement).map_err(|_| StorageError::restore())?;
        validate_restore_source(replacement).map_err(|_| StorageError::restore())?;

        let mut moved = preserve_database_bundle(&self.path, preservation)?;
        if has_portable_payload {
            preserve_attachment_root(&self.path, preservation, &mut moved)?;
        }
        if fs::rename(replacement, &self.path).is_err() {
            rollback_preserved_bundle(&moved);
            return Err(StorageError::restore());
        }
        if has_portable_payload && fs::rename(attachment_staging, self.attachment_root()).is_err() {
            remove_database_files(&self.path);
            rollback_preserved_bundle(&moved);
            return Err(StorageError::restore());
        }
        if ConversationStore::initialize(self.path.clone()).is_err() {
            remove_database_files(&self.path);
            if has_portable_payload {
                let _ = fs::remove_dir_all(self.attachment_root());
            }
            rollback_preserved_bundle(&moved);
            return Err(StorageError::restore());
        }
        self.recovery_required.store(false, Ordering::Release);
        Ok(())
    }

    /// Finds valid managed snapshots newest-first while ignoring malformed or corrupt lookalikes.
    fn valid_automatic_backups(&self) -> Result<Vec<(i64, PathBuf)>, StorageError> {
        let directory = automatic_backup_directory(&self.path)?;
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let mut backups = managed_backups(&directory)?
            .into_iter()
            .filter(|backup| validate_restore_source(&backup.path).is_ok())
            .map(|backup| (backup.timestamp_ms, backup.path))
            .collect::<Vec<_>>();
        backups.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
        Ok(backups)
    }
}

/// Checks an existing database without mutating it and classifies only SQLite corruption failures.
fn existing_store_is_corrupt(path: &Path) -> Result<bool, StorageError> {
    if !path.exists() || fs::metadata(path)?.len() == 0 {
        return Ok(false);
    }
    let connection = match Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(connection) => connection,
        Err(error) if is_corruption_error(&error) => return Ok(true),
        Err(_) => return Err(StorageError::internal()),
    };
    match connection.pragma_query_value::<String, _>(None, "quick_check", |row| row.get(0)) {
        Ok(result) => Ok(result != "ok"),
        Err(error) if is_corruption_error(&error) => Ok(true),
        Err(_) => Err(StorageError::internal()),
    }
}

/// Returns whether SQLite explicitly classified an operation as corruption or a non-database file.
fn is_corruption_error(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase,
                ..
            },
            _
        )
    )
}

/// Chooses a unique same-directory replacement so installation can use one filesystem rename.
fn recovery_replacement_path(live: &Path) -> Result<PathBuf, StorageError> {
    let parent = live.parent().ok_or_else(StorageError::restore)?;
    Ok(parent.join(format!(
        "{RECOVERY_REPLACEMENT_PREFIX}-{}.sqlite3",
        uuid::Uuid::new_v4()
    )))
}

/// Moves the damaged main database and present SQLite sidecars into one private directory.
fn preserve_database_bundle(
    live: &Path,
    preservation: &Path,
) -> Result<Vec<(PathBuf, PathBuf)>, StorageError> {
    fs::create_dir(preservation).map_err(|_| StorageError::recovery_preservation())?;
    let mut moved = Vec::new();
    for source in database_bundle_paths(live) {
        if !source.exists() {
            continue;
        }
        let file_name = source
            .file_name()
            .ok_or_else(StorageError::recovery_preservation)?;
        let destination = preservation.join(file_name);
        if fs::rename(&source, &destination).is_err() {
            rollback_preserved_bundle(&moved);
            let _ = fs::remove_dir(preservation);
            return Err(StorageError::recovery_preservation());
        }
        moved.push((source, destination));
    }
    if moved.is_empty() {
        let _ = fs::remove_dir(preservation);
        return Err(StorageError::recovery_preservation());
    }
    Ok(moved)
}

/// Restores a partially or fully preserved damaged bundle to its original filenames.
fn rollback_preserved_bundle(moved: &[(PathBuf, PathBuf)]) {
    for (source, destination) in moved.iter().rev() {
        let _ = fs::rename(destination, source);
    }
}

/// Moves the live attachment tree into the same damaged-data preservation directory.
fn preserve_attachment_root(
    live: &Path,
    preservation: &Path,
    moved: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), StorageError> {
    let source = live
        .parent()
        .ok_or_else(StorageError::recovery_preservation)?
        .join("attachments");
    if !source.exists() {
        return Ok(());
    }
    let destination = preservation.join("attachments");
    if fs::rename(&source, &destination).is_err() {
        rollback_preserved_bundle(moved);
        let _ = fs::remove_dir(preservation);
        return Err(StorageError::recovery_preservation());
    }
    moved.push((source, destination));
    Ok(())
}

/// Resolves the exact main database plus WAL and shared-memory sidecar paths.
fn database_bundle_paths(live: &Path) -> Vec<PathBuf> {
    let mut paths = vec![live.to_path_buf()];
    for suffix in SQLITE_SIDECAR_SUFFIXES {
        let mut sidecar = live.as_os_str().to_os_string();
        sidecar.push(suffix);
        paths.push(PathBuf::from(sidecar));
    }
    paths
}

/// Chooses a unique native-only directory for rehydrating a recovery snapshot's attachment bytes.
fn recovery_attachment_staging_path(live: &Path) -> Result<PathBuf, StorageError> {
    let parent = live.parent().ok_or_else(StorageError::restore)?;
    Ok(parent.join(format!(
        "{RECOVERY_ATTACHMENT_STAGING_PREFIX}-{}",
        uuid::Uuid::new_v4()
    )))
}
