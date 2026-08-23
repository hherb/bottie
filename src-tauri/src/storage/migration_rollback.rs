//! Isolated schema migration, verified source recovery points, and journalled promotion.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

use rusqlite::{Connection, MAIN_DB};
use serde::{Deserialize, Serialize};

use super::{
    CURRENT_SCHEMA_VERSION, ConversationStore, StorageError,
    backup::{copy_database, remove_database_files},
    migration_validation::{
        SourceSnapshot, read_only_connection, schema_is_empty, source_snapshot,
        validate_connection, validate_database, validate_source_snapshot,
    },
    now_ms,
};

const CANDIDATE_PREFIX: &str = ".bottie-migration-candidate";
const MARKER_FILE: &str = ".bottie-migration-promotion.json";
const MARKER_TEMP_PREFIX: &str = ".bottie-migration-marker";
const MARKER_MAX_BYTES: u64 = 4 * 1_024;
const RECOVERY_DIRECTORY: &str = "migration-backups";
const RECOVERY_PREFIX: &str = "bottie-migration";
const REPLACEMENT_PREFIX: &str = ".bottie-migration-replacement";
const DISPLACED_PREFIX: &str = ".bottie-migration-displaced";
const RECOVERY_RETENTION_COUNT: usize = 2;

/// Deterministic storage fault points used only by path-backed migration tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) enum MigrationFault {
    /// Production behavior without an injected failure.
    None,
    /// Stop before copying the live store into an isolated candidate.
    BeforeCandidateCopy,
    /// Stop after candidate copy but before any pending migration runs.
    BeforeCandidateMigration,
    /// Stop after candidate migration but before candidate acceptance.
    AfterCandidateMigration,
    /// Reject an otherwise migrated candidate at the validation boundary.
    DuringCandidateValidation,
    /// Stop before creating the source-version recovery point.
    BeforeSafetyCopy,
    /// Reject the recovery point after its online copy completes.
    DuringSafetyCopyValidation,
    /// Fail the atomic marker write after candidate and recovery validation.
    DuringMarkerWrite,
    /// Stop after the durable promotion marker is written.
    AfterPromotionMarker,
    /// Treat the installed candidate as a failed live promotion and restore the source.
    DuringLivePromotion,
    /// Stop after target validation but before marker and candidate cleanup.
    AfterLivePromotion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StoreState {
    New,
    Current,
    Older(i64),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct PromotionMarker {
    operation_id: String,
    source_version: i64,
    target_version: i64,
    candidate_file: String,
    recovery_file: String,
    phase: String,
}

#[derive(Debug)]
struct RecoveryPoint {
    timestamp_ms: i64,
    path: PathBuf,
}

/// Preflights one store and stages any supported upgrade without mutating attachments.
pub(super) fn prepare_store(live: &Path, fault: MigrationFault) -> Result<(), StorageError> {
    if let Some(parent) = live.parent() {
        fs::create_dir_all(parent).map_err(|_| StorageError::migration())?;
    }
    reconcile_promotion(live)?;
    match classify_store(live)? {
        StoreState::New | StoreState::Current => Ok(()),
        StoreState::Older(source_version) => staged_migration(live, source_version, fault),
    }
}

/// Returns the fixed marker path so startup corruption classification can reconcile it first.
pub(super) fn migration_marker_path(live: &Path) -> PathBuf {
    live.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(MARKER_FILE)
}

/// Classifies the source through read-only SQLite connections and exact ledger validation.
fn classify_store(live: &Path) -> Result<StoreState, StorageError> {
    if !live.exists()
        || fs::metadata(live)
            .map_err(|_| StorageError::migration())?
            .len()
            == 0
    {
        return Ok(StoreState::New);
    }
    let connection = read_only_connection(live)?;
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| StorageError::migration())?;
    if version == 0 && schema_is_empty(&connection)? {
        return Ok(StoreState::New);
    }
    if version > CURRENT_SCHEMA_VERSION {
        return Err(StorageError::newer_schema());
    }
    validate_connection(&connection, version, version == CURRENT_SCHEMA_VERSION)?;
    if version == CURRENT_SCHEMA_VERSION {
        Ok(StoreState::Current)
    } else {
        Ok(StoreState::Older(version))
    }
}

