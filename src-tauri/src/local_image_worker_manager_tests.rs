//! Lifecycle policy tests for the long-lived local image-worker manager.

use std::time::Duration;

use crate::local_image_worker::{
    manager::{ManagerError, ManagerEvent, WorkerManager, WorkerReadiness},
    protocol::{
        CURRENT_PROTOCOL_VERSION, HostMessage, ModelLocation, WorkerCapabilities, WorkerMessage,
        WorkerOperation, WorkerOutput, WorkerProgressStage, WorkerResult,
    },
};

fn capabilities() -> WorkerCapabilities {
    WorkerCapabilities {
        runtime_id: "mlx-gen-pinned".into(),
        generation: true,
        supports_seed: true,
        max_outputs: 1,
        max_pixels: 4_194_304,
    }
}

fn model() -> ModelLocation {
    ModelLocation {
        model_id: "Qwen/Qwen-Image-2512".into(),
        model_revision: "0123456789abcdef".into(),
        model_directory: "/private/app-cache/qwen-image-2512".into(),
    }
}

fn ready_manager() -> WorkerManager {
    let mut manager = WorkerManager::new();
    manager.begin_handshake("0.9.0").unwrap();
    manager
        .accept(WorkerMessage::Hello {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            worker_version: "0.1.0".into(),
        })
        .unwrap();
    manager
        .accept(WorkerMessage::Capabilities {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            capabilities: capabilities(),
        })
        .unwrap();
    manager
}

#[test]
fn requires_ordered_hello_and_capabilities_before_loading() {
    let mut manager = WorkerManager::new();
    assert_eq!(manager.readiness(), WorkerReadiness::Stopped);
    assert!(matches!(
        manager.begin_handshake("0.9.0").unwrap(),
        HostMessage::Hello { .. }
    ));
    assert_eq!(manager.readiness(), WorkerReadiness::Starting);
    assert_eq!(
        manager.accept(WorkerMessage::Capabilities {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            capabilities: capabilities(),
        }),
        Err(ManagerError::UnexpectedMessage)
    );

    let mut manager = WorkerManager::new();
    manager.begin_handshake("0.9.0").unwrap();
    manager
        .accept(WorkerMessage::Hello {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            worker_version: "0.1.0".into(),
        })
        .unwrap();
    assert_eq!(manager.readiness(), WorkerReadiness::Starting);
    assert_eq!(
        manager
            .accept(WorkerMessage::Capabilities {
                protocol_version: CURRENT_PROTOCOL_VERSION,
                capabilities: capabilities(),
            })
            .unwrap(),
        ManagerEvent::Ready
    );
    assert_eq!(manager.readiness(), WorkerReadiness::Ready);
}

#[test]
fn loads_once_and_keeps_the_worker_warm_for_generation() {
    let mut manager = ready_manager();
    assert!(matches!(
        manager.begin_load("load-1", model()).unwrap(),
        HostMessage::Load { .. }
    ));
    assert_eq!(manager.readiness(), WorkerReadiness::Busy);
    assert_eq!(
        manager.begin_load("load-2", model()),
        Err(ManagerError::Busy)
    );
    assert_eq!(
        manager
            .accept(WorkerMessage::Result {
                protocol_version: CURRENT_PROTOCOL_VERSION,
                request_id: "load-1".into(),
                operation: WorkerOperation::Load,
                result: WorkerResult::Completed { outputs: vec![] },
            })
            .unwrap(),
        ManagerEvent::ModelLoaded
    );
    assert_eq!(manager.readiness(), WorkerReadiness::Loaded);

    assert!(matches!(
        manager
            .begin_generation("generation-1", "image", 1_024, 1_024, 1, Some(7))
            .unwrap(),
        HostMessage::Generate { .. }
    ));
    assert_eq!(
        manager.begin_generation("generation-2", "image", 1_024, 1_024, 1, None),
        Err(ManagerError::Busy)
    );
}

