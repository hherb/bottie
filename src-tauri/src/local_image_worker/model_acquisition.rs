//! Explicit manifest, progress, integrity, and activation policy for local image models.

use std::{
    collections::HashSet,
    fs::File,
    io::Read,
    path::{Component, Path},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::protocol::ModelLocation;

/// Maximum files accepted in one exact local model package.
const MAX_PACKAGE_FILES: usize = 1_024;
/// Maximum UTF-8 bytes accepted for one manifest identity.
const MAX_IDENTITY_BYTES: usize = 256;
/// Maximum UTF-8 bytes accepted for one relative package path.
const MAX_RELATIVE_PATH_BYTES: usize = 4_096;
/// Maximum expected package size accepted by this generic acquisition boundary.
const MAX_PACKAGE_BYTES: u64 = 64 * 1_024 * 1_024 * 1_024;
/// Maximum declared peak memory accepted before runtime-specific probing.
const MAX_EXPECTED_MEMORY_BYTES: u64 = 128 * 1_024 * 1_024 * 1_024;
/// Fixed bytes read while hashing without retaining model contents.
const HASH_BUFFER_BYTES: usize = 64 * 1_024;
/// Lowercase SHA-256 text length.
const SHA256_HEX_BYTES: usize = 64;
/// Hosted-only model identity that must never be accepted as a local package.
const HOSTED_QWEN_IMAGE_2_MODEL_ID: &str = "qwen-image-2.0-2026-03-03";

/// Stable native acquisition failures without paths, hashes, or file contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AcquisitionError {
    /// The exact package description is malformed or internally inconsistent.
    InvalidManifest,
    /// The requested lifecycle transition is not currently permitted.
    InvalidState,
    /// Download counters regressed or exceeded the exact manifest totals.
    InvalidProgress,
    /// A package file was missing, escaped the root, or failed size/hash verification.
    Integrity,
}

/// One native-only file identity required by an immutable model package.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ModelFileContract {
    /// Portable relative path beneath the app-owned package directory.
    pub(crate) relative_path: String,
    /// Exact expected byte length.
    pub(crate) byte_size: u64,
    /// Exact lowercase SHA-256 digest retained only by Rust.
    pub(crate) sha256: String,
}

/// Immutable local model/runtime package metadata reviewed before download.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ModelPackageManifest {
    /// Exact open-weight model identity, never an alias for hosted Qwen Image 2.0.
    pub(crate) model_id: String,
    /// Exact runtime identity with a pinned source or package revision.
    pub(crate) runtime_id: String,
    /// SPDX license identifier shown before acquisition.
    pub(crate) license: String,
    /// Immutable lowercase source revision for the model repository.
    pub(crate) source_revision: String,
    /// Exact installed bytes represented by every file contract.
    pub(crate) expected_disk_bytes: u64,
    /// Reviewed peak-memory expectation, pending later hardware-specific proof.
    pub(crate) expected_memory_bytes: u64,
    /// Closed list of files that must verify before activation.
    pub(crate) files: Vec<ModelFileContract>,
}

/// Path-free acquisition phase available to a future native command adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AcquisitionPhase {
    /// Exact identity, license, size, and revision await explicit user approval.
    AwaitingApproval,
    /// The separately implemented downloader is filling the app-owned cache.
    Downloading,
    /// All expected bytes arrived and native verification is in progress.
    Verifying,
    /// Every exact file verified and the package may be passed to the worker.
    Ready,
    /// Download or integrity work failed without exposing native detail.
    Failed,
}

/// Path-free bounded acquisition metadata for future Svelte presentation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelAcquisitionStatus {
    /// Exact open-weight model identity.
    pub(crate) model_id: String,
    /// Exact pinned runtime identity.
    pub(crate) runtime_id: String,
    /// Reviewed SPDX license identifier.
    pub(crate) license: String,
    /// Immutable model source revision.
    pub(crate) source_revision: String,
    /// Exact expected installed bytes.
    pub(crate) expected_disk_bytes: u64,
    /// Reviewed expected peak memory bytes.
    pub(crate) expected_memory_bytes: u64,
    /// Current explicit lifecycle phase.
    pub(crate) phase: AcquisitionPhase,
    /// Files durably downloaded under the manifest.
    pub(crate) downloaded_files: u32,
    /// Total files declared by the manifest.
    pub(crate) total_files: u32,
    /// Bytes durably downloaded under the manifest.
    pub(crate) downloaded_bytes: u64,
    /// Files whose exact size and hash passed native verification.
    pub(crate) verified_files: u32,
}

