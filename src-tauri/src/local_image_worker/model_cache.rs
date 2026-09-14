//! Transactional app-owned cache for exact local image-model packages.

use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use super::{
    model_acquisition::{
        AcquisitionError, ModelAcquisition, ModelFileContract, ModelPackageManifest,
    },
    protocol::ModelLocation,
    transport::{TransportError, WorkerTransport},
};

#[path = "model_cache/filesystem.rs"]
mod filesystem;
#[path = "model_cache/tree.rs"]
mod tree;
#[path = "model_cache/writer.rs"]
mod writer;

use filesystem::{
    hash_prefix, open_for_append, remove_failed_file, remove_managed_entry,
    symlink_metadata_if_exists, sync_directory, sync_parent, verify_contract_file,
};
use tree::validate_cache_tree;
pub(crate) use writer::CacheFileWriter;

/// Directory containing resumable transactions.
const STAGING_DIRECTORY: &str = "staging";
/// Directory containing fully verified packages.
const PACKAGES_DIRECTORY: &str = "packages";
/// Native-only exact manifest retained with staged and promoted bytes.
const MANIFEST_FILE: &str = ".bottie-model-manifest.json";
/// Opaque digest binding resumable bytes to their exact repository source plan.
const SOURCE_BINDING_FILE: &str = ".bottie-model-source-binding";
/// Prefix for a same-directory durable manifest write.
const MANIFEST_TEMP_PREFIX: &str = ".bottie-model-manifest";
/// Maximum serialized exact manifest accepted from the cache.
const MAX_MANIFEST_BYTES: u64 = 8 * 1_024 * 1_024;
/// Buffer used to hash prior bytes and stream injected source bytes.
const STREAM_BUFFER_BYTES: usize = 64 * 1_024;

/// Stable cache failures that reveal no native path, hash, or file contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CacheError {
    /// The supplied exact package manifest did not pass native policy.
    InvalidManifest,
    /// A requested resume offset or cache lifecycle transition was invalid.
    InvalidState,
    /// The requested file was not one exact declared portable path.
    InvalidPath,
    /// Cached bytes, file types, or containment did not match the exact package.
    Integrity,
    /// A local source ended with an I/O error after retaining resumable bytes.
    Interrupted,
    /// App-owned filesystem work could not be completed durably.
    Storage,
}

/// Path-free failure from cache verification followed by private worker loading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CachedModelLoadError {
    /// The promoted package was absent, unsafe, or no longer exact.
    Cache(CacheError),
    /// The private worker rejected or could not receive the verified load request.
    Transport(TransportError),
}

/// Result of streaming one bounded segment into an exact declared file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CacheWriteStatus {
    /// Valid bytes were synced but the exact file remains incomplete.
    Incomplete,
    /// Exact size and SHA-256 matched and the file is ready for package verification.
    Complete,
}

/// Deterministic promotion boundary used by path-backed atomicity tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CachePromotionFault {
    /// Normal production behavior.
    None,
    /// Stop after full verification but before the atomic directory rename.
    BeforeRename,
}

/// One resumable transaction bound to an exact native package manifest.
#[derive(Debug)]
pub(crate) struct ModelCacheTransaction {
    manifest: ModelPackageManifest,
    staging_root: PathBuf,
    final_root: PathBuf,
    staging_parent: PathBuf,
    packages_parent: PathBuf,
}

impl ModelCacheTransaction {
    /// Opens or creates the single resumable slot for this exact model identity.
    pub(crate) fn open(
        cache_root: &Path,
        manifest: ModelPackageManifest,
    ) -> Result<Self, CacheError> {
        Self::open_internal(cache_root, manifest, None)
    }

    /// Opens a transaction whose retained partials must match one exact opaque source-plan digest.
    pub(crate) fn open_bound(
        cache_root: &Path,
        manifest: ModelPackageManifest,
        source_binding: &str,
    ) -> Result<Self, CacheError> {
        if !valid_source_binding(source_binding) {
            return Err(CacheError::InvalidState);
        }
        Self::open_internal(cache_root, manifest, Some(source_binding))
    }

