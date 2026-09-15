//! Transactional app-owned cache for an explicitly imported private worker bundle.

use std::{
    fs::{self, File},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    availability::{WorkerInstallationReadiness, inspect_worker_installation},
    model_package::PackageAcceptanceEvidence,
    worker_bundle::hash_worker_bundle,
};

const CACHE_ID_DOMAIN: &[u8] = b"bottie-local-image-worker-cache-v1";
const PACKAGES_DIRECTORY: &str = "packages";
const STAGING_DIRECTORY: &str = "staging";

/// Stable worker-cache failures without native locations or integrity values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkerCacheError {
    /// The app-owned root, source root, or executable name was not a safe native layout.
    UnsafeLayout,
    /// The exact native hardware/readiness gate does not permit this worker installation.
    Unavailable,
    /// The path-free approval did not exactly acknowledge the selected worker contract.
    ApprovalRequired,
    /// Another native image operation owns a mutually exclusive lifecycle boundary.
    Busy,
    /// Source, staged, or promoted bytes did not match the accepted worker evidence.
    Integrity,
    /// App-owned filesystem work could not be completed durably.
    Storage,
}

/// Path-free acknowledgement required before Bottie copies a user-selected worker bundle.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WorkerImportApproval {
    /// Exact accepted runtime identity shown to the user.
    pub(crate) runtime_id: String,
    /// Exact accepted worker-bundle bytes shown to the user.
    pub(crate) expected_disk_bytes: u64,
    /// Affirmative acknowledgement; false never permits native mutation.
    pub(crate) approved: bool,
}

/// Native-only location of one freshly verified promoted worker bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PromotedWorker {
    bundle_root: PathBuf,
    executable: PathBuf,
}

impl PromotedWorker {
    /// Returns the app-owned promoted bundle root without crossing IPC.
    pub(crate) fn bundle_root(&self) -> &Path {
        &self.bundle_root
    }

    /// Returns the app-owned promoted executable without crossing IPC.
    pub(crate) fn executable(&self) -> &Path {
        &self.executable
    }
}

/// Copies one pre-verified native bundle into staging, re-verifies it, and atomically promotes it.
pub(crate) fn import_worker_bundle(
    cache_root: &Path,
    source_root: &Path,
    executable_name: &str,
    runtime_id: &str,
    evidence: &PackageAcceptanceEvidence,
    approval: &WorkerImportApproval,
) -> Result<PromotedWorker, WorkerCacheError> {
    validate_approval(runtime_id, evidence, approval)?;
    validate_layout(cache_root, source_root, executable_name)?;
    verify_bundle(source_root, executable_name, evidence)?;

    let cache_root = prepare_directory(cache_root)?;
    let packages = ensure_child_directory(&cache_root, PACKAGES_DIRECTORY)?;
    let staging = ensure_child_directory(&cache_root, STAGING_DIRECTORY)?;
    let final_root = packages.join(cache_identity(runtime_id, evidence));
    match inspect_at(&final_root, executable_name, evidence) {
        Ok(Some(promoted)) => return Ok(promoted),
        Ok(None) | Err(WorkerCacheError::Integrity) => {}
        Err(error) => return Err(error),
    }

    let staging_root = staging.join(uuid::Uuid::new_v4().to_string());
    fs::create_dir(&staging_root).map_err(|_| WorkerCacheError::Storage)?;
    sync_directory(&staging).map_err(|_| WorkerCacheError::Storage)?;
    let result = (|| {
        copy_tree(source_root, &staging_root)?;
        sync_directory(&staging_root).map_err(|_| WorkerCacheError::Storage)?;
        verify_bundle(&staging_root, executable_name, evidence)?;
        if fs::symlink_metadata(&final_root).is_ok() {
            remove_immediate_child(&packages, &final_root)?;
        }
        fs::rename(&staging_root, &final_root).map_err(|_| WorkerCacheError::Storage)?;
        sync_directory(&staging).map_err(|_| WorkerCacheError::Storage)?;
        sync_directory(&packages).map_err(|_| WorkerCacheError::Storage)?;
        inspect_at(&final_root, executable_name, evidence)?.ok_or(WorkerCacheError::Integrity)
    })();
    if staging_root.exists() {
        let _ = fs::remove_dir_all(&staging_root);
        let _ = sync_directory(&staging);
    }
    result
}

