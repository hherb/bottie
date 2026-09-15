//! Tests for app-owned local-image path resolution and path-free command metadata.

use std::{fs, path::Path};

use serde_json::json;

use crate::local_image_worker::availability_service::{
    LOCAL_IMAGE_MODEL_CACHE_DIRECTORY, LOCAL_IMAGE_WORKER_DIRECTORY, LOCAL_IMAGE_WORKER_EXECUTABLE,
    LocalImageAvailabilityService, LocalImageServiceError, metadata_for_availability,
};
use crate::local_image_worker::{
    availability::{
        LocalImageAvailability, WorkerInstallationReadiness, inspect_worker_installation,
    },
    model_package::selected_qwen_image_2512_q4_package,
};

#[test]
fn service_resolves_only_the_fixed_app_owned_layout() {
    let root = temporary_directory("availability-service-layout");
    let resources = root.join("resources");
    let app_data = root.join("app-data");
    let service = LocalImageAvailabilityService::new(&resources, &app_data).unwrap();

    assert_eq!(
        service.worker_bundle_root_for_test(),
        resources.join(LOCAL_IMAGE_WORKER_DIRECTORY)
    );
    assert_eq!(
        service.worker_executable_for_test(),
        resources
            .join(LOCAL_IMAGE_WORKER_DIRECTORY)
            .join(LOCAL_IMAGE_WORKER_EXECUTABLE)
    );
    assert_eq!(
        service.model_cache_root_for_test(),
        app_data.join(LOCAL_IMAGE_MODEL_CACHE_DIRECTORY)
    );
}

#[test]
fn service_rejects_relative_or_lexically_unsafe_app_roots() {
    let root = temporary_directory("availability-service-unsafe");

    assert_eq!(
        LocalImageAvailabilityService::new(Path::new("resources"), &root).unwrap_err(),
        LocalImageServiceError::UnsafeLayout
    );
    assert_eq!(
        LocalImageAvailabilityService::new(&root, Path::new("app-data")).unwrap_err(),
        LocalImageServiceError::UnsafeLayout
    );
    assert_eq!(
        LocalImageAvailabilityService::new(&root.join("safe/../resources"), &root).unwrap_err(),
        LocalImageServiceError::UnsafeLayout
    );
}

#[tokio::test]
async fn missing_installations_are_stable_across_repeated_requests() {
    let root = temporary_directory("availability-service-missing");
    let service =
        LocalImageAvailabilityService::new(root.join("resources"), root.join("data")).unwrap();

    let first = service.inspect().await.unwrap();
    let second = service.inspect().await.unwrap();

    assert_eq!(first, second);
    assert!(
        !root.exists(),
        "read-only requests must not create app-owned roots"
    );
}

#[test]
fn metadata_serialization_is_exact_and_contains_no_paths_or_hashes() {
    let root = temporary_directory("availability-service-metadata");
    let metadata = metadata_for_availability(
        &selected_qwen_image_2512_q4_package().unwrap(),
        LocalImageAvailability::WorkerMissing,
    );
    let serialized = serde_json::to_value(metadata).unwrap();

    assert_eq!(
        serialized,
        json!({
            "modelId": "Qwen/Qwen-Image-2512",
            "packageId": "AbstractFramework/qwen-image-2512-4bit",
            "runtimeId": "mlx-gen@fca64a283737c68b67a7bfd88d93f7aa9101a95c",
            "license": "Apache-2.0",
            "sourceRevision": "423f1f5bf708c6e11eb78881ef9738422cea0814",
            "expectedDiskBytes": 17_442_350_812_u64,
            "requiredMemoryBytes": 29_526_129_448_u64,
            "availability": "worker_missing",
        })
    );
    let text = serialized.to_string();
    assert!(!text.contains(root.to_string_lossy().as_ref()));
    assert!(!text.to_ascii_lowercase().contains("sha256"));
}

#[cfg(unix)]
#[test]
fn unsafe_worker_symlinks_fail_closed_without_leaking_the_target() {
    use std::os::unix::fs::symlink;

    let root = temporary_directory("availability-service-worker-symlink");
    let resources = root.join("resources");
    let worker = resources.join(LOCAL_IMAGE_WORKER_DIRECTORY);
    let outside = root.join("outside-worker");
    fs::create_dir_all(&resources).unwrap();
    fs::create_dir_all(&outside).unwrap();
    symlink(&outside, &worker).unwrap();
    let executable = worker.join(LOCAL_IMAGE_WORKER_EXECUTABLE);
    assert_eq!(
        inspect_worker_installation(
            &worker,
            &executable,
            selected_qwen_image_2512_q4_package().unwrap().evidence(),
        ),
        WorkerInstallationReadiness::Mismatch
    );
    let metadata = metadata_for_availability(
        &selected_qwen_image_2512_q4_package().unwrap(),
        LocalImageAvailability::WorkerMismatch,
    );
    assert!(
        !serde_json::to_string(&metadata)
            .unwrap()
            .contains(outside.to_string_lossy().as_ref())
    );
    fs::remove_dir_all(root).unwrap();
}

fn temporary_directory(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("bottie-{label}-{}", uuid::Uuid::new_v4()))
}
