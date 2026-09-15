//! End-to-end tests for the verified local-image execution adapter.
#![allow(dead_code)]

use std::{fs, io::Cursor, path::PathBuf, sync::Arc};

use image::GenericImageView;
use sha2::{Digest, Sha256};
use tokio::sync::watch;

#[path = "../src/local_image_worker/execution.rs"]
mod execution;
#[path = "../src/local_image_worker/manager.rs"]
mod manager;
#[path = "../src/local_image_worker/model_acquisition.rs"]
mod model_acquisition;
#[path = "../src/local_image_worker/model_cache.rs"]
mod model_cache;
#[path = "../src/local_image_worker/protocol.rs"]
mod protocol;
#[path = "../src/local_image_worker/transport.rs"]
mod transport;

use execution::{
    LocalGenerationRequest, LocalImageExecutionError, LocalImageWorkerRuntime,
    VerifiedLocalImageInstallation,
};
use model_acquisition::{ModelFileContract, ModelPackageManifest};
use model_cache::ModelCacheTransaction;

const FIXTURE_RUNTIME_ID: &str = "fixture-runtime@0123456789abcdef";

#[tokio::test]
async fn verified_cache_runs_twice_through_one_private_worker_and_returns_native_pngs() {
    let fixture = prepared_fixture("complete");
    let runtime = LocalImageWorkerRuntime::for_fixture(
        fixture.output_parent.clone(),
        fixture.worker.clone(),
        "normal",
    )
    .unwrap();
    let (_cancel, cancellation) = watch::channel(false);

    for prompt in ["A violet glass robot", "A small moonlit greenhouse"] {
        let output = runtime
            .generate(
                fixture.installation(),
                request(prompt),
                cancellation.clone(),
                || {},
            )
            .await
            .expect("the verified fixture should generate");

        assert_eq!(output.len(), 1);
        assert_eq!(output[0].dimensions(), (512, 512));
        assert_eq!(output[0].seed(), 42);
        let image = image::open(output[0].path()).expect("fixture output should be a valid PNG");
        assert_eq!(image.dimensions(), (512, 512));
        drop(output);
    }

    assert_eq!(runtime.worker_start_count_for_test().await, 1);
    runtime.shutdown().await.unwrap();
    fs::remove_dir_all(fixture.root).unwrap();
}

#[tokio::test]
async fn cancellation_is_cooperative_and_keeps_the_loaded_worker_reusable() {
    let fixture = prepared_fixture("cancel");
    let runtime = Arc::new(
        LocalImageWorkerRuntime::for_fixture(
            fixture.output_parent.clone(),
            fixture.worker.clone(),
            "cooperative-cancel",
        )
        .unwrap(),
    );
    let (cancel, cancellation) = watch::channel(false);
    let task_runtime = runtime.clone();
    let installation = fixture.installation();
    let task = tokio::spawn(async move {
        task_runtime
            .generate(
                installation,
                request("Cancel this image"),
                cancellation,
                || {},
            )
            .await
    });

    tokio::task::yield_now().await;
    cancel.send(true).unwrap();

    assert_eq!(
        task.await.unwrap(),
        Err(LocalImageExecutionError::Cancelled)
    );
    assert!(runtime.has_loaded_worker_for_test().await);
    runtime.shutdown().await.unwrap();
    fs::remove_dir_all(fixture.root).unwrap();
}

#[tokio::test]
async fn runtime_identity_and_missing_output_bytes_fail_closed_without_native_detail() {
    for (mode, expected) in [
        ("wrong-runtime", LocalImageExecutionError::RuntimeMismatch),
        ("missing-output", LocalImageExecutionError::InvalidOutput),
    ] {
        let fixture = prepared_fixture(mode);
        let runtime = LocalImageWorkerRuntime::for_fixture(
            fixture.output_parent.clone(),
            fixture.worker.clone(),
            mode,
        )
        .unwrap();
        let (_cancel, cancellation) = watch::channel(false);

        assert_eq!(
            runtime
                .generate(
                    fixture.installation(),
                    request("Fail closed"),
                    cancellation,
                    || {}
                )
                .await,
            Err(expected)
        );

        let debug = format!("{expected:?}");
        assert!(!debug.contains(fixture.root.to_string_lossy().as_ref()));
        runtime.shutdown().await.unwrap();
        fs::remove_dir_all(fixture.root).unwrap();
    }
}

fn request(prompt: &str) -> LocalGenerationRequest {
    LocalGenerationRequest::new(prompt, 512, 512, 1, 42).unwrap()
}

struct Fixture {
    root: PathBuf,
    output_parent: PathBuf,
    worker: PathBuf,
    cache_root: PathBuf,
    manifest: ModelPackageManifest,
}

impl Fixture {
    fn installation(&self) -> VerifiedLocalImageInstallation {
        VerifiedLocalImageInstallation::for_test(
            self.worker.clone(),
            self.cache_root.clone(),
            self.manifest.clone(),
        )
    }
}

fn prepared_fixture(label: &str) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "bottie-local-execution-{label}-{}",
        uuid::Uuid::new_v4()
    ));
    let cache_root = root.join("cache");
    let output_parent = root.join("outputs");
    fs::create_dir_all(&output_parent).unwrap();
    let manifest = fixture_manifest();
    let transaction = ModelCacheTransaction::open(&cache_root, manifest.clone()).unwrap();
    transaction
        .write_file("weights.bin", 0, &mut Cursor::new(b"weights"))
        .unwrap();
    transaction.promote().unwrap();
    Fixture {
        root,
        output_parent,
        worker: PathBuf::from(env!("CARGO_BIN_EXE_bottie-local-image-worker-fixture")),
        cache_root,
        manifest,
    }
}

fn fixture_manifest() -> ModelPackageManifest {
    let bytes = b"weights";
    ModelPackageManifest {
        model_id: "Qwen/Qwen-Image-2512".into(),
        package_id: "fixture/Qwen-Image-2512-4bit".into(),
        runtime_id: FIXTURE_RUNTIME_ID.into(),
        license: "Apache-2.0".into(),
        source_revision: "0123456789abcdef0123456789abcdef01234567".into(),
        expected_disk_bytes: bytes.len() as u64,
        expected_memory_bytes: 1,
        files: vec![ModelFileContract {
            relative_path: "weights.bin".into(),
            byte_size: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        }],
    }
}
