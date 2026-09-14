//! Tests for path-free local-image availability and native installation inspection.

use std::fs;

use sha2::{Digest, Sha256};

use crate::local_image_worker::{
    availability::{
        HardwareEvidenceProfile, HostArchitecture, HostOperatingSystem, LocalImageAvailability,
        LocalImageHardwareFacts, ModelCacheReadiness, WorkerInstallationReadiness,
        evaluate_local_image_availability, inspect_model_cache, inspect_worker_installation,
    },
    hardware::{evidence_profile_for, probe_local_image_hardware},
    model_acquisition::{ModelFileContract, ModelPackageManifest},
    model_cache::ModelCacheTransaction,
    model_package::{
        MLX_GEN_RUNTIME_REVISION, PackageAcceptanceEvidence, QWEN_IMAGE_2512_GENERATION_PROFILE,
        QWEN_IMAGE_2512_HARDWARE_PROFILE, QWEN_IMAGE_2512_PACKAGE_REVISION,
        qwen_image_2512_q4_candidate,
    },
    worker_bundle::hash_worker_bundle,
};

const GIB: u64 = 1_024 * 1_024 * 1_024;

#[test]
fn hardware_profile_requires_the_exact_accepted_chip_and_memory_tier() {
    assert_eq!(
        evidence_profile_for("Apple M3 Max", 128 * GIB),
        Some(HardwareEvidenceProfile::AppleM3Max128Gb)
    );
    assert_eq!(evidence_profile_for("Apple M3 Max", 64 * GIB), None);
    assert_eq!(evidence_profile_for("Apple M4 Max", 128 * GIB), None);
}

#[test]
fn native_probe_reports_the_closed_compile_target_identity() {
    let facts = probe_local_image_hardware().unwrap();

    #[cfg(target_os = "macos")]
    assert_eq!(facts.operating_system, HostOperatingSystem::MacOs);
    #[cfg(target_os = "linux")]
    assert_eq!(facts.operating_system, HostOperatingSystem::Linux);
    #[cfg(target_os = "windows")]
    assert_eq!(facts.operating_system, HostOperatingSystem::Windows);
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    assert_eq!(facts.operating_system, HostOperatingSystem::Other);

    #[cfg(target_arch = "aarch64")]
    assert_eq!(facts.architecture, HostArchitecture::Aarch64);
    #[cfg(target_arch = "x86_64")]
    assert_eq!(facts.architecture, HostArchitecture::X86_64);
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    assert_eq!(facts.architecture, HostArchitecture::Other);

    #[cfg(target_os = "macos")]
    assert!(facts.physical_memory_bytes > 0);
    #[cfg(not(target_os = "macos"))]
    assert_eq!(facts.physical_memory_bytes, 0);
}

#[test]
fn availability_fails_closed_at_each_path_free_gate() {
    let selected = selected_package("1".repeat(64), 1, "2".repeat(64), 2, 24 * GIB);
    let supported = hardware(
        HostOperatingSystem::MacOs,
        HostArchitecture::Aarch64,
        128 * GIB,
    );

    let cases = [
        (
            hardware(
                HostOperatingSystem::Linux,
                HostArchitecture::Aarch64,
                32 * GIB,
            ),
            WorkerInstallationReadiness::Verified,
            ModelCacheReadiness::Verified,
            LocalImageAvailability::UnsupportedPlatform,
        ),
        (
            hardware(
                HostOperatingSystem::MacOs,
                HostArchitecture::X86_64,
                32 * GIB,
            ),
            WorkerInstallationReadiness::Verified,
            ModelCacheReadiness::Verified,
            LocalImageAvailability::UnsupportedArchitecture,
        ),
        (
            hardware(
                HostOperatingSystem::MacOs,
                HostArchitecture::Aarch64,
                24 * GIB - 1,
            ),
            WorkerInstallationReadiness::Verified,
            ModelCacheReadiness::Verified,
            LocalImageAvailability::InsufficientMemory,
        ),
        (
            LocalImageHardwareFacts {
                evidence_profile: None,
                ..supported
            },
            WorkerInstallationReadiness::Verified,
            ModelCacheReadiness::Verified,
            LocalImageAvailability::UnsupportedHardware,
        ),
        (
            hardware(
                HostOperatingSystem::MacOs,
                HostArchitecture::Aarch64,
                32 * GIB,
            ),
            WorkerInstallationReadiness::Verified,
            ModelCacheReadiness::Verified,
            LocalImageAvailability::UnsupportedHardware,
        ),
        (
            supported,
            WorkerInstallationReadiness::Missing,
            ModelCacheReadiness::Verified,
            LocalImageAvailability::WorkerMissing,
        ),
        (
            supported,
            WorkerInstallationReadiness::Mismatch,
            ModelCacheReadiness::Verified,
            LocalImageAvailability::WorkerMismatch,
        ),
        (
            supported,
            WorkerInstallationReadiness::Verified,
            ModelCacheReadiness::Missing,
            LocalImageAvailability::ModelMissing,
        ),
        (
            supported,
            WorkerInstallationReadiness::Verified,
            ModelCacheReadiness::Mismatch,
            LocalImageAvailability::ModelMismatch,
        ),
        (
            supported,
            WorkerInstallationReadiness::Verified,
            ModelCacheReadiness::Verified,
            LocalImageAvailability::Ready,
        ),
    ];

    for (hardware, worker, model, expected) in cases {
        assert_eq!(
            evaluate_local_image_availability(&selected, hardware, worker, model),
            expected
        );
    }
}