    fn open_internal(
        cache_root: &Path,
        manifest: ModelPackageManifest,
        source_binding: Option<&str>,
    ) -> Result<Self, CacheError> {
        validate_manifest(&manifest)?;
        let cache_root = prepare_cache_root(cache_root)?;
        let staging_parent = ensure_managed_directory(&cache_root, STAGING_DIRECTORY)?;
        let packages_parent = ensure_managed_directory(&cache_root, PACKAGES_DIRECTORY)?;
        let manifest_bytes = manifest_bytes(&manifest)?;
        let staging_root = staging_parent.join(identity_digest(manifest.model_id.as_bytes()));
        let final_root = packages_parent.join(identity_digest(&manifest_bytes));

        if let Some(metadata) = symlink_metadata_if_exists(&staging_root)? {
            let resumes_exactly = metadata.is_dir()
                && !metadata.file_type().is_symlink()
                && read_manifest(&staging_root).is_ok_and(|cached| cached == manifest)
                && source_binding.is_none_or(|expected| {
                    read_source_binding(&staging_root).is_ok_and(|cached| cached == expected)
                })
                && validate_cache_tree(&staging_root, &manifest).is_ok();
            if !resumes_exactly {
                remove_managed_entry(&staging_parent, &staging_root)?;
            }
        }
        if symlink_metadata_if_exists(&staging_root)?.is_none() {
            fs::create_dir(&staging_root).map_err(|_| CacheError::Storage)?;
            sync_directory(&staging_parent).map_err(|_| CacheError::Storage)?;
            write_manifest(&staging_root, &manifest_bytes)?;
            if let Some(source_binding) = source_binding {
                write_source_binding(&staging_root, source_binding)?;
            }
        }
        validate_cache_tree(&staging_root, &manifest)?;
        Ok(Self {
            manifest,
            staging_root,
            final_root,
            staging_parent,
            packages_parent,
        })
    }

    /// Returns the exact durable byte offset accepted for one declared file.
    pub(crate) fn resume_offset(&self, relative_path: &str) -> Result<u64, CacheError> {
        let contract = self.contract(relative_path)?;
        let path = self.secure_file_path(contract, false)?;
        let Some(metadata) = symlink_metadata_if_exists(&path)? else {
            return Ok(0);
        };
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > contract.byte_size
        {
            return Err(CacheError::Integrity);
        }
        if metadata.len() == contract.byte_size {
            verify_contract_file(&path, contract)?;
        }
        Ok(metadata.len())
    }

    /// Streams and hashes bytes at one exact resume offset, retaining only valid partials.
    pub(crate) fn write_file(
        &self,
        relative_path: &str,
        expected_offset: u64,
        source: &mut dyn Read,
    ) -> Result<CacheWriteStatus, CacheError> {
        let mut writer = self.begin_file_write(relative_path, expected_offset)?;
        let mut buffer = [0_u8; STREAM_BUFFER_BYTES];
        loop {
            let count = match source.read(&mut buffer) {
                Ok(count) => count,
                Err(_) => return writer.retain_partial(CacheError::Interrupted),
            };
            if count == 0 {
                break;
            }
            writer.append(&buffer[..count])?;
        }
        writer.finish()
    }

    /// Opens one exact file once so an async downloader can hash and append without quadratic re-reads.
    pub(crate) fn begin_file_write(
        &self,
        relative_path: &str,
        expected_offset: u64,
    ) -> Result<CacheFileWriter, CacheError> {
        let contract = self.contract(relative_path)?;
        let path = self.secure_file_path(contract, true)?;
        let current_offset = self.resume_offset(relative_path)?;
        if current_offset != expected_offset || current_offset == contract.byte_size {
            return Err(CacheError::InvalidState);
        }
        CacheFileWriter::open(path, current_offset, contract)
    }

    /// Verifies and atomically promotes a complete transaction into the immutable package area.
    pub(crate) fn promote(&self) -> Result<ModelLocation, CacheError> {
        self.promote_with_fault(CachePromotionFault::None)
    }

