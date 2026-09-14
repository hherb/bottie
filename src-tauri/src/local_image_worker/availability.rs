//! Path-free readiness policy for Bottie's selected local image package.

use std::{fs, io::ErrorKind, path::Path};

use serde::Serialize;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use super::{
    model_cache::inspect_cached_package,
    model_package::{
        PackageAcceptanceEvidence, QWEN_IMAGE_2512_HARDWARE_PROFILE, SelectedModelPackage,
    },
    worker_bundle::hash_worker_bundle,
};

const GIB: u64 = 1_024 * 1_024 * 1_024;

/// Operating-system identity observed by a native hardware probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostOperatingSystem {
    /// Apple's macOS desktop operating system.
    MacOs,
    /// A Linux desktop operating system.
    Linux,
    /// Microsoft's Windows desktop operating system.
    Windows,
    /// An operating system outside Bottie's closed support vocabulary.
    Other,
}

/// Native processor architecture observed independently from a model or marketing name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostArchitecture {
    /// 64-bit ARM, required by the selected MLX package.
    Aarch64,
    /// 64-bit x86, which cannot run the selected MLX package.
    X86_64,
    /// An architecture outside Bottie's closed support vocabulary.
    Other,
}

/// Closed hardware profile backed by one accepted runtime proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HardwareEvidenceProfile {
    /// Apple M3 Max with exactly 128 GiB unified memory.
    AppleM3Max128Gb,
}

impl HardwareEvidenceProfile {
    /// Returns the immutable identifier used by selected package evidence.
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::AppleM3Max128Gb => QWEN_IMAGE_2512_HARDWARE_PROFILE,
        }
    }

    /// Returns the exact physical-memory tier exercised by this profile's proof.
    pub(crate) const fn physical_memory_bytes(self) -> u64 {
        match self {
            Self::AppleM3Max128Gb => 128 * GIB,
        }
    }
}

/// Exact native facts consumed by the pure availability policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LocalImageHardwareFacts {
    /// Operating system reported by the native probe.
    pub(crate) operating_system: HostOperatingSystem,
    /// Processor architecture reported by the native probe.
    pub(crate) architecture: HostArchitecture,
    /// Total physical or unified memory reported by the operating system.
    pub(crate) physical_memory_bytes: u64,
    /// Accepted measured profile, or none when this exact hardware remains unproved.
    pub(crate) evidence_profile: Option<HardwareEvidenceProfile>,
}

/// Native re-verification status for the installed private worker bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkerInstallationReadiness {
    /// The worker bundle or its executable is absent.
    Missing,
    /// Installed bytes or filesystem shape differ from the selected evidence.
    Mismatch,
    /// Executable and complete bundle bytes match the selected evidence.
    Verified,
}

/// Native all-files verification status for the promoted model cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModelCacheReadiness {
    /// The selected immutable package has not been promoted.
    Missing,
    /// Promoted bytes, manifest, or filesystem shape no longer match.
    Mismatch,
    /// Every promoted file matches the selected immutable manifest.
    Verified,
}

/// Coarse path-free reason that the selected local image route is or is not ready.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LocalImageAvailability {
    /// The selected runtime has no accepted implementation for this operating system.
    UnsupportedPlatform,
    /// The selected runtime cannot execute on this processor architecture.
    UnsupportedArchitecture,
    /// Physical memory is below the package's measured generation requirement.
    InsufficientMemory,
    /// The exact host chip and memory tier has no accepted runtime proof.
    UnsupportedHardware,
    /// The private worker is not installed.
    WorkerMissing,
    /// The private worker installation does not match accepted evidence.
    WorkerMismatch,
    /// The selected model package is not installed.
    ModelMissing,
    /// The selected model package failed exact cache re-verification.
    ModelMismatch,
    /// Hardware, worker, and cached model satisfy every selected-package gate.
    Ready,
}