#[test]
fn installed_worker_is_rehashed_against_the_selected_evidence() {
    let root = temporary_directory("availability-worker");
    fs::create_dir_all(root.join("runtime")).unwrap();
    let executable = root.join("bottie-local-image-mlx-worker");
    fs::write(&executable, b"worker").unwrap();
    fs::write(root.join("runtime/library"), b"runtime").unwrap();
    let measured = hash_worker_bundle(&root, &executable).unwrap();
    let selected = selected_package(
        measured.executable_sha256,
        measured.executable_byte_size,
        measured.bundle_sha256,
        measured.bundle_byte_size,
        24 * GIB,
    );

    assert_eq!(
        inspect_worker_installation(&root, &executable, selected.evidence()),
        WorkerInstallationReadiness::Verified
    );
    fs::write(root.join("runtime/library"), b"changed").unwrap();
    assert_eq!(
        inspect_worker_installation(&root, &executable, selected.evidence()),
        WorkerInstallationReadiness::Mismatch
    );
    assert_eq!(
        inspect_worker_installation(&root.join("absent"), &executable, selected.evidence()),
        WorkerInstallationReadiness::Missing
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn promoted_cache_is_reverified_without_exposing_its_location() {
    let root = temporary_directory("availability-cache");
    let bytes = b"verified model";
    let manifest = manifest_for(bytes);

    assert_eq!(
        inspect_model_cache(&root, &manifest),
        ModelCacheReadiness::Missing
    );
    assert!(!root.exists(), "readiness inspection must remain read-only");

    let transaction = ModelCacheTransaction::open(&root, manifest.clone()).unwrap();
    transaction
        .write_file("weights.bin", 0, &mut &bytes[..])
        .unwrap();
    transaction.promote().unwrap();
    assert_eq!(
        inspect_model_cache(&root, &manifest),
        ModelCacheReadiness::Verified
    );

    fs::write(
        transaction.final_root_for_test().join("weights.bin"),
        b"tampered",
    )
    .unwrap();
    assert_eq!(
        inspect_model_cache(&root, &manifest),
        ModelCacheReadiness::Mismatch
    );
    fs::remove_dir_all(root).unwrap();
}

fn selected_package(
    worker_sha256: String,
    worker_byte_size: u64,
    worker_bundle_sha256: String,
    worker_bundle_byte_size: u64,
    peak_memory_bytes: u64,
) -> crate::local_image_worker::model_package::SelectedModelPackage {
    qwen_image_2512_q4_candidate()
        .accept(PackageAcceptanceEvidence {
            package_revision: QWEN_IMAGE_2512_PACKAGE_REVISION.into(),
            runtime_revision: MLX_GEN_RUNTIME_REVISION.into(),
            hardware_profile: QWEN_IMAGE_2512_HARDWARE_PROFILE.into(),
            generation_profile: QWEN_IMAGE_2512_GENERATION_PROFILE.into(),
            generated_rgb_sha256: "3".repeat(64),
            worker_sha256,
            worker_byte_size,
            worker_bundle_sha256,
            worker_bundle_byte_size,
            peak_memory_bytes,
            cancellation_latency_ms: 250,
            network_sandbox_proved: true,
            visual_reviewed: true,
        })
        .unwrap()
}

fn hardware(
    operating_system: HostOperatingSystem,
    architecture: HostArchitecture,
    physical_memory_bytes: u64,
) -> LocalImageHardwareFacts {
    LocalImageHardwareFacts {
        operating_system,
        architecture,
        physical_memory_bytes,
        evidence_profile: Some(HardwareEvidenceProfile::AppleM3Max128Gb),
    }
}

fn manifest_for(bytes: &[u8]) -> ModelPackageManifest {
    ModelPackageManifest {
        model_id: "Qwen/Qwen-Image-2512".into(),
        package_id: "test/package".into(),
        runtime_id: "test-runtime@0123456789abcdef".into(),
        license: "Apache-2.0".into(),
        source_revision: "0123456789abcdef0123456789abcdef01234567".into(),
        expected_disk_bytes: bytes.len() as u64,
        expected_memory_bytes: 24 * GIB,
        files: vec![ModelFileContract {
            relative_path: "weights.bin".into(),
            byte_size: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        }],
    }
}

fn temporary_directory(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("bottie-{label}-{}", uuid::Uuid::new_v4()))
}