/// Migrates and validates an isolated candidate before journalled live promotion.
fn staged_migration(
    live: &Path,
    source_version: i64,
    fault: MigrationFault,
) -> Result<(), StorageError> {
    let operation_id = uuid::Uuid::new_v4();
    let candidate = candidate_path(live, operation_id)?;
    let source_snapshot = source_snapshot(live)?;
    if fault == MigrationFault::BeforeCandidateCopy {
        return Err(StorageError::migration());
    }
    if copy_database(live, &candidate).is_err() {
        remove_database_files(&candidate);
        return Err(StorageError::migration());
    }
    let candidate_store = ConversationStore {
        path: candidate.clone(),
        recovery_required: Arc::new(AtomicBool::new(false)),
    };
    let candidate_result = (|| {
        if fault == MigrationFault::BeforeCandidateMigration {
            return Err(StorageError::migration());
        }
        let mut connection = candidate_store.open_unchecked()?;
        candidate_store.migrate(&mut connection)?;
        drop(connection);
        if fault == MigrationFault::AfterCandidateMigration {
            return Err(StorageError::migration());
        }
        validate_database(&candidate, CURRENT_SCHEMA_VERSION, true)?;
        if fault == MigrationFault::DuringCandidateValidation {
            return Err(StorageError::migration());
        }
        validate_source_snapshot(&candidate, &source_snapshot)
    })();
    if candidate_result.is_err() {
        remove_database_files(&candidate);
        return Err(StorageError::migration());
    }
    if fault == MigrationFault::BeforeSafetyCopy {
        remove_database_files(&candidate);
        return Err(StorageError::migration());
    }

    let recovery = recovery_path(live, operation_id, source_version)?;
    if copy_database(live, &recovery).is_err()
        || fault == MigrationFault::DuringSafetyCopyValidation
        || validate_database(&recovery, source_version, false).is_err()
        || validate_source_snapshot(&recovery, &source_snapshot).is_err()
    {
        remove_database_files(&candidate);
        remove_database_files(&recovery);
        return Err(StorageError::migration());
    }
    let marker = PromotionMarker {
        operation_id: operation_id.to_string(),
        source_version,
        target_version: CURRENT_SCHEMA_VERSION,
        candidate_file: leaf_name(&candidate)?,
        recovery_file: leaf_name(&recovery)?,
        phase: "prepared".into(),
    };
    let marker_result = if fault == MigrationFault::DuringMarkerWrite {
        Err(StorageError::migration())
    } else {
        write_marker(live, &marker)
    };
    if marker_result.is_err() {
        remove_database_files(&candidate);
        remove_database_files(&recovery);
        return Err(StorageError::migration());
    }
    if fault == MigrationFault::AfterPromotionMarker {
        return Err(StorageError::migration());
    }

    let promoted = restore_database(&candidate, live)
        .and_then(|()| validate_database(live, CURRENT_SCHEMA_VERSION, true))
        .and_then(|()| validate_source_snapshot(live, &source_snapshot));
    if promoted.is_err() || fault == MigrationFault::DuringLivePromotion {
        return rollback_failed_promotion(
            live,
            &candidate,
            &recovery,
            source_version,
            &source_snapshot,
        );
    }
    if fault == MigrationFault::AfterLivePromotion {
        return Err(StorageError::migration());
    }
    finish_promotion(live, &candidate)
}

/// Restores a verified source recovery point after a failed live promotion.
fn rollback_failed_promotion(
    live: &Path,
    candidate: &Path,
    recovery: &Path,
    source_version: i64,
    source_snapshot: &SourceSnapshot,
) -> Result<(), StorageError> {
    let restored = restore_database(recovery, live)
        .and_then(|()| validate_database(live, source_version, false))
        .and_then(|()| validate_source_snapshot(live, source_snapshot));
    if restored.is_ok() {
        remove_database_files(candidate);
        remove_marker(live)?;
    }
    Err(StorageError::migration())
}

/// Reconciles a durable marker before ordinary startup corruption classification.
pub(super) fn reconcile_promotion(live: &Path) -> Result<(), StorageError> {
    let marker_path = migration_marker_path(live);
    if !marker_path.exists() {
        return Ok(());
    }
    let marker = read_marker(&marker_path)?;
    let (candidate, recovery) = marker_paths(live, &marker)?;
    if validate_database(live, marker.target_version, true).is_ok() {
        return finish_promotion(live, &candidate);
    }
    if validate_database(&recovery, marker.source_version, false).is_err()
        || restore_database(&recovery, live).is_err()
        || validate_database(live, marker.source_version, false).is_err()
    {
        return Err(StorageError::migration());
    }
    remove_database_files(&candidate);
    remove_marker(live)?;
    Err(StorageError::migration())
}