/// Native-only acquisition state that gates worker activation on exact file verification.
#[derive(Debug)]
pub(crate) struct ModelAcquisition {
    manifest: ModelPackageManifest,
    phase: AcquisitionPhase,
    downloaded_files: u32,
    downloaded_bytes: u64,
    verified_files: u32,
}

impl ModelAcquisition {
    /// Validates an immutable manifest and begins at the explicit approval gate.
    pub(crate) fn new(manifest: ModelPackageManifest) -> Result<Self, AcquisitionError> {
        validate_manifest(&manifest)?;
        Ok(Self {
            manifest,
            phase: AcquisitionPhase::AwaitingApproval,
            downloaded_files: 0,
            downloaded_bytes: 0,
            verified_files: 0,
        })
    }

    /// Returns exact path-free display metadata without native file contracts or hashes.
    pub(crate) fn status(&self) -> ModelAcquisitionStatus {
        ModelAcquisitionStatus {
            model_id: self.manifest.model_id.clone(),
            runtime_id: self.manifest.runtime_id.clone(),
            license: self.manifest.license.clone(),
            source_revision: self.manifest.source_revision.clone(),
            expected_disk_bytes: self.manifest.expected_disk_bytes,
            expected_memory_bytes: self.manifest.expected_memory_bytes,
            phase: self.phase,
            downloaded_files: self.downloaded_files,
            total_files: self.manifest.files.len() as u32,
            downloaded_bytes: self.downloaded_bytes,
            verified_files: self.verified_files,
        }
    }

    /// Returns the exact native manifest for trusted acquisition orchestration only.
    pub(crate) fn manifest(&self) -> &ModelPackageManifest {
        &self.manifest
    }

    /// Records explicit approval and permits the separate downloader to start.
    pub(crate) fn begin_download(&mut self) -> Result<(), AcquisitionError> {
        if self.phase != AcquisitionPhase::AwaitingApproval {
            return Err(AcquisitionError::InvalidState);
        }
        self.phase = AcquisitionPhase::Downloading;
        Ok(())
    }

    /// Applies monotonic durable progress bounded by exact manifest totals.
    pub(crate) fn record_download_progress(
        &mut self,
        completed_files: u32,
        downloaded_bytes: u64,
    ) -> Result<(), AcquisitionError> {
        let total_files = self.manifest.files.len() as u32;
        if self.phase != AcquisitionPhase::Downloading {
            return Err(AcquisitionError::InvalidState);
        }
        if completed_files < self.downloaded_files
            || downloaded_bytes < self.downloaded_bytes
            || completed_files > total_files
            || downloaded_bytes > self.manifest.expected_disk_bytes
            || (completed_files == total_files
                && downloaded_bytes != self.manifest.expected_disk_bytes)
        {
            return Err(AcquisitionError::InvalidProgress);
        }
        self.downloaded_files = completed_files;
        self.downloaded_bytes = downloaded_bytes;
        Ok(())
    }

    /// Enters verification only after every exact expected byte was downloaded.
    pub(crate) fn begin_verification(&mut self) -> Result<(), AcquisitionError> {
        if self.phase != AcquisitionPhase::Downloading
            || self.downloaded_files != self.manifest.files.len() as u32
            || self.downloaded_bytes != self.manifest.expected_disk_bytes
        {
            return Err(AcquisitionError::InvalidState);
        }
        self.phase = AcquisitionPhase::Verifying;
        Ok(())
    }

    /// Verifies every manifest file and returns a worker location only on complete success.
    pub(crate) fn activate(
        &mut self,
        package_root: &Path,
    ) -> Result<ModelLocation, AcquisitionError> {
        if self.phase != AcquisitionPhase::Verifying {
            return Err(AcquisitionError::InvalidState);
        }
        let result = verify_package(package_root, &self.manifest);
        match result {
            Ok(model_directory) => {
                self.verified_files = self.manifest.files.len() as u32;
                self.phase = AcquisitionPhase::Ready;
                Ok(ModelLocation {
                    model_id: self.manifest.model_id.clone(),
                    model_revision: self.manifest.source_revision.clone(),
                    model_directory,
                })
            }
            Err(error) => {
                self.verified_files = 0;
                self.phase = AcquisitionPhase::Failed;
                Err(error)
            }
        }
    }
}