    /// Applies one deterministic failure immediately before the only visible promotion rename.
    pub(crate) fn promote_with_fault(
        &self,
        fault: CachePromotionFault,
    ) -> Result<ModelLocation, CacheError> {
        if read_manifest(&self.staging_root)? != self.manifest {
            return Err(CacheError::Integrity);
        }
        validate_cache_tree(&self.staging_root, &self.manifest)?;
        activate_package(&self.staging_root, &self.manifest)?;
        if symlink_metadata_if_exists(&self.final_root)?.is_some() {
            match reopen_exact_package(&self.final_root, &self.manifest) {
                Ok(location) => {
                    remove_managed_entry(&self.staging_parent, &self.staging_root)?;
                    return Ok(location);
                }
                Err(CacheError::Integrity) => {
                    remove_managed_entry(&self.packages_parent, &self.final_root)?;
                }
                Err(error) => return Err(error),
            }
        }
        if fault == CachePromotionFault::BeforeRename {
            return Err(CacheError::Storage);
        }
        fs::rename(&self.staging_root, &self.final_root).map_err(|_| CacheError::Storage)?;
        sync_directory(&self.staging_parent).map_err(|_| CacheError::Storage)?;
        sync_directory(&self.packages_parent).map_err(|_| CacheError::Storage)?;
        reopen_exact_package(&self.final_root, &self.manifest)
    }

    /// Discards only this model identity's exact resumable staging slot.
    pub(crate) fn discard(&self) -> Result<(), CacheError> {
        remove_managed_entry(&self.staging_parent, &self.staging_root)
    }

    fn contract(&self, relative_path: &str) -> Result<&ModelFileContract, CacheError> {
        self.manifest
            .files
            .iter()
            .find(|contract| contract.relative_path == relative_path)
            .ok_or(CacheError::InvalidPath)
    }

    fn secure_file_path(
        &self,
        contract: &ModelFileContract,
        create_parents: bool,
    ) -> Result<PathBuf, CacheError> {
        let relative = Path::new(&contract.relative_path);
        let mut current = self.staging_root.clone();
        if let Some(parent) = relative.parent() {
            for component in parent.components() {
                let next = current.join(component.as_os_str());
                match symlink_metadata_if_exists(&next)? {
                    Some(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
                    Some(_) => return Err(CacheError::Integrity),
                    None if create_parents => {
                        fs::create_dir(&next).map_err(|_| CacheError::Storage)?;
                        sync_directory(&current).map_err(|_| CacheError::Storage)?;
                    }
                    None => return Ok(self.staging_root.join(relative)),
                }
                current = next;
            }
        }
        Ok(self.staging_root.join(relative))
    }

    #[cfg(test)]
    pub(crate) fn staging_root_for_test(&self) -> PathBuf {
        self.staging_root.clone()
    }

    #[cfg(test)]
    pub(crate) fn final_root_for_test(&self) -> PathBuf {
        self.final_root.clone()
    }
}

fn valid_source_binding(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

/// Reopens one promoted exact package through the existing all-files activation gate.
pub(crate) fn reopen_cached_package(
    cache_root: &Path,
    manifest: ModelPackageManifest,
) -> Result<Option<ModelLocation>, CacheError> {
    validate_manifest(&manifest)?;
    let cache_root = prepare_cache_root(cache_root)?;
    let packages = ensure_managed_directory(&cache_root, PACKAGES_DIRECTORY)?;
    let final_root = packages.join(identity_digest(&manifest_bytes(&manifest)?));
    if symlink_metadata_if_exists(&final_root)?.is_none() {
        return Ok(None);
    }
    reopen_exact_package(&final_root, &manifest).map(Some)
}

/// Re-verifies every promoted byte immediately before sending one private worker load frame.
pub(crate) async fn begin_cached_model_load(
    transport: &mut WorkerTransport,
    request_id: impl Into<String>,
    cache_root: &Path,
    manifest: ModelPackageManifest,
) -> Result<(), CachedModelLoadError> {
    let model = reopen_cached_package(cache_root, manifest)
        .map_err(CachedModelLoadError::Cache)?
        .ok_or(CachedModelLoadError::Cache(CacheError::InvalidState))?;
    transport
        .begin_load(request_id, model)
        .await
        .map_err(CachedModelLoadError::Transport)
}

fn validate_manifest(manifest: &ModelPackageManifest) -> Result<(), CacheError> {
    ModelAcquisition::new(manifest.clone())
        .map(|_| ())
        .map_err(map_acquisition_error)
}

fn manifest_bytes(manifest: &ModelPackageManifest) -> Result<Vec<u8>, CacheError> {
    let bytes = serde_json::to_vec(manifest).map_err(|_| CacheError::InvalidManifest)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(CacheError::InvalidManifest);
    }
    Ok(bytes)
}

fn identity_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn prepare_cache_root(cache_root: &Path) -> Result<PathBuf, CacheError> {
    if !cache_root.exists() {
        fs::create_dir_all(cache_root).map_err(|_| CacheError::Storage)?;
    }
    let metadata = fs::symlink_metadata(cache_root).map_err(|_| CacheError::Storage)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CacheError::Integrity);
    }
    cache_root.canonicalize().map_err(|_| CacheError::Storage)
}