/// Applies the closed availability precedence without reading paths or mutating native state.
pub(crate) fn evaluate_local_image_availability(
    selected: &SelectedModelPackage,
    hardware: LocalImageHardwareFacts,
    worker: WorkerInstallationReadiness,
    model: ModelCacheReadiness,
) -> LocalImageAvailability {
    if hardware.operating_system != HostOperatingSystem::MacOs {
        return LocalImageAvailability::UnsupportedPlatform;
    }
    if hardware.architecture != HostArchitecture::Aarch64 {
        return LocalImageAvailability::UnsupportedArchitecture;
    }
    if hardware.physical_memory_bytes < selected.evidence().peak_memory_bytes {
        return LocalImageAvailability::InsufficientMemory;
    }
    if hardware.evidence_profile.is_none_or(|profile| {
        profile.id() != selected.evidence().hardware_profile
            || profile.physical_memory_bytes() != hardware.physical_memory_bytes
    }) {
        return LocalImageAvailability::UnsupportedHardware;
    }
    match worker {
        WorkerInstallationReadiness::Missing => return LocalImageAvailability::WorkerMissing,
        WorkerInstallationReadiness::Mismatch => return LocalImageAvailability::WorkerMismatch,
        WorkerInstallationReadiness::Verified => {}
    }
    match model {
        ModelCacheReadiness::Missing => LocalImageAvailability::ModelMissing,
        ModelCacheReadiness::Mismatch => LocalImageAvailability::ModelMismatch,
        ModelCacheReadiness::Verified => LocalImageAvailability::Ready,
    }
}

/// Re-hashes an installed worker executable and its closed bundle against accepted evidence.
pub(crate) fn inspect_worker_installation(
    bundle_root: &Path,
    executable: &Path,
    evidence: &PackageAcceptanceEvidence,
) -> WorkerInstallationReadiness {
    if path_is_missing(bundle_root) {
        return WorkerInstallationReadiness::Missing;
    }
    if !bundle_root_is_safe_directory(bundle_root) {
        return WorkerInstallationReadiness::Mismatch;
    }
    if path_is_missing(executable) {
        return WorkerInstallationReadiness::Missing;
    }
    if !executable_is_runnable(executable) {
        return WorkerInstallationReadiness::Mismatch;
    }
    let Ok(measured) = hash_worker_bundle(bundle_root, executable) else {
        return WorkerInstallationReadiness::Mismatch;
    };
    if measured.executable_sha256 == evidence.worker_sha256
        && measured.executable_byte_size == evidence.worker_byte_size
        && measured.bundle_sha256 == evidence.worker_bundle_sha256
        && measured.bundle_byte_size == evidence.worker_bundle_byte_size
    {
        WorkerInstallationReadiness::Verified
    } else {
        WorkerInstallationReadiness::Mismatch
    }
}

fn bundle_root_is_safe_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_dir())
}

/// Re-verifies a promoted exact model without creating cache state or returning its location.
pub(crate) fn inspect_model_cache(
    cache_root: &Path,
    manifest: &super::model_acquisition::ModelPackageManifest,
) -> ModelCacheReadiness {
    match inspect_cached_package(cache_root, manifest) {
        Ok(true) => ModelCacheReadiness::Verified,
        Ok(false) => ModelCacheReadiness::Missing,
        Err(_) => ModelCacheReadiness::Mismatch,
    }
}

fn path_is_missing(path: &Path) -> bool {
    matches!(
        fs::symlink_metadata(path),
        Err(error) if error.kind() == ErrorKind::NotFound
    )
}

#[cfg(unix)]
fn executable_is_runnable(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.file_type().is_file() {
        return false;
    }
    // SAFETY: these identity reads have no pointer arguments or side effects.
    let (effective_user, effective_group) = unsafe { (libc::geteuid(), libc::getegid()) };
    let mode = metadata.permissions().mode();
    if effective_user == 0 {
        return mode & 0o111 != 0;
    }
    let required_bit = if metadata.uid() == effective_user {
        0o100
    } else if process_belongs_to_group(metadata.gid(), effective_group) {
        0o010
    } else {
        0o001
    };
    mode & required_bit != 0
}

#[cfg(unix)]
fn process_belongs_to_group(group: libc::gid_t, effective_group: libc::gid_t) -> bool {
    if group == effective_group {
        return true;
    }
    // SAFETY: a zero-size query requires no output buffer.
    let count = unsafe { libc::getgroups(0, std::ptr::null_mut()) };
    if count <= 0 {
        return false;
    }
    let mut groups = vec![0; count as usize];
    // SAFETY: the vector is writable for exactly the count returned by the preceding query.
    let copied = unsafe { libc::getgroups(count, groups.as_mut_ptr()) };
    copied == count && groups.contains(&group)
}

#[cfg(not(unix))]
fn executable_is_runnable(_path: &Path) -> bool {
    true
}