fn validate_manifest(manifest: &ModelPackageManifest) -> Result<(), AcquisitionError> {
    if !valid_identity(&manifest.model_id)
        || manifest.model_id == HOSTED_QWEN_IMAGE_2_MODEL_ID
        || !valid_pinned_runtime(&manifest.runtime_id)
        || !valid_license(&manifest.license)
        || !valid_revision(&manifest.source_revision)
        || manifest.files.is_empty()
        || manifest.files.len() > MAX_PACKAGE_FILES
        || manifest.expected_disk_bytes == 0
        || manifest.expected_disk_bytes > MAX_PACKAGE_BYTES
        || manifest.expected_memory_bytes == 0
        || manifest.expected_memory_bytes > MAX_EXPECTED_MEMORY_BYTES
    {
        return Err(AcquisitionError::InvalidManifest);
    }
    let mut paths = HashSet::new();
    let mut total = 0_u64;
    for file in &manifest.files {
        if !valid_relative_path(&file.relative_path)
            || !paths.insert(file.relative_path.to_ascii_lowercase())
            || file.byte_size == 0
            || file.byte_size > MAX_PACKAGE_BYTES
            || !is_lowercase_hex(&file.sha256, SHA256_HEX_BYTES)
        {
            return Err(AcquisitionError::InvalidManifest);
        }
        total = total
            .checked_add(file.byte_size)
            .ok_or(AcquisitionError::InvalidManifest)?;
    }
    if total != manifest.expected_disk_bytes {
        return Err(AcquisitionError::InvalidManifest);
    }
    Ok(())
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTITY_BYTES
        && !value.chars().any(char::is_control)
        && value.trim() == value
}

fn valid_pinned_runtime(value: &str) -> bool {
    let Some((identity, revision)) = value.rsplit_once('@') else {
        return false;
    };
    valid_identity(identity)
        && (16..=SHA256_HEX_BYTES).contains(&revision.len())
        && revision
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_license(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTITY_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b'-'))
}

fn valid_revision(value: &str) -> bool {
    matches!(value.len(), 40 | SHA256_HEX_BYTES) && is_lowercase_hex(value, value.len())
}

fn is_lowercase_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_RELATIVE_PATH_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn verify_package(
    package_root: &Path,
    manifest: &ModelPackageManifest,
) -> Result<String, AcquisitionError> {
    let canonical_root = package_root
        .canonicalize()
        .map_err(|_| AcquisitionError::Integrity)?;
    if !canonical_root.is_dir() {
        return Err(AcquisitionError::Integrity);
    }
    for contract in &manifest.files {
        let path = package_root.join(&contract.relative_path);
        let canonical_path = path
            .canonicalize()
            .map_err(|_| AcquisitionError::Integrity)?;
        if !canonical_path.starts_with(&canonical_root) {
            return Err(AcquisitionError::Integrity);
        }
        verify_file(&canonical_path, contract)?;
    }
    canonical_root
        .to_str()
        .map(str::to_owned)
        .ok_or(AcquisitionError::Integrity)
}

fn verify_file(path: &Path, contract: &ModelFileContract) -> Result<(), AcquisitionError> {
    let file = File::open(path).map_err(|_| AcquisitionError::Integrity)?;
    let metadata = file.metadata().map_err(|_| AcquisitionError::Integrity)?;
    if !metadata.is_file() || metadata.len() != contract.byte_size {
        return Err(AcquisitionError::Integrity);
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    let mut reader = file.take(contract.byte_size.saturating_add(1));
    let mut total = 0_u64;
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|_| AcquisitionError::Integrity)?;
        if count == 0 {
            break;
        }
        total = total.saturating_add(count as u64);
        hasher.update(&buffer[..count]);
    }
    if total == contract.byte_size && format!("{:x}", hasher.finalize()) == contract.sha256 {
        Ok(())
    } else {
        Err(AcquisitionError::Integrity)
    }
}