/// Removes only the exact candidate and marker, then keeps two strict recovery points.
fn finish_promotion(live: &Path, candidate: &Path) -> Result<(), StorageError> {
    remove_database_files(candidate);
    remove_marker(live)?;
    prune_recovery_points(live)?;
    Ok(())
}

/// Restores a source database into one destination through SQLite's online backup API.
fn restore_database(source: &Path, destination: &Path) -> Result<(), StorageError> {
    if Connection::open(destination)
        .and_then(|mut live| live.restore(MAIN_DB, source, None::<fn(rusqlite::backup::Progress)>))
        .is_ok()
    {
        return Ok(());
    }
    replace_database(source, destination)
}

/// Falls back to a same-volume replacement when SQLite cannot open the damaged destination.
fn replace_database(source: &Path, destination: &Path) -> Result<(), StorageError> {
    let parent = destination.parent().ok_or_else(StorageError::migration)?;
    let operation_id = uuid::Uuid::new_v4();
    let replacement = parent.join(format!("{REPLACEMENT_PREFIX}-{operation_id}.sqlite3"));
    let displaced = parent.join(format!("{DISPLACED_PREFIX}-{operation_id}.sqlite3"));
    copy_database(source, &replacement).map_err(|_| StorageError::migration())?;
    remove_sidecars(destination);
    let displaced_live = destination.exists();
    if displaced_live && fs::rename(destination, &displaced).is_err() {
        remove_database_files(&replacement);
        return Err(StorageError::migration());
    }
    if fs::rename(&replacement, destination).is_err() {
        if displaced_live {
            let _ = fs::rename(&displaced, destination);
        }
        remove_database_files(&replacement);
        return Err(StorageError::migration());
    }
    remove_database_files(&displaced);
    Ok(())
}

/// Removes only the WAL and shared-memory files associated with one exact database.
fn remove_sidecars(database: &Path) {
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = database.as_os_str().to_os_string();
        sidecar.push(suffix);
        let _ = fs::remove_file(PathBuf::from(sidecar));
    }
}

/// Writes one bounded marker through a same-directory synced temporary file and rename.
fn write_marker(live: &Path, marker: &PromotionMarker) -> Result<(), StorageError> {
    let path = migration_marker_path(live);
    let temporary = path.with_file_name(format!("{MARKER_TEMP_PREFIX}-{}", uuid::Uuid::new_v4()));
    let bytes = serde_json::to_vec(marker).map_err(|_| StorageError::migration())?;
    if bytes.len() as u64 > MARKER_MAX_BYTES {
        return Err(StorageError::migration());
    }
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, &path)?;
        Ok::<(), std::io::Error>(())
    })();
    let _ = fs::remove_file(&temporary);
    result.map_err(|_| StorageError::migration())
}

/// Loads a strict bounded promotion marker without accepting extra fields.
fn read_marker(path: &Path) -> Result<PromotionMarker, StorageError> {
    let mut file = File::open(path).map_err(|_| StorageError::migration())?;
    if file
        .metadata()
        .map_err(|_| StorageError::migration())?
        .len()
        > MARKER_MAX_BYTES
    {
        return Err(StorageError::migration());
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|_| StorageError::migration())?;
    serde_json::from_slice(&bytes).map_err(|_| StorageError::migration())
}

/// Removes the fixed promotion marker after a target or source has validated.
fn remove_marker(live: &Path) -> Result<(), StorageError> {
    let marker = migration_marker_path(live);
    if marker.exists() {
        fs::remove_file(marker).map_err(|_| StorageError::migration())?;
    }
    Ok(())
}

/// Resolves and validates the two strict managed leaf names inside a marker.
fn marker_paths(live: &Path, marker: &PromotionMarker) -> Result<(PathBuf, PathBuf), StorageError> {
    let operation_id =
        uuid::Uuid::parse_str(&marker.operation_id).map_err(|_| StorageError::migration())?;
    if marker.target_version != CURRENT_SCHEMA_VERSION
        || marker.source_version < 1
        || marker.source_version >= marker.target_version
        || marker.phase != "prepared"
        || !is_strict_leaf(&marker.candidate_file)
        || !is_strict_leaf(&marker.recovery_file)
        || marker.candidate_file != candidate_file_name(operation_id)
        || recovery_metadata(Path::new(&marker.recovery_file))
            .is_none_or(|(_, id, version)| id != operation_id || version != marker.source_version)
    {
        return Err(StorageError::migration());
    }
    let parent = live.parent().ok_or_else(StorageError::migration)?;
    Ok((
        parent.join(&marker.candidate_file),
        parent.join(RECOVERY_DIRECTORY).join(&marker.recovery_file),
    ))
}