/// Resolves only the deterministic app-owned promoted worker and performs no cache mutation.
pub(crate) fn resolve_promoted_worker(
    cache_root: &Path,
    executable_name: &str,
    runtime_id: &str,
    evidence: &PackageAcceptanceEvidence,
) -> Result<Option<PromotedWorker>, WorkerCacheError> {
    validate_cache_root(cache_root)?;
    validate_executable_name(executable_name)?;
    if path_is_missing(cache_root)? {
        return Ok(None);
    }
    let metadata = fs::symlink_metadata(cache_root).map_err(|_| WorkerCacheError::Storage)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(WorkerCacheError::Integrity);
    }
    let cache_root = cache_root
        .canonicalize()
        .map_err(|_| WorkerCacheError::Storage)?;
    let final_root = cache_root
        .join(PACKAGES_DIRECTORY)
        .join(cache_identity(runtime_id, evidence));
    inspect_at(&final_root, executable_name, evidence)
}

fn validate_approval(
    runtime_id: &str,
    evidence: &PackageAcceptanceEvidence,
    approval: &WorkerImportApproval,
) -> Result<(), WorkerCacheError> {
    if !approval.approved
        || approval.runtime_id != runtime_id
        || approval.expected_disk_bytes != evidence.worker_bundle_byte_size
    {
        return Err(WorkerCacheError::ApprovalRequired);
    }
    Ok(())
}

fn validate_layout(
    cache_root: &Path,
    source_root: &Path,
    executable_name: &str,
) -> Result<(), WorkerCacheError> {
    validate_cache_root(cache_root)?;
    validate_cache_root(source_root)?;
    validate_executable_name(executable_name)?;
    let source = source_root
        .canonicalize()
        .map_err(|_| WorkerCacheError::UnsafeLayout)?;
    let cache = canonical_cache_location(cache_root)?;
    if source.starts_with(&cache) || cache.starts_with(&source) {
        return Err(WorkerCacheError::UnsafeLayout);
    }
    Ok(())
}

fn canonical_cache_location(path: &Path) -> Result<PathBuf, WorkerCacheError> {
    if path.exists() {
        return path
            .canonicalize()
            .map_err(|_| WorkerCacheError::UnsafeLayout);
    }
    let parent = path.parent().ok_or(WorkerCacheError::UnsafeLayout)?;
    let name = path.file_name().ok_or(WorkerCacheError::UnsafeLayout)?;
    Ok(parent
        .canonicalize()
        .map_err(|_| WorkerCacheError::UnsafeLayout)?
        .join(name))
}

fn validate_cache_root(path: &Path) -> Result<(), WorkerCacheError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(WorkerCacheError::UnsafeLayout);
    }
    Ok(())
}

fn validate_executable_name(name: &str) -> Result<(), WorkerCacheError> {
    let path = Path::new(name);
    if name.is_empty()
        || path.is_absolute()
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(WorkerCacheError::UnsafeLayout);
    }
    Ok(())
}

fn verify_bundle(
    root: &Path,
    executable_name: &str,
    evidence: &PackageAcceptanceEvidence,
) -> Result<(), WorkerCacheError> {
    let measured = hash_worker_bundle(root, &root.join(executable_name))
        .map_err(|_| WorkerCacheError::Integrity)?;
    if measured.executable_sha256 != evidence.worker_sha256
        || measured.executable_byte_size != evidence.worker_byte_size
        || measured.bundle_sha256 != evidence.worker_bundle_sha256
        || measured.bundle_byte_size != evidence.worker_bundle_byte_size
    {
        return Err(WorkerCacheError::Integrity);
    }
    Ok(())
}

