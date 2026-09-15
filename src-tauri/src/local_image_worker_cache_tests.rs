//! Tests for explicit transactional import into Bottie's private worker cache.

use std::{fs, path::Path};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::local_image_worker::{
    availability::{WorkerInstallationReadiness, inspect_worker_installation},
    model_package::PackageAcceptanceEvidence,
    worker_bundle::hash_worker_bundle,
    worker_cache::{
        WorkerCacheError, WorkerImportApproval, import_worker_bundle, resolve_promoted_worker,
    },
};

const EXECUTABLE: &str = "bottie-local-image-mlx-worker";

#[test]
fn import_requires_an_exact_approval_before_creating_cache_state() {
    let root = temporary_directory("worker-cache-approval");
    let source = worker_fixture(&root.join("source"));
    let cache = root.join("cache");
    let evidence = evidence_for(&source);

    let wrong = WorkerImportApproval {
        runtime_id: "mlx-gen@wrong".into(),
        expected_disk_bytes: evidence.worker_bundle_byte_size,
        approved: true,
    };
    assert_eq!(
        import_worker_bundle(&cache, &source, EXECUTABLE, runtime_id(), &evidence, &wrong),
        Err(WorkerCacheError::ApprovalRequired)
    );
    assert!(!cache.exists());

    let declined = WorkerImportApproval {
        runtime_id: runtime_id().into(),
        expected_disk_bytes: evidence.worker_bundle_byte_size,
        approved: false,
    };
    assert_eq!(
        import_worker_bundle(
            &cache,
            &source,
            EXECUTABLE,
            runtime_id(),
            &evidence,
            &declined,
        ),
        Err(WorkerCacheError::ApprovalRequired)
    );
    assert!(!cache.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn exact_bundle_is_copied_reverified_and_atomically_promoted() {
    let root = temporary_directory("worker-cache-promote");
    let source = worker_fixture(&root.join("source"));
    let cache = root.join("cache");
    let evidence = evidence_for(&source);
    let approval = approval_for(&evidence);

    let promoted = import_worker_bundle(
        &cache,
        &source,
        EXECUTABLE,
        runtime_id(),
        &evidence,
        &approval,
    )
    .unwrap();
    assert!(
        promoted
            .bundle_root()
            .starts_with(cache.canonicalize().unwrap())
    );
    assert_eq!(
        inspect_worker_installation(promoted.bundle_root(), promoted.executable(), &evidence),
        WorkerInstallationReadiness::Verified
    );
    assert_eq!(
        resolve_promoted_worker(&cache, EXECUTABLE, runtime_id(), &evidence).unwrap(),
        Some(promoted)
    );
    assert!(!cache.join("staging").read_dir().unwrap().next().is_some());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_and_tampered_promotions_are_read_only_fail_closed_states() {
    let root = temporary_directory("worker-cache-inspect");
    let source = worker_fixture(&root.join("source"));
    let cache = root.join("cache");
    let evidence = evidence_for(&source);

    assert_eq!(
        resolve_promoted_worker(&cache, EXECUTABLE, runtime_id(), &evidence).unwrap(),
        None
    );
    assert!(!cache.exists());

    let promoted = import_worker_bundle(
        &cache,
        &source,
        EXECUTABLE,
        runtime_id(),
        &evidence,
        &approval_for(&evidence),
    )
    .unwrap();
    fs::write(promoted.bundle_root().join("runtime/library"), b"tampered").unwrap();
    assert_eq!(
        resolve_promoted_worker(&cache, EXECUTABLE, runtime_id(), &evidence),
        Err(WorkerCacheError::Integrity)
    );
    assert!(promoted.bundle_root().exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn exact_reimport_replaces_drift_only_after_approved_source_verification() {
    let root = temporary_directory("worker-cache-replace");
    let source = worker_fixture(&root.join("source"));
    let cache = root.join("cache");
    let evidence = evidence_for(&source);
    let approval = approval_for(&evidence);
    let first = import_worker_bundle(
        &cache,
        &source,
        EXECUTABLE,
        runtime_id(),
        &evidence,
        &approval,
    )
    .unwrap();
    fs::write(first.bundle_root().join("runtime/library"), b"tampered").unwrap();

    let wrong_source = worker_fixture(&root.join("wrong-source"));
    fs::write(wrong_source.join("runtime/library"), b"wrong").unwrap();
    assert_eq!(
        import_worker_bundle(
            &cache,
            &wrong_source,
            EXECUTABLE,
            runtime_id(),
            &evidence,
            &approval,
        ),
        Err(WorkerCacheError::Integrity)
    );
    assert_eq!(
        fs::read(first.bundle_root().join("runtime/library")).unwrap(),
        b"tampered"
    );

    let replaced = import_worker_bundle(
        &cache,
        &source,
        EXECUTABLE,
        runtime_id(),
        &evidence,
        &approval,
    )
    .unwrap();
    assert_eq!(replaced, first);
    assert_eq!(
        inspect_worker_installation(replaced.bundle_root(), replaced.executable(), &evidence),
        WorkerInstallationReadiness::Verified
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn source_symlinks_are_rejected_before_cache_mutation() {
    use std::os::unix::fs::symlink;

    let root = temporary_directory("worker-cache-symlink");
    let source = worker_fixture(&root.join("source"));
    let cache = root.join("cache");
    let evidence = evidence_for(&source);
    symlink(root.join("outside"), source.join("unsafe-link")).unwrap();

    assert_eq!(
        import_worker_bundle(
            &cache,
            &source,
            EXECUTABLE,
            runtime_id(),
            &evidence,
            &approval_for(&evidence),
        ),
        Err(WorkerCacheError::Integrity)
    );
    assert!(!cache.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_and_cache_must_be_disjoint_before_cache_mutation() {
    let root = temporary_directory("worker-cache-overlap");
    let source = worker_fixture(&root.join("source"));
    let evidence = evidence_for(&source);

    assert_eq!(
        import_worker_bundle(
            &source.join("app-cache"),
            &source,
            EXECUTABLE,
            runtime_id(),
            &evidence,
            &approval_for(&evidence),
        ),
        Err(WorkerCacheError::UnsafeLayout)
    );
    assert!(!source.join("app-cache").exists());
    fs::remove_dir_all(root).unwrap();
}

fn worker_fixture(root: &Path) -> std::path::PathBuf {
    fs::create_dir_all(root.join("runtime")).unwrap();
    let executable = root.join(EXECUTABLE);
    fs::write(&executable, b"worker").unwrap();
    #[cfg(unix)]
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(root.join("runtime/library"), b"runtime").unwrap();
    root.to_path_buf()
}

fn evidence_for(source: &Path) -> PackageAcceptanceEvidence {
    let measured = hash_worker_bundle(source, &source.join(EXECUTABLE)).unwrap();
    PackageAcceptanceEvidence {
        package_revision: "0123456789abcdef0123456789abcdef01234567".into(),
        runtime_revision: "0123456789abcdef0123456789abcdef01234567".into(),
        hardware_profile: "test-hardware".into(),
        generation_profile: "test-generation".into(),
        generated_rgb_sha256: "1".repeat(64),
        worker_sha256: measured.executable_sha256,
        worker_byte_size: measured.executable_byte_size,
        worker_bundle_sha256: measured.bundle_sha256,
        worker_bundle_byte_size: measured.bundle_byte_size,
        peak_memory_bytes: 1,
        cancellation_latency_ms: 1,
        network_sandbox_proved: true,
        visual_reviewed: true,
    }
}

fn approval_for(evidence: &PackageAcceptanceEvidence) -> WorkerImportApproval {
    WorkerImportApproval {
        runtime_id: runtime_id().into(),
        expected_disk_bytes: evidence.worker_bundle_byte_size,
        approved: true,
    }
}

fn runtime_id() -> &'static str {
    "mlx-gen@0123456789abcdef0123456789abcdef01234567"
}

fn temporary_directory(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("bottie-{label}-{}", uuid::Uuid::new_v4()))
}
