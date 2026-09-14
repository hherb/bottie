//! Tests for explicit local image-model acquisition and activation policy.

use std::{fs, path::PathBuf};

use sha2::{Digest, Sha256};

use crate::local_image_worker::model_acquisition::{
    AcquisitionError, AcquisitionPhase, ModelAcquisition, ModelFileContract, ModelPackageManifest,
};

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn manifest(files: Vec<ModelFileContract>) -> ModelPackageManifest {
    ModelPackageManifest {
        model_id: "Qwen/Qwen-Image-2512".into(),
        runtime_id: "mlx-gen@0123456789abcdef".into(),
        license: "Apache-2.0".into(),
        source_revision: "0123456789abcdef0123456789abcdef01234567".into(),
        expected_disk_bytes: files.iter().map(|file| file.byte_size).sum(),
        expected_memory_bytes: 20 * 1_024 * 1_024 * 1_024,
        files,
    }
}

fn file(relative_path: &str, bytes: &[u8]) -> ModelFileContract {
    ModelFileContract {
        relative_path: relative_path.into(),
        byte_size: bytes.len() as u64,
        sha256: digest(bytes),
    }
}

fn temp_package(name: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("bottie-local-model-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn exposes_path_free_exact_metadata_before_download() {
    let acquisition = ModelAcquisition::new(manifest(vec![file(
        "weights/model.safetensors",
        b"weights",
    )]))
    .unwrap();
    let status = acquisition.status();
    assert_eq!(status.phase, AcquisitionPhase::AwaitingApproval);
    assert_eq!(status.model_id, "Qwen/Qwen-Image-2512");
    assert_eq!(status.runtime_id, "mlx-gen@0123456789abcdef");
    assert_eq!(status.license, "Apache-2.0");
    assert_eq!(status.total_files, 1);
    let serialized = serde_json::to_string(&status).unwrap();
    assert!(!serialized.contains("sha256"));
    assert!(!serialized.contains("relativePath"));
    assert!(!serialized.contains("directory"));
}

#[test]
fn rejects_unsafe_or_internally_inconsistent_manifests() {
    for invalid in [
        manifest(vec![]),
        manifest(vec![file("../outside", b"weights")]),
        manifest(vec![file("/absolute", b"weights")]),
        manifest(vec![
            file("weights.bin", b"weights"),
            file("weights.bin", b"other"),
        ]),
        manifest(vec![
            file("Weights.bin", b"weights"),
            file("weights.bin", b"weights"),
        ]),
        manifest(vec![ModelFileContract {
            relative_path: "weights.bin".into(),
            byte_size: 7,
            sha256: "A".repeat(64),
        }]),
    ] {
        assert_eq!(
            ModelAcquisition::new(invalid).unwrap_err(),
            AcquisitionError::InvalidManifest
        );
    }
    let mut hosted_alias = manifest(vec![file("weights.bin", b"weights")]);
    hosted_alias.model_id = "qwen-image-2.0-2026-03-03".into();
    assert_eq!(
        ModelAcquisition::new(hosted_alias).unwrap_err(),
        AcquisitionError::InvalidManifest
    );
    let mut mismatched_total = manifest(vec![file("weights.bin", b"weights")]);
    mismatched_total.expected_disk_bytes += 1;
    assert_eq!(
        ModelAcquisition::new(mismatched_total).unwrap_err(),
        AcquisitionError::InvalidManifest
    );
}

#[test]
fn progress_is_monotonic_bounded_and_requires_explicit_phase_order() {
    let mut acquisition = ModelAcquisition::new(manifest(vec![
        file("model.json", b"model"),
        file("weights.bin", b"weights"),
    ]))
    .unwrap();
    assert_eq!(
        acquisition.record_download_progress(1, 5),
        Err(AcquisitionError::InvalidState)
    );
    acquisition.begin_download().unwrap();
    acquisition.record_download_progress(1, 5).unwrap();
    assert_eq!(
        acquisition.record_download_progress(0, 4),
        Err(AcquisitionError::InvalidProgress)
    );
    assert_eq!(
        acquisition.begin_verification(),
        Err(AcquisitionError::InvalidState)
    );
    acquisition.record_download_progress(2, 12).unwrap();
    acquisition.begin_verification().unwrap();
    assert_eq!(acquisition.status().phase, AcquisitionPhase::Verifying);
}

#[test]
fn activation_requires_every_exact_file_size_and_digest() {
    let root = temp_package("integrity");
    fs::create_dir_all(root.join("weights")).unwrap();
    fs::write(root.join("model.json"), b"model").unwrap();
    fs::write(root.join("weights/model.bin"), b"changed").unwrap();
    let package = manifest(vec![
        file("model.json", b"model"),
        file("weights/model.bin", b"weights"),
    ]);
    let mut acquisition = ModelAcquisition::new(package).unwrap();
    acquisition.begin_download().unwrap();
    acquisition.record_download_progress(2, 12).unwrap();
    acquisition.begin_verification().unwrap();
    assert_eq!(
        acquisition.activate(&root).unwrap_err(),
        AcquisitionError::Integrity
    );
    assert_eq!(acquisition.status().phase, AcquisitionPhase::Failed);
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn activation_rejects_a_manifest_file_symlinked_outside_the_package() {
    use std::os::unix::fs::symlink;

    let root = temp_package("symlink-root");
    let outside = temp_package("symlink-outside");
    fs::write(outside.join("weights.bin"), b"weights").unwrap();
    symlink(outside.join("weights.bin"), root.join("weights.bin")).unwrap();
    let mut acquisition =
        ModelAcquisition::new(manifest(vec![file("weights.bin", b"weights")])).unwrap();
    acquisition.begin_download().unwrap();
    acquisition.record_download_progress(1, 7).unwrap();
    acquisition.begin_verification().unwrap();
    assert_eq!(
        acquisition.activate(&root).unwrap_err(),
        AcquisitionError::Integrity
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[test]
fn verified_package_activates_only_after_all_hashes_match() {
    let root = temp_package("ready");
    fs::create_dir_all(root.join("weights")).unwrap();
    fs::write(root.join("model.json"), b"model").unwrap();
    fs::write(root.join("weights/model.bin"), b"weights").unwrap();
    let mut acquisition = ModelAcquisition::new(manifest(vec![
        file("model.json", b"model"),
        file("weights/model.bin", b"weights"),
    ]))
    .unwrap();
    acquisition.begin_download().unwrap();
    acquisition.record_download_progress(2, 12).unwrap();
    acquisition.begin_verification().unwrap();
    let location = acquisition.activate(&root).unwrap();
    assert_eq!(location.model_id, "Qwen/Qwen-Image-2512");
    assert_eq!(
        location.model_revision,
        "0123456789abcdef0123456789abcdef01234567"
    );
    assert_eq!(
        location.model_directory,
        root.canonicalize().unwrap().to_string_lossy()
    );
    let status = acquisition.status();
    assert_eq!(status.phase, AcquisitionPhase::Ready);
    assert_eq!(status.verified_files, 2);
    let _ = fs::remove_dir_all(root);
}
