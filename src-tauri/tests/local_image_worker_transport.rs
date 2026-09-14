//! End-to-end process tests for the private local image-worker transport.
#![allow(dead_code)]

use std::{ffi::OsString, path::PathBuf, time::Duration};

#[path = "../src/local_image_worker/manager.rs"]
mod manager;
#[path = "../src/local_image_worker/protocol.rs"]
mod protocol;
#[path = "../src/local_image_worker/transport.rs"]
mod transport;

use manager::{ManagerError, ManagerEvent, WorkerReadiness};
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
