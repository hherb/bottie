//! State-machine tests for explicit local-image model acquisition orchestration.

use serde_json::json;

use crate::local_image_worker::{
    acquisition_coordinator::{
        LocalImageAcquisitionApproval, LocalImageAcquisitionErrorCode,
        LocalImageAcquisitionFailure, LocalImageAcquisitionPhase, LocalImageAcquisitionSession,
    },
    availability::LocalImageAvailability,
    availability_service::metadata_for_availability,
    model_cache::CacheResumeProgress,
    model_download::{DownloadError, ModelDownloadProgress},
    model_package::selected_qwen_image_2512_q4_package,
};

fn session() -> LocalImageAcquisitionSession {
    LocalImageAcquisitionSession::new(selected_qwen_image_2512_q4_package().unwrap())
}

#[test]
fn ready_external_21_reuses_installed_bytes_without_offering_an_acquisition() {
    let mut session = session();
    let metadata = crate::local_image_worker::availability_service::server_metadata();
    let status = session.refresh_idle(metadata, None);
    assert_eq!(status.model_id, "ddalcu/Qwen-Image-2.1-MLX-Serve-8bit");
    assert_eq!(status.phase, LocalImageAcquisitionPhase::Ready);
    assert_eq!(status.expected_disk_bytes, 0);
    assert_eq!(status.required_memory_bytes, 0);
    assert_eq!(status.failure, None);
    assert!(
        session
            .begin(
                &approval(),
                &crate::local_image_worker::availability_service::server_metadata(),
                None
            )
            .is_err()
    );
}

fn metadata(
    availability: LocalImageAvailability,
) -> crate::local_image_worker::availability_service::LocalImageAvailabilityMetadata {
    metadata_for_availability(
        &selected_qwen_image_2512_q4_package().unwrap(),
        availability,
    )
}

fn approval() -> LocalImageAcquisitionApproval {
    LocalImageAcquisitionApproval {
        model_id: "Qwen/Qwen-Image-2512".into(),
        package_id: "AbstractFramework/qwen-image-2512-4bit".into(),
        runtime_id: "mlx-gen@fca64a283737c68b67a7bfd88d93f7aa9101a95c".into(),
        license: "Apache-2.0".into(),
        source_revision: "423f1f5bf708c6e11eb78881ef9738422cea0814".into(),
        expected_disk_bytes: 17_442_350_812,
        worker_expected_disk_bytes: 1_107_880_778,
        required_memory_bytes: 29_526_129_448,
        approved: true,
        license_accepted: false,
    }
}

#[test]
fn awaiting_status_discloses_the_exact_package_without_paths_or_hashes() {
    let mut session = session();
    let status = session.refresh_idle(metadata(LocalImageAvailability::ModelMissing), None);
    let serialized = serde_json::to_value(status).unwrap();

    assert_eq!(
        serialized,
        json!({
            "modelId": "Qwen/Qwen-Image-2512",
            "packageId": "AbstractFramework/qwen-image-2512-4bit",
            "runtimeId": "mlx-gen@fca64a283737c68b67a7bfd88d93f7aa9101a95c",
            "license": "Apache-2.0",
            "sourceRevision": "423f1f5bf708c6e11eb78881ef9738422cea0814",
            "expectedDiskBytes": 17_442_350_812_u64,
            "workerExpectedDiskBytes": 1_107_880_778_u64,
            "requiredMemoryBytes": 29_526_129_448_u64,
            "availability": "model_missing",
            "phase": "awaiting_approval",
            "failure": null,
            "downloadedFiles": 0,
            "totalFiles": 18,
            "downloadedBytes": 0,
            "verifiedFiles": 0,
        })
    );
    let text = serialized.to_string().to_ascii_lowercase();
    assert!(!text.contains("sha256"));
    assert!(!text.contains("/users/"));
}

#[test]
fn start_requires_an_exact_affirmative_disclosure_and_verified_worker_gate() {
    let mut session = session();
    let ready_to_install = metadata(LocalImageAvailability::ModelMissing);

    let mut denied = approval();
    denied.approved = false;
    assert_eq!(
        session.begin(&denied, &ready_to_install, None).unwrap_err(),
        LocalImageAcquisitionErrorCode::ApprovalMismatch
    );
    let mut stale = approval();
    stale.source_revision = "0000000000000000000000000000000000000000".into();
    assert_eq!(
        session.begin(&stale, &ready_to_install, None).unwrap_err(),
        LocalImageAcquisitionErrorCode::ApprovalMismatch
    );
    let mut inconsistent_metadata = ready_to_install.clone();
    inconsistent_metadata.package_id = "changed/package".into();
    let mut changed_approval = approval();
    changed_approval.package_id = inconsistent_metadata.package_id.clone();
    assert_eq!(
        session
            .begin(&changed_approval, &inconsistent_metadata, None)
            .unwrap_err(),
        LocalImageAcquisitionErrorCode::ApprovalMismatch
    );
    assert_eq!(
        session
            .begin(
                &approval(),
                &metadata(LocalImageAvailability::WorkerMissing),
                None,
            )
            .unwrap_err(),
        LocalImageAcquisitionErrorCode::Unavailable
    );

    let cancellation = session.begin(&approval(), &ready_to_install, None).unwrap();
    assert!(!cancellation.is_cancelled());
    assert_eq!(
        session.status().phase,
        LocalImageAcquisitionPhase::Downloading
    );
    assert_eq!(
        session
            .begin(&approval(), &ready_to_install, None)
            .unwrap_err(),
        LocalImageAcquisitionErrorCode::AlreadyActive
    );
}

