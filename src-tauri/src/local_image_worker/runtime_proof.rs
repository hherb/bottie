//! Native hashing and macOS process measurement for the explicit local-image runtime proof.

use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

const HASH_BUFFER_BYTES: usize = 1024 * 1024;
const BUNDLE_HASH_DOMAIN: &[u8] = b"bottie-local-image-worker-bundle-v1";

/// Stable proof utility failures without filesystem paths or file contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeProofError {
    /// A proof input was absent, outside its declared bundle, or not a regular portable file tree.
    InvalidBundle,
    /// A proof file could not be opened, read, or measured completely.
    Storage,
    /// The operating system did not provide a usable whole-process measurement.
    Measurement,
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
) -> Result<WorkerBundleEvidence, RuntimeProofError> {
    let root = canonical_directory(bundle_root)?;
    let executable = executable
        .canonicalize()
        .map_err(|_| RuntimeProofError::InvalidBundle)?;
    if !executable.starts_with(&root) || !executable.is_file() {
        return Err(RuntimeProofError::InvalidBundle);
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
            .ok_or(RuntimeProofError::InvalidBundle)?;
    }
    let (executable_sha256, executable_byte_size) = hash_file(&executable)?;
    Ok(WorkerBundleEvidence {
        executable_sha256,
        executable_byte_size,
        bundle_sha256: format!("{:x}", bundle_hasher.finalize()),
        bundle_byte_size,
    })
}

/// Returns the lifetime maximum physical footprint for one live macOS process.
#[cfg(target_os = "macos")]
pub(crate) fn lifetime_peak_memory(process_id: u32) -> Result<u64, RuntimeProofError> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage_info_v4>::uninit();
    // SAFETY: macOS writes one rusage_info_v4 into the valid out pointer for the live PID.
    let result = unsafe {
        libc::proc_pid_rusage(
            process_id as libc::c_int,
            libc::RUSAGE_INFO_V4,
            usage.as_mut_ptr().cast(),
        )
    };
    if result != 0 {
        return Err(RuntimeProofError::Measurement);
    }
    // SAFETY: a zero result from proc_pid_rusage initialized the complete requested structure.
    let usage = unsafe { usage.assume_init() };
    if usage.ri_lifetime_max_phys_footprint == 0 {
        Err(RuntimeProofError::Measurement)
    } else {
        Ok(usage.ri_lifetime_max_phys_footprint)
    }
}

/// Rejects runtime measurement on targets outside the approved Apple-silicon proof host.
#[cfg(not(target_os = "macos"))]
pub(crate) fn lifetime_peak_memory(_process_id: u32) -> Result<u64, RuntimeProofError> {
    Err(RuntimeProofError::Measurement)
}

fn canonical_directory(path: &Path) -> Result<PathBuf, RuntimeProofError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| RuntimeProofError::InvalidBundle)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(RuntimeProofError::InvalidBundle);
    }
    path.canonicalize()
        .map_err(|_| RuntimeProofError::InvalidBundle)
}

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), RuntimeProofError> {
    for entry in fs::read_dir(directory).map_err(|_| RuntimeProofError::Storage)? {
        let entry = entry.map_err(|_| RuntimeProofError::Storage)?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|_| RuntimeProofError::Storage)?;
        if metadata.file_type().is_symlink() {
            return Err(RuntimeProofError::InvalidBundle);
        }
        if metadata.is_dir() {
            collect_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .ok()
                .and_then(Path::to_str)
                .filter(|value| !value.is_empty())
                .ok_or(RuntimeProofError::InvalidBundle)?;
            files.push((relative.replace(std::path::MAIN_SEPARATOR, "/"), path));
        } else {
            return Err(RuntimeProofError::InvalidBundle);
        }
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<(String, u64), RuntimeProofError> {
    let mut file = File::open(path).map_err(|_| RuntimeProofError::Storage)?;
    let metadata = file.metadata().map_err(|_| RuntimeProofError::Storage)?;
    if !metadata.is_file() {
        return Err(RuntimeProofError::InvalidBundle);
    }
    let mut hasher = Sha256::new();
    let mut read_bytes = 0_u64;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| RuntimeProofError::Storage)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        read_bytes = read_bytes
            .checked_add(count as u64)
            .ok_or(RuntimeProofError::InvalidBundle)?;
    }
    if read_bytes != metadata.len() {
        return Err(RuntimeProofError::Storage);
    }
    Ok((format!("{:x}", hasher.finalize()), read_bytes))
}

fn hash_file_into(path: &Path, hasher: &mut Sha256) -> Result<u64, RuntimeProofError> {
    let mut file = File::open(path).map_err(|_| RuntimeProofError::Storage)?;
    let metadata = file.metadata().map_err(|_| RuntimeProofError::Storage)?;
    if !metadata.is_file() {
        return Err(RuntimeProofError::InvalidBundle);
    }
    hasher.update(metadata.len().to_be_bytes());
    let mut read_bytes = 0_u64;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| RuntimeProofError::Storage)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        read_bytes = read_bytes
            .checked_add(count as u64)
            .ok_or(RuntimeProofError::InvalidBundle)?;
    }
    if read_bytes != metadata.len() {
        return Err(RuntimeProofError::Storage);
    }
    Ok(read_bytes)
}

fn update_length_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_hash_is_sorted_and_changes_with_runtime_bytes() {
        let root =
            std::env::temp_dir().join(format!("bottie-worker-bundle-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("runtime")).unwrap();
        fs::write(root.join("worker"), b"binary").unwrap();
        fs::write(root.join("runtime/library"), b"first").unwrap();

        let first = hash_worker_bundle(&root, &root.join("worker")).unwrap();
        let repeated = hash_worker_bundle(&root, &root.join("worker")).unwrap();
        assert_eq!(first, repeated);
        fs::write(root.join("runtime/library"), b"second").unwrap();
        let changed = hash_worker_bundle(&root, &root.join("worker")).unwrap();
        assert_ne!(first.bundle_sha256, changed.bundle_sha256);
        fs::remove_dir_all(root).unwrap();
    }
}
