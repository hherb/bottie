//! End-to-end process tests for the private local image-worker transport.
#![allow(dead_code)]

use std::{ffi::OsString, fs, io::Cursor, path::PathBuf, time::Duration};

use sha2::{Digest, Sha256};

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

use manager::{ManagerError, ManagerEvent, WorkerReadiness};
use model_acquisition::{ModelFileContract, ModelPackageManifest};
use model_cache::{CachedModelLoadError, ModelCacheTransaction, begin_cached_model_load};
use protocol::ModelLocation;
use transport::{TransportError, TransportTimeouts, WorkerProcessSpec, WorkerTransport};

const TEST_HANDSHAKE_DEADLINE: Duration = Duration::from_secs(2);
const TEST_IO_DEADLINE: Duration = Duration::from_secs(1);
const TEST_SHUTDOWN_DEADLINE: Duration = Duration::from_millis(500);

fn fixture_spec(mode: &str) -> WorkerProcessSpec {
    WorkerProcessSpec::new(
        PathBuf::from(env!("CARGO_BIN_EXE_bottie-local-image-worker-fixture")),
        vec![OsString::from(mode)],
    )
}

fn test_timeouts() -> TransportTimeouts {
    TransportTimeouts::new(
        TEST_HANDSHAKE_DEADLINE,
        TEST_IO_DEADLINE,
        TEST_IO_DEADLINE,
        TEST_SHUTDOWN_DEADLINE,
    )
}

#[cfg(not(target_os = "windows"))]
fn model_directory() -> &'static str {
    "/private/app-cache/qwen-image-2512"
}

#[cfg(target_os = "windows")]
fn model_directory() -> &'static str {
    r"C:\app-cache\qwen-image-2512"
}

fn model() -> ModelLocation {
    ModelLocation {
        model_id: "Qwen/Qwen-Image-2512".into(),
        model_revision: "0123456789abcdef".into(),
        model_directory: model_directory().into(),
    }
}

fn cache_manifest() -> ModelPackageManifest {
    let bytes = b"weights";
    ModelPackageManifest {
        model_id: "Qwen/Qwen-Image-2512".into(),
        package_id: "fixture/Qwen-Image-2512-4bit".into(),
        runtime_id: "fixture-runtime@0123456789abcdef".into(),
        license: "Apache-2.0".into(),
        source_revision: "0123456789abcdef0123456789abcdef01234567".into(),
        expected_disk_bytes: bytes.len() as u64,
        expected_memory_bytes: 20 * 1_024 * 1_024 * 1_024,
        files: vec![ModelFileContract {
            relative_path: "weights.bin".into(),
            byte_size: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        }],
    }
}

fn prepared_cache(name: &str) -> (PathBuf, ModelPackageManifest, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "bottie-worker-cache-{name}-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&root).unwrap();
    let manifest = cache_manifest();
    let transaction = ModelCacheTransaction::open(&root, manifest.clone()).unwrap();
    transaction
        .write_file("weights.bin", 0, &mut Cursor::new(b"weights"))
        .unwrap();
    let final_root = transaction.final_root_for_test();
    transaction.promote().unwrap();
    (root, manifest, final_root)
}

async fn loaded_transport(mode: &str) -> WorkerTransport {
    let mut transport =
        WorkerTransport::spawn_with_timeouts(fixture_spec(mode), "0.9.0", test_timeouts())
            .await
            .unwrap();
    transport.begin_load("load-1", model()).await.unwrap();
    assert_eq!(
        transport.next_event().await.unwrap(),
        ManagerEvent::ModelLoaded
    );
    transport
}

#[tokio::test]
async fn rejects_relative_executables_and_zero_deadlines_before_spawn() {
    let relative = WorkerProcessSpec::new(PathBuf::from("fixture"), Vec::new());
    assert_eq!(
        WorkerTransport::spawn_with_timeouts(relative, "0.9.0", test_timeouts())
            .await
            .unwrap_err(),
        TransportError::InvalidExecutable
    );
    let zero_timeouts = TransportTimeouts::new(
        Duration::ZERO,
        TEST_IO_DEADLINE,
        TEST_IO_DEADLINE,
        TEST_SHUTDOWN_DEADLINE,
    );
    assert_eq!(
        WorkerTransport::spawn_with_timeouts(fixture_spec("normal"), "0.9.0", zero_timeouts)
            .await
            .unwrap_err(),
        TransportError::InvalidTimeout
    );
}

#[tokio::test]
async fn fragmented_consecutive_handshake_frames_keep_one_worker_warm() {
    let mut transport = loaded_transport("fragmented").await;
    assert_eq!(transport.readiness(), WorkerReadiness::Loaded);
    transport
        .begin_generation("generation-1", "fixture image", 1_024, 1_024, 1, Some(7))
        .await
        .unwrap();
    assert_eq!(
        transport.next_event().await.unwrap(),
        ManagerEvent::Progress
    );
    assert!(matches!(
        transport.next_event().await.unwrap(),
        ManagerEvent::Generated(outputs) if outputs.len() == 1
    ));
    transport.shutdown().await.unwrap();
}