/// Accepts one UTF-8 filename component without parent, root, or traversal syntax.
fn is_strict_leaf(value: &str) -> bool {
    let path = Path::new(value);
    path.components().count() == 1 && path.file_name().and_then(|name| name.to_str()) == Some(value)
}

/// Builds one same-directory candidate path from its operation identity.
fn candidate_path(live: &Path, operation_id: uuid::Uuid) -> Result<PathBuf, StorageError> {
    Ok(live
        .parent()
        .ok_or_else(StorageError::migration)?
        .join(candidate_file_name(operation_id)))
}

/// Builds the strict candidate leaf name.
fn candidate_file_name(operation_id: uuid::Uuid) -> String {
    format!("{CANDIDATE_PREFIX}-{operation_id}.sqlite3")
}

/// Builds one source-version recovery path inside the dedicated managed directory.
fn recovery_path(
    live: &Path,
    operation_id: uuid::Uuid,
    source_version: i64,
) -> Result<PathBuf, StorageError> {
    let directory = live
        .parent()
        .ok_or_else(StorageError::migration)?
        .join(RECOVERY_DIRECTORY);
    fs::create_dir_all(&directory).map_err(|_| StorageError::migration())?;
    Ok(directory.join(format!(
        "{RECOVERY_PREFIX}-{}-{operation_id}-v{source_version}.sqlite3",
        now_ms().map_err(|_| StorageError::migration())?
    )))
}

/// Returns a native-only leaf filename without permitting directory traversal.
fn leaf_name(path: &Path) -> Result<String, StorageError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .ok_or_else(StorageError::migration)
}

/// Parses only exact managed migration recovery filenames.
fn recovery_metadata(path: &Path) -> Option<(i64, uuid::Uuid, i64)> {
    let name = path.file_name()?.to_str()?;
    let body = name
        .strip_prefix(&format!("{RECOVERY_PREFIX}-"))?
        .strip_suffix(".sqlite3")?;
    let version_at = body.rfind("-v")?;
    let source_version = body[(version_at + 2)..].parse().ok()?;
    let timestamp_and_id = &body[..version_at];
    let id_at = timestamp_and_id.find('-')?;
    let timestamp_ms = timestamp_and_id[..id_at].parse().ok()?;
    let operation_id = uuid::Uuid::parse_str(&timestamp_and_id[(id_at + 1)..]).ok()?;
    Some((timestamp_ms, operation_id, source_version))
}

/// Lists only regular files matching the exact managed recovery-point contract.
pub(super) fn managed_recovery_points(live: &Path) -> Result<Vec<PathBuf>, StorageError> {
    let directory = live
        .parent()
        .ok_or_else(StorageError::migration)?
        .join(RECOVERY_DIRECTORY);
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut points = fs::read_dir(directory)
        .map_err(|_| StorageError::migration())?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let (timestamp_ms, _, _) = recovery_metadata(&entry.path())?;
            Some(RecoveryPoint {
                timestamp_ms,
                path: entry.path(),
            })
        })
        .collect::<Vec<_>>();
    points.sort_by(|left, right| {
        right
            .timestamp_ms
            .cmp(&left.timestamp_ms)
            .then_with(|| right.path.cmp(&left.path))
    });
    Ok(points.into_iter().map(|point| point.path).collect())
}

/// Retains the two newest completed managed recovery points and ignores lookalikes.
pub(super) fn prune_recovery_points(live: &Path) -> Result<(), StorageError> {
    let protected = if migration_marker_path(live).exists() {
        let marker = read_marker(&migration_marker_path(live))?;
        Some(marker_paths(live, &marker)?.1)
    } else {
        None
    };
    let mut retained = 0;
    for point in managed_recovery_points(live)? {
        if protected.as_ref() == Some(&point) {
            continue;
        }
        if retained < RECOVERY_RETENTION_COUNT {
            retained += 1;
        } else {
            fs::remove_file(point).map_err(|_| StorageError::migration())?;
        }
    }
    Ok(())
}
