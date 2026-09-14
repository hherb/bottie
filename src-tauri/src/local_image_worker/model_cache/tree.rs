//! Link-aware exact tree validation using native path components on every platform.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use super::{
    CacheError, MANIFEST_FILE, ModelPackageManifest, symlink_metadata_if_exists,
    verify_contract_file,
};

/// Rejects unknown entries, unsafe types, escaped links, and invalid complete files.
pub(super) fn validate_cache_tree(
    root: &Path,
    manifest: &ModelPackageManifest,
) -> Result<(), CacheError> {
    let files: HashSet<PathBuf> = manifest
        .files
        .iter()
        .map(|contract| PathBuf::from(&contract.relative_path))
        .collect();
    let mut directories = HashSet::new();
    for file in &manifest.files {
        let mut parent = Path::new(&file.relative_path).parent();
        while let Some(path) = parent.filter(|path| !path.as_os_str().is_empty()) {
            directories.insert(path.to_path_buf());
            parent = path.parent();
        }
    }
    validate_tree_entries(root, root, &files, &directories, manifest)
}

fn validate_tree_entries(
    root: &Path,
    current: &Path,
    files: &HashSet<PathBuf>,
    directories: &HashSet<PathBuf>,
    manifest: &ModelPackageManifest,
) -> Result<(), CacheError> {
    for entry in fs::read_dir(current).map_err(|_| CacheError::Integrity)? {
        let entry = entry.map_err(|_| CacheError::Integrity)?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|_| CacheError::Integrity)?;
        if metadata.file_type().is_symlink() {
            return Err(CacheError::Integrity);
        }
        let relative = path.strip_prefix(root).map_err(|_| CacheError::Integrity)?;
        if metadata.is_dir() && directories.contains(relative) {
            validate_tree_entries(root, &path, files, directories, manifest)?;
        } else if !metadata.is_file()
            || (relative != Path::new(MANIFEST_FILE) && !files.contains(relative))
        {
            return Err(CacheError::Integrity);
        }
    }
    for contract in &manifest.files {
        let path = root.join(&contract.relative_path);
        if let Some(metadata) = symlink_metadata_if_exists(&path)? {
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.len() > contract.byte_size
            {
                return Err(CacheError::Integrity);
            }
            if metadata.len() == contract.byte_size {
                verify_contract_file(&path, contract)?;
            }
        }
    }
    Ok(())
}