#[tokio::test]
async fn early_exit_hung_handshake_and_malformed_output_fail_closed() {
    for (mode, expected) in [
        ("early-exit", TransportError::UnexpectedEof),
        ("hung-handshake", TransportError::HandshakeTimeout),
        ("malformed", TransportError::Protocol),
        (
            "wrong-order",
            TransportError::Manager(ManagerError::UnexpectedMessage),
        ),
    ] {
        let result =
            WorkerTransport::spawn_with_timeouts(fixture_spec(mode), "0.9.0", test_timeouts())
                .await;
        assert_eq!(result.unwrap_err(), expected, "fixture mode {mode}");
    }
}

#[tokio::test]
async fn ordinary_read_timeout_kills_and_reaps_the_worker() {
    let mut transport = loaded_transport("hung-operation").await;
    transport
        .begin_generation("generation-1", "fixture image", 1_024, 1_024, 1, Some(7))
        .await
        .unwrap();
    assert_eq!(
        transport.next_event().await.unwrap_err(),
        TransportError::ReadTimeout
    );
    assert_eq!(transport.readiness(), WorkerReadiness::Stopped);
}

#[tokio::test]
async fn inherited_environment_and_unbounded_stderr_fail_closed() {
    WorkerTransport::spawn_with_timeouts(fixture_spec("normal"), "0.9.0", test_timeouts())
        .await
        .unwrap()
        .shutdown()
        .await
        .unwrap();

    let result = WorkerTransport::spawn_with_timeouts(
        fixture_spec("stderr-flood"),
        "0.9.0",
        test_timeouts(),
    )
    .await;
    assert_eq!(result.unwrap_err(), TransportError::StderrLimitExceeded);
}

#[tokio::test]
async fn cooperative_cancellation_inside_grace_keeps_worker_available() {
    let mut transport = loaded_transport("cooperative-cancel").await;
    transport
        .begin_generation("generation-1", "fixture image", 1_024, 1_024, 1, Some(7))
        .await
        .unwrap();
    assert_eq!(
        transport.next_event().await.unwrap(),
        ManagerEvent::Progress
    );
    transport.cancel_active().await.unwrap();
    assert_eq!(
        transport.next_event().await.unwrap(),
        ManagerEvent::Cancelled
    );
    assert_eq!(transport.readiness(), WorkerReadiness::Loaded);
    transport.shutdown().await.unwrap();
}

#[tokio::test]
async fn ignored_cancellation_is_forcibly_killed_and_reaped_after_three_seconds() {
    let mut transport = loaded_transport("ignore-cancel").await;
    transport
        .begin_generation("generation-1", "fixture image", 1_024, 1_024, 1, Some(7))
        .await
        .unwrap();
    assert_eq!(
        transport.next_event().await.unwrap(),
        ManagerEvent::Progress
    );
    transport.cancel_active().await.unwrap();
    assert_eq!(
        transport.next_event().await.unwrap_err(),
        TransportError::CancellationTimeout
    );
    assert_eq!(transport.readiness(), WorkerReadiness::Stopped);
}

#[tokio::test]
async fn clean_shutdown_reaps_and_hung_shutdown_is_forcibly_reaped() {
    loaded_transport("normal").await.shutdown().await.unwrap();

    let result = loaded_transport("hung-shutdown").await.shutdown().await;
    assert_eq!(result.unwrap_err(), TransportError::ShutdownTimeout);
}

#[tokio::test]
async fn cached_load_reverifies_the_exact_promoted_package_at_the_transport_boundary() {
    let (root, manifest, _) = prepared_cache("verified-load");
    let mut transport =
        WorkerTransport::spawn_with_timeouts(fixture_spec("normal"), "0.9.0", test_timeouts())
            .await
            .unwrap();
    begin_cached_model_load(&mut transport, "load-1", &root, manifest)
        .await
        .unwrap();
    assert_eq!(
        transport.next_event().await.unwrap(),
        ManagerEvent::ModelLoaded
    );
    transport.shutdown().await.unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn post_promotion_mutation_is_rejected_before_any_worker_load_frame() {
    let (root, manifest, final_root) = prepared_cache("mutated-load");
    fs::write(final_root.join("weights.bin"), b"changed").unwrap();
    let mut transport =
        WorkerTransport::spawn_with_timeouts(fixture_spec("normal"), "0.9.0", test_timeouts())
            .await
            .unwrap();
    assert_eq!(
        begin_cached_model_load(&mut transport, "load-1", &root, manifest).await,
        Err(CachedModelLoadError::Cache(
            model_cache::CacheError::Integrity
        ))
    );
    assert_eq!(transport.readiness(), WorkerReadiness::Ready);
    transport.shutdown().await.unwrap();
    fs::remove_dir_all(root).unwrap();
}