fn ensure_managed_directory(root: &Path, name: &str) -> Result<PathBuf, CacheError> {
    let path = root.join(name);
    match symlink_metadata_if_exists(&path)? {
        Some(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Some(_) => return Err(CacheError::Integrity),
        None => {
            fs::create_dir(&path).map_err(|_| CacheError::Storage)?;
            sync_directory(root).map_err(|_| CacheError::Storage)?;
        }
    }
    Ok(path)
}

fn write_manifest(root: &Path, bytes: &[u8]) -> Result<(), CacheError> {
    let destination = root.join(MANIFEST_FILE);
    let temporary = root.join(format!(
        "{MANIFEST_TEMP_PREFIX}-{}.tmp",
        uuid::Uuid::new_v4()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, &destination)?;
        sync_directory(root)
    })();
    let _ = fs::remove_file(temporary);
    result.map_err(|_| CacheError::Storage)
}

fn read_manifest(root: &Path) -> Result<ModelPackageManifest, CacheError> {
    let path = root.join(MANIFEST_FILE);
    let metadata = fs::symlink_metadata(&path).map_err(|_| CacheError::Integrity)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_MANIFEST_BYTES
    {
        return Err(CacheError::Integrity);
    }
    let bytes = fs::read(path).map_err(|_| CacheError::Integrity)?;
    serde_json::from_slice(&bytes).map_err(|_| CacheError::Integrity)
}

fn write_source_binding(root: &Path, source_binding: &str) -> Result<(), CacheError> {
    let destination = root.join(SOURCE_BINDING_FILE);
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&destination)
        .map_err(|_| CacheError::Storage)?;
    file.write_all(source_binding.as_bytes())
        .map_err(|_| CacheError::Storage)?;
    file.sync_all().map_err(|_| CacheError::Storage)?;
    drop(file);
    sync_directory(root).map_err(|_| CacheError::Storage)
}

fn read_source_binding(root: &Path) -> Result<String, CacheError> {
    let path = root.join(SOURCE_BINDING_FILE);
    let metadata = fs::symlink_metadata(&path).map_err(|_| CacheError::Integrity)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() != 64 {
        return Err(CacheError::Integrity);
    }
    let value = fs::read_to_string(path).map_err(|_| CacheError::Integrity)?;
    if !valid_source_binding(&value) {
        return Err(CacheError::Integrity);
    }
    Ok(value)
}

fn activate_package(
    root: &Path,
    manifest: &ModelPackageManifest,
) -> Result<ModelLocation, CacheError> {
    let mut acquisition = ModelAcquisition::new(manifest.clone()).map_err(map_acquisition_error)?;
    acquisition
        .begin_download()
        .map_err(map_acquisition_error)?;
    acquisition
        .record_download_progress(manifest.files.len() as u32, manifest.expected_disk_bytes)
        .map_err(map_acquisition_error)?;
    acquisition
        .begin_verification()
        .map_err(map_acquisition_error)?;
    acquisition.activate(root).map_err(map_acquisition_error)
}

fn reopen_exact_package(
    root: &Path,
    manifest: &ModelPackageManifest,
) -> Result<ModelLocation, CacheError> {
    let metadata = fs::symlink_metadata(root).map_err(|_| CacheError::Integrity)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CacheError::Integrity);
    }
    if read_manifest(root)? != *manifest {
        return Err(CacheError::Integrity);
    }
    validate_cache_tree(root, manifest)?;
    activate_package(root, manifest)
}

fn map_acquisition_error(error: AcquisitionError) -> CacheError {
    match error {
        AcquisitionError::InvalidManifest => CacheError::InvalidManifest,
        AcquisitionError::InvalidState | AcquisitionError::InvalidProgress => {
            CacheError::InvalidState
        }
        AcquisitionError::Integrity => CacheError::Integrity,
    }
}
