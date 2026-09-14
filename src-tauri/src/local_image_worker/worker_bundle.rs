//! Canonical native integrity measurement for an installed private worker bundle.

use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

const HASH_BUFFER_BYTES: usize = 1024 * 1024;
const BUNDLE_HASH_DOMAIN: &[u8] = b"bottie-local-image-worker-bundle-v1";

/// Stable bundle failures without filesystem paths or file contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkerBundleError {
    /// An input was absent, outside its declared bundle, or not a regular portable file tree.
    InvalidBundle,
    /// A bundle file could not be opened, read, or measured completely.
    Storage,
}

/// Exact native executable and canonical runtime-bundle measurements.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkerBundleEvidence {
    /// SHA-256 digest of the exact native worker executable.
    pub(crate) executable_sha256: String,
    /// Exact worker executable byte length.
    pub(crate) executable_byte_size: u64,
    /// SHA-256 digest of every sorted relative path, length, and regular-file byte sequence.
    pub(crate) bundle_sha256: String,
    /// Sum of all regular-file bytes represented by the bundle digest.
    pub(crate) bundle_byte_size: u64,
}

/// Hashes one closed, symlink-free worker bundle and its exact executable.
pub(crate) fn hash_worker_bundle(
    bundle_root: &Path,
    executable: &Path,
) -> Result<WorkerBundleEvidence, WorkerBundleError> {
    let root = canonical_directory(bundle_root)?;
    let executable = executable
        .canonicalize()
        .map_err(|_| WorkerBundleError::InvalidBundle)?;
    if !executable.starts_with(&root) || !executable.is_file() {
        return Err(WorkerBundleError::InvalidBundle);
    }
    let mut files = Vec::new();
    collect_files(&root, &root, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut bundle_hasher = Sha256::new();
    bundle_hasher.update(BUNDLE_HASH_DOMAIN);
    let mut bundle_byte_size = 0_u64;
    for (relative, path) in files {
        update_length_prefixed(&mut bundle_hasher, relative.as_bytes());
        let size = hash_file_into(&path, &mut bundle_hasher)?;
        bundle_byte_size = bundle_byte_size
            .checked_add(size)
            .ok_or(WorkerBundleError::InvalidBundle)?;
    }
    let (executable_sha256, executable_byte_size) = hash_file(&executable)?;
    Ok(WorkerBundleEvidence {
        executable_sha256,
        executable_byte_size,
        bundle_sha256: format!("{:x}", bundle_hasher.finalize()),
        bundle_byte_size,
    })
}

fn canonical_directory(path: &Path) -> Result<PathBuf, WorkerBundleError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| WorkerBundleError::InvalidBundle)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(WorkerBundleError::InvalidBundle);
    }
    path.canonicalize()
        .map_err(|_| WorkerBundleError::InvalidBundle)
}

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), WorkerBundleError> {
    for entry in fs::read_dir(directory).map_err(|_| WorkerBundleError::Storage)? {
        let entry = entry.map_err(|_| WorkerBundleError::Storage)?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|_| WorkerBundleError::Storage)?;
        if metadata.file_type().is_symlink() {
            return Err(WorkerBundleError::InvalidBundle);
        }
        if metadata.is_dir() {
            collect_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .ok()
                .and_then(Path::to_str)
                .filter(|value| !value.is_empty())
                .ok_or(WorkerBundleError::InvalidBundle)?;
            files.push((relative.replace(std::path::MAIN_SEPARATOR, "/"), path));
        } else {
            return Err(WorkerBundleError::InvalidBundle);
        }
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<(String, u64), WorkerBundleError> {
    let mut file = File::open(path).map_err(|_| WorkerBundleError::Storage)?;
    let metadata = file.metadata().map_err(|_| WorkerBundleError::Storage)?;
    if !metadata.is_file() {
        return Err(WorkerBundleError::InvalidBundle);
    }
    let mut hasher = Sha256::new();
    let mut read_bytes = 0_u64;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| WorkerBundleError::Storage)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        read_bytes = read_bytes
            .checked_add(count as u64)
            .ok_or(WorkerBundleError::InvalidBundle)?;
    }
    if read_bytes != metadata.len() {
        return Err(WorkerBundleError::Storage);
    }
    Ok((format!("{:x}", hasher.finalize()), read_bytes))
}

fn hash_file_into(path: &Path, hasher: &mut Sha256) -> Result<u64, WorkerBundleError> {
    let mut file = File::open(path).map_err(|_| WorkerBundleError::Storage)?;
    let metadata = file.metadata().map_err(|_| WorkerBundleError::Storage)?;
    if !metadata.is_file() {
        return Err(WorkerBundleError::InvalidBundle);
    }
    hasher.update(metadata.len().to_be_bytes());
    let mut read_bytes = 0_u64;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| WorkerBundleError::Storage)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        read_bytes = read_bytes
            .checked_add(count as u64)
            .ok_or(WorkerBundleError::InvalidBundle)?;
    }
    if read_bytes != metadata.len() {
        return Err(WorkerBundleError::Storage);
    }
    Ok(read_bytes)
}

fn update_length_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}
