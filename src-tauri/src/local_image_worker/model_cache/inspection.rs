//! Read-only and load-boundary inspection of promoted model packages.

use std::path::Path;

use super::super::{model_acquisition::ModelPackageManifest, protocol::ModelLocation};
use super::{
    CacheError, PACKAGES_DIRECTORY, ensure_managed_directory, identity_digest,
    package_identity_bytes, prepare_cache_root, reopen_exact_package, symlink_metadata_if_exists,
    validate_manifest,
};

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