#[test]
fn exact_restart_progress_is_presented_as_resumable_before_approval() {
    let mut session = session();
    let status = session.refresh_idle(
        metadata(LocalImageAvailability::ModelMissing),
        Some(CacheResumeProgress {
            completed_files: 4,
            downloaded_bytes: 2_000_000_000,
        }),
    );

    assert_eq!(status.phase, LocalImageAcquisitionPhase::Paused);
    assert_eq!(status.downloaded_files, 4);
    assert_eq!(status.downloaded_bytes, 2_000_000_000);
    assert_eq!(status.failure, None);

    session
        .begin(
            &approval(),
            &metadata(LocalImageAvailability::ModelMissing),
            Some(CacheResumeProgress {
                completed_files: 4,
                downloaded_bytes: 2_000_000_000,
            }),
        )
        .unwrap();
    assert_eq!(session.status().downloaded_files, 4);
}

#[test]
fn progress_cancel_failure_and_ready_transitions_remain_bounded_and_resumable() {
    let mut session = session();
    session
        .begin(
            &approval(),
            &metadata(LocalImageAvailability::ModelMismatch),
            None,
        )
        .unwrap();
    session
        .record_progress(ModelDownloadProgress {
            completed_files: 3,
            total_files: 18,
            downloaded_bytes: 1_500_000_000,
            total_bytes: 17_442_350_812,
        })
        .unwrap();
    assert_eq!(session.status().downloaded_files, 3);

    session.request_cancel().unwrap();
    assert_eq!(
        session.status().phase,
        LocalImageAcquisitionPhase::Cancelling
    );
    session.finish_failure(DownloadError::Cancelled).unwrap();
    assert_eq!(session.status().phase, LocalImageAcquisitionPhase::Paused);
    assert_eq!(
        session.status().failure,
        Some(LocalImageAcquisitionFailure::Cancelled)
    );
    assert_eq!(session.status().downloaded_bytes, 1_500_000_000);

    session
        .begin(
            &approval(),
            &metadata(LocalImageAvailability::ModelMissing),
            Some(CacheResumeProgress {
                completed_files: 3,
                downloaded_bytes: 1_500_000_000,
            }),
        )
        .unwrap();
    session
        .record_progress(ModelDownloadProgress {
            completed_files: 18,
            total_files: 18,
            downloaded_bytes: 17_442_350_812,
            total_bytes: 17_442_350_812,
        })
        .unwrap();
    assert_eq!(
        session.status().phase,
        LocalImageAcquisitionPhase::Verifying
    );
    session
        .finish_success(&metadata(LocalImageAvailability::Ready))
        .unwrap();
    assert_eq!(session.status().phase, LocalImageAcquisitionPhase::Ready);
    assert_eq!(session.status().verified_files, 18);
}

#[test]
fn progress_with_changed_totals_fails_closed() {
    let mut session = session();
    session
        .begin(
            &approval(),
            &metadata(LocalImageAvailability::ModelMissing),
            None,
        )
        .unwrap();

    assert_eq!(
        session
            .record_progress(ModelDownloadProgress {
                completed_files: 1,
                total_files: 19,
                downloaded_bytes: 1,
                total_bytes: 17_442_350_812,
            })
            .unwrap_err(),
        LocalImageAcquisitionErrorCode::InvalidState
    );
}

#[test]
fn ready_moss_keeps_its_identity_in_the_initial_native_status_without_offering_qwen_downloads() {
    let mut session = session();
    let mut metadata = crate::local_image_worker::availability_service::server_metadata();
    metadata.model_id = "mlx-community/MOSS-Image-SFT-bf16".into();
    metadata.package_id = metadata.model_id.clone();
    metadata.runtime_id = "moss-image-mlx@external".into();
    metadata.license = "Apache-2.0".into();
    metadata.source_revision = "ea698e99312015505884559ebad94b4a5a0d0753".into();
    metadata.expected_disk_bytes = 27_219_783_878;
    let status = session.refresh_idle(metadata, None);
    assert_eq!(status.model_id, "mlx-community/MOSS-Image-SFT-bf16");
    assert_eq!(status.phase, LocalImageAcquisitionPhase::Ready);
    assert_eq!(status.expected_disk_bytes, 27_219_783_878);
    assert_eq!(status.total_files, 0);
    assert_eq!(status.failure, None);
}
