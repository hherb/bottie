//! Read-only and load-boundary inspection of promoted model packages.

use std::path::Path;

use super::super::{model_acquisition::ModelPackageManifest, protocol::ModelLocation};
use super::{
    CacheError, PACKAGES_DIRECTORY, STAGING_DIRECTORY, ensure_managed_directory, identity_digest,
    package_identity_bytes, prepare_cache_root, read_manifest, read_source_binding,
    reopen_exact_package, symlink_metadata_if_exists, valid_source_binding, validate_cache_tree,
    validate_manifest,
};

/// Read-only durable progress recovered from one exact source-bound staging transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CacheResumeProgress {
    /// Manifest files whose exact size and digest are already complete.
    pub(crate) completed_files: u32,
    /// Exact bytes durably retained across all manifest files.
    pub(crate) downloaded_bytes: u64,
}

/// Inspects one exact resumable staging slot without creating, repairing, or deleting cache state.
pub(crate) fn inspect_bound_resume(
    cache_root: &Path,
    manifest: &ModelPackageManifest,
    source_binding: &str,
) -> Result<Option<CacheResumeProgress>, CacheError> {
    validate_manifest(manifest)?;
    if !valid_source_binding(source_binding) {
        return Err(CacheError::InvalidState);
    }
    let Some(cache_metadata) = symlink_metadata_if_exists(cache_root)? else {
        return Ok(None);
    };
    if cache_metadata.file_type().is_symlink() || !cache_metadata.is_dir() {
        return Err(CacheError::Integrity);
    }
    let cache_root = cache_root.canonicalize().map_err(|_| CacheError::Storage)?;
    let staging_parent = cache_root.join(STAGING_DIRECTORY);
    let Some(parent_metadata) = symlink_metadata_if_exists(&staging_parent)? else {
        return Ok(None);
    };
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Err(CacheError::Integrity);
    }
    let staging_root = staging_parent.join(identity_digest(manifest.model_id.as_bytes()));
    let Some(staging_metadata) = symlink_metadata_if_exists(&staging_root)? else {
        return Ok(None);
    };
    if staging_metadata.file_type().is_symlink() || !staging_metadata.is_dir() {
        return Ok(None);
    }
    if read_manifest(&staging_root).ok().as_ref() != Some(manifest)
        || read_source_binding(&staging_root).ok().as_deref() != Some(source_binding)
    {
        return Ok(None);
    }
    if validate_cache_tree(&staging_root, manifest).is_err() {
        return Ok(None);
    }
    let mut completed_files = 0_u32;
    let mut downloaded_bytes = 0_u64;
    for contract in &manifest.files {
        let path = staging_root.join(&contract.relative_path);
        let Some(metadata) = symlink_metadata_if_exists(&path)? else {
            continue;
        };
        downloaded_bytes = downloaded_bytes
            .checked_add(metadata.len())
            .ok_or(CacheError::Integrity)?;
        if metadata.len() == contract.byte_size {
            completed_files += 1;
        }
    }
    Ok(Some(CacheResumeProgress {
        completed_files,
        downloaded_bytes,
    }))
}

/// Reopens one promoted exact package through the existing all-files activation gate.
pub(crate) fn reopen_cached_package(
    cache_root: &Path,
    manifest: ModelPackageManifest,
) -> Result<Option<ModelLocation>, CacheError> {
    validate_manifest(&manifest)?;
    let cache_root = prepare_cache_root(cache_root)?;
    let packages = ensure_managed_directory(&cache_root, PACKAGES_DIRECTORY)?;
    let final_root = packages.join(identity_digest(&package_identity_bytes(&manifest)?));
    if symlink_metadata_if_exists(&final_root)?.is_none() {
        return Ok(None);
    }
    reopen_exact_package(&final_root, &manifest).map(Some)
}

/// Inspects one promoted exact package without creating or repairing any cache directories.
pub(crate) fn inspect_cached_package(
    cache_root: &Path,
    manifest: &ModelPackageManifest,
) -> Result<bool, CacheError> {
    validate_manifest(manifest)?;
    let Some(cache_metadata) = symlink_metadata_if_exists(cache_root)? else {
        return Ok(false);
    };
    if cache_metadata.file_type().is_symlink() || !cache_metadata.is_dir() {
        return Err(CacheError::Integrity);
    }
    let cache_root = cache_root.canonicalize().map_err(|_| CacheError::Storage)?;
    let packages = cache_root.join(PACKAGES_DIRECTORY);
    let Some(packages_metadata) = symlink_metadata_if_exists(&packages)? else {
        return Ok(false);
    };
    if packages_metadata.file_type().is_symlink() || !packages_metadata.is_dir() {
        return Err(CacheError::Integrity);
    }
    let final_root = packages.join(identity_digest(&package_identity_bytes(manifest)?));
    if symlink_metadata_if_exists(&final_root)?.is_none() {
        return Ok(false);
    }
    reopen_exact_package(&final_root, manifest).map(|_| true)
}