fn inspect_at(
    root: &Path,
    executable_name: &str,
    evidence: &PackageAcceptanceEvidence,
) -> Result<Option<PromotedWorker>, WorkerCacheError> {
    let executable = root.join(executable_name);
    match inspect_worker_installation(root, &executable, evidence) {
        WorkerInstallationReadiness::Missing => Ok(None),
        WorkerInstallationReadiness::Mismatch => Err(WorkerCacheError::Integrity),
        WorkerInstallationReadiness::Verified => Ok(Some(PromotedWorker {
            bundle_root: root.to_path_buf(),
            executable,
        })),
    }
}

fn cache_identity(runtime_id: &str, evidence: &PackageAcceptanceEvidence) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CACHE_ID_DOMAIN);
    update_field(&mut hasher, runtime_id.as_bytes());
    update_field(&mut hasher, evidence.runtime_revision.as_bytes());
    update_field(&mut hasher, evidence.worker_sha256.as_bytes());
    hasher.update(evidence.worker_byte_size.to_be_bytes());
    update_field(&mut hasher, evidence.worker_bundle_sha256.as_bytes());
    hasher.update(evidence.worker_bundle_byte_size.to_be_bytes());
    format!("{:x}", hasher.finalize())
}

fn update_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn prepare_directory(path: &Path) -> Result<PathBuf, WorkerCacheError> {
    if !path.exists() {
        fs::create_dir_all(path).map_err(|_| WorkerCacheError::Storage)?;
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| WorkerCacheError::Storage)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(WorkerCacheError::Integrity);
    }
    path.canonicalize().map_err(|_| WorkerCacheError::Storage)
}

fn ensure_child_directory(root: &Path, name: &str) -> Result<PathBuf, WorkerCacheError> {
    let path = root.join(name);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(path),
        Ok(_) => Err(WorkerCacheError::Integrity),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&path).map_err(|_| WorkerCacheError::Storage)?;
            sync_directory(root).map_err(|_| WorkerCacheError::Storage)?;
            Ok(path)
        }
        Err(_) => Err(WorkerCacheError::Storage),
    }
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), WorkerCacheError> {
    for entry in fs::read_dir(source).map_err(|_| WorkerCacheError::Integrity)? {
        let entry = entry.map_err(|_| WorkerCacheError::Integrity)?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata =
            fs::symlink_metadata(&source_path).map_err(|_| WorkerCacheError::Integrity)?;
        if metadata.file_type().is_symlink() {
            return Err(WorkerCacheError::Integrity);
        }
        if metadata.is_dir() {
            fs::create_dir(&destination_path).map_err(|_| WorkerCacheError::Storage)?;
            copy_tree(&source_path, &destination_path)?;
            sync_directory(&destination_path).map_err(|_| WorkerCacheError::Storage)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &destination_path).map_err(|_| WorkerCacheError::Storage)?;
            File::open(&destination_path)
                .and_then(|file| file.sync_all())
                .map_err(|_| WorkerCacheError::Storage)?;
        } else {
            return Err(WorkerCacheError::Integrity);
        }
    }
    Ok(())
}

fn remove_immediate_child(parent: &Path, path: &Path) -> Result<(), WorkerCacheError> {
    if path.parent() != Some(parent) {
        return Err(WorkerCacheError::Integrity);
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| WorkerCacheError::Storage)?;
    if metadata.file_type().is_symlink() || metadata.is_file() {
        fs::remove_file(path).map_err(|_| WorkerCacheError::Storage)?;
    } else if metadata.is_dir() {
        fs::remove_dir_all(path).map_err(|_| WorkerCacheError::Storage)?;
    } else {
        return Err(WorkerCacheError::Integrity);
    }
    sync_directory(parent).map_err(|_| WorkerCacheError::Storage)
}

fn path_is_missing(path: &Path) -> Result<bool, WorkerCacheError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(_) => Err(WorkerCacheError::Storage),
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(windows)]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    fs::OpenOptions::new()
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?
        .sync_all()
}