#[test]
fn accepts_matching_progress_and_terminal_generation_result() {
    let mut manager = ready_manager();
    manager.begin_load("load-1", model()).unwrap();
    manager
        .accept(WorkerMessage::Result {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            request_id: "load-1".into(),
            operation: WorkerOperation::Load,
            result: WorkerResult::Completed { outputs: vec![] },
        })
        .unwrap();
    manager
        .begin_generation("generation-1", "image", 1_024, 1_024, 1, Some(7))
        .unwrap();
    assert_eq!(
        manager
            .accept(WorkerMessage::Progress {
                protocol_version: CURRENT_PROTOCOL_VERSION,
                request_id: "generation-1".into(),
                stage: WorkerProgressStage::Denoising,
                completed_steps: 2,
                total_steps: 20,
            })
            .unwrap(),
        ManagerEvent::Progress
    );
    let output = WorkerOutput {
        output_name: "output-0.png".into(),
        width: 1_024,
        height: 1_024,
        seed: Some(7),
    };
    assert_eq!(
        manager
            .accept(WorkerMessage::Result {
                protocol_version: CURRENT_PROTOCOL_VERSION,
                request_id: "generation-1".into(),
                operation: WorkerOperation::Generate,
                result: WorkerResult::Completed {
                    outputs: vec![output.clone()]
                },
            })
            .unwrap(),
        ManagerEvent::Generated(vec![output])
    );
    assert_eq!(manager.readiness(), WorkerReadiness::Loaded);
}

#[test]
fn cancellation_forces_teardown_only_after_the_named_grace_period() {
    let mut manager = ready_manager();
    manager.begin_load("load-1", model()).unwrap();
    let started = Duration::from_secs(10);
    assert!(matches!(
        manager.cancel_active(started).unwrap(),
        HostMessage::Cancel { .. }
    ));
    assert!(!manager.requires_forced_teardown(started + Duration::from_millis(2_999)));
    assert!(manager.requires_forced_teardown(started + Duration::from_secs(3)));
    assert_eq!(manager.readiness(), WorkerReadiness::Stopped);
}

#[test]
fn rejects_cross_request_results_and_requires_teardown() {
    let mut manager = ready_manager();
    manager.begin_load("load-1", model()).unwrap();
    assert_eq!(
        manager.accept(WorkerMessage::Result {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            request_id: "other".into(),
            operation: WorkerOperation::Load,
            result: WorkerResult::Completed { outputs: vec![] },
        }),
        Err(ManagerError::CorrelationMismatch)
    );
    assert!(manager.take_teardown_required());
    assert_eq!(manager.readiness(), WorkerReadiness::Stopped);
}

#[test]
fn cross_request_progress_also_fails_closed_and_requires_teardown() {
    let mut manager = ready_manager();
    manager.begin_load("load-1", model()).unwrap();
    assert_eq!(
        manager.accept(WorkerMessage::Progress {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            request_id: "other".into(),
            stage: WorkerProgressStage::Loading,
            completed_steps: 1,
            total_steps: 2,
        }),
        Err(ManagerError::CorrelationMismatch)
    );
    assert!(manager.take_teardown_required());
    assert_eq!(manager.readiness(), WorkerReadiness::Stopped);
}

#[test]
fn clean_shutdown_and_failed_reload_discard_process_specific_model_state() {
    let mut manager = ready_manager();
    manager.begin_load("load-1", model()).unwrap();
    manager
        .accept(WorkerMessage::Result {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            request_id: "load-1".into(),
            operation: WorkerOperation::Load,
            result: WorkerResult::Completed { outputs: vec![] },
        })
        .unwrap();
    manager.begin_load("load-2", model()).unwrap();
    manager
        .accept(WorkerMessage::Result {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            request_id: "load-2".into(),
            operation: WorkerOperation::Load,
            result: WorkerResult::Failed {
                code: crate::local_image_worker::protocol::WorkerFailureCode::ModelLoadFailed,
                message: None,
            },
        })
        .unwrap();
    assert_eq!(manager.readiness(), WorkerReadiness::Ready);
    assert_eq!(
        manager.begin_generation("generation-1", "image", 1_024, 1_024, 1, None),
        Err(ManagerError::InvalidState)
    );
    assert!(matches!(
        manager.begin_shutdown().unwrap(),
        HostMessage::Shutdown { .. }
    ));
    manager.mark_stopped();
    assert_eq!(manager.readiness(), WorkerReadiness::Stopped);
}
