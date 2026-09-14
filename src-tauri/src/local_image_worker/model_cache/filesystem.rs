//! Small filesystem primitives for resumable cache writes and durable directory changes.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::Path,
};

use sha2::{Digest, Sha256};

use super::super::model_acquisition::ModelFileContract;
use super::{CacheError, STREAM_BUFFER_BYTES};

/// Opens a new or exact existing partial only in append mode.
pub(super) fn open_for_append(path: &Path, offset: u64) -> Result<File, CacheError> {
    if offset == 0 && !path.exists() {
        OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(path)
            .map_err(|_| CacheError::Storage)
    } else {
        OpenOptions::new()
            .append(true)
            .open(path)
            .map_err(|_| CacheError::Storage)
    }
}

/// Re-hashes an exact retained prefix before accepting resumed bytes.
pub(super) fn hash_prefix(path: &Path, length: u64) -> Result<Sha256, CacheError> {
    let mut hasher = Sha256::new();
    if length == 0 {
        return Ok(hasher);
    }
    let mut file = File::open(path).map_err(|_| CacheError::Integrity)?;
    let mut buffer = [0_u8; STREAM_BUFFER_BYTES];
    let mut total = 0_u64;
    loop {
        let count = file.read(&mut buffer).map_err(|_| CacheError::Integrity)?;
        if count == 0 {
            break;
        }
        total = total.saturating_add(count as u64);
        hasher.update(&buffer[..count]);
    }
    if total != length {
        return Err(CacheError::Integrity);
    }
    Ok(hasher)
}

/// Re-verifies one complete declared file without retaining its bytes.
pub(super) fn verify_contract_file(
    path: &Path,
    contract: &ModelFileContract,
) -> Result<(), CacheError> {
    let hasher = hash_prefix(path, contract.byte_size)?;
    if format!("{:x}", hasher.finalize()) == contract.sha256 {
        Ok(())
    } else {
        Err(CacheError::Integrity)
    }
}

/// Removes one failed declared file and durably records its absence.
pub(super) fn remove_failed_file(path: &Path) -> Result<(), CacheError> {
    if path.exists() {
        fs::remove_file(path).map_err(|_| CacheError::Storage)?;
        sync_parent(path)?;
    }
    Ok(())
}

/// Removes only one previously derived immediate child of a managed cache directory.
pub(super) fn remove_managed_entry(parent: &Path, path: &Path) -> Result<(), CacheError> {
    if path.parent() != Some(parent) {
        return Err(CacheError::Integrity);
    }
    let Some(metadata) = symlink_metadata_if_exists(path)? else {
        return Ok(());
    };
    if metadata.file_type().is_symlink() || metadata.is_file() {
        fs::remove_file(path).map_err(|_| CacheError::Storage)?;
    } else if metadata.is_dir() {
        fs::remove_dir_all(path).map_err(|_| CacheError::Storage)?;
    } else {
        return Err(CacheError::Integrity);
    }
    sync_directory(parent).map_err(|_| CacheError::Storage)
}

/// Reads link-aware metadata while treating only absence as an empty result.
pub(super) fn symlink_metadata_if_exists(path: &Path) -> Result<Option<fs::Metadata>, CacheError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(CacheError::Storage),
    }
}

/// Syncs the directory that owns one changed cache entry.
pub(super) fn sync_parent(path: &Path) -> Result<(), CacheError> {
    let parent = path.parent().ok_or(CacheError::Storage)?;
    sync_directory(parent).map_err(|_| CacheError::Storage)
}

/// Persists directory-entry changes on platforms exposing directory handles.
#[cfg(unix)]
pub(super) fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

/// Persists directory-entry changes using a Windows backup-semantics directory handle.
#[cfg(windows)]
pub(super) fn sync_directory(path: &Path) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    OpenOptions::new()
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?
        .sync_all()
}
