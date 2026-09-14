//! Tests for the reviewed Qwen Image 2512 MLX package candidate and evidence gate.

use crate::local_image_worker::model_package::{
    MLX_GEN_RUNTIME_REVISION, PackageAcceptanceEvidence, PackageEvidenceError,
    QWEN_IMAGE_2512_GENERATION_PROFILE, QWEN_IMAGE_2512_PACKAGE_REVISION,
    qwen_image_2512_q4_candidate, selected_qwen_image_2512_q4_package,
};

const GIB: u64 = 1_024 * 1_024 * 1_024;

fn evidence() -> PackageAcceptanceEvidence {
    PackageAcceptanceEvidence {
        package_revision: QWEN_IMAGE_2512_PACKAGE_REVISION.into(),
        runtime_revision: MLX_GEN_RUNTIME_REVISION.into(),
        hardware_profile: "apple-m3-max-128gb".into(),
        generation_profile: QWEN_IMAGE_2512_GENERATION_PROFILE.into(),
        generated_rgb_sha256: "1".repeat(64),
        worker_sha256: "2".repeat(64),
        worker_byte_size: 1_024,
        worker_bundle_sha256: "3".repeat(64),
        worker_bundle_byte_size: 2 * GIB,
        peak_memory_bytes: 24 * GIB,
        cancellation_latency_ms: 250,
        network_sandbox_proved: true,
        visual_reviewed: true,
    }
}

#[test]
fn freezes_every_reviewed_qwen_image_2512_q4_repository_file() {
    let candidate = qwen_image_2512_q4_candidate();

    assert_eq!(candidate.model_id(), "Qwen/Qwen-Image-2512");
    assert_eq!(
        candidate.package_id(),
        "AbstractFramework/qwen-image-2512-4bit"
    );
    assert_eq!(
        candidate.source_revision(),
        QWEN_IMAGE_2512_PACKAGE_REVISION
    );
    assert_eq!(candidate.license(), "Apache-2.0");
    assert_eq!(candidate.file_count(), 18);
    assert_eq!(candidate.expected_disk_bytes(), 17_442_350_812);
}

#[test]
fn selection_rejects_mismatched_or_incomplete_runtime_evidence() {
    let candidate = qwen_image_2512_q4_candidate();
    let mut cases = Vec::new();

    let mut wrong_package = evidence();
    wrong_package.package_revision = "0123456789abcdef0123456789abcdef01234567".into();
    cases.push(wrong_package);
    let mut wrong_runtime = evidence();
    wrong_runtime.runtime_revision = "0123456789abcdef0123456789abcdef01234567".into();
    cases.push(wrong_runtime);
    let mut wrong_profile = evidence();
    wrong_profile.generation_profile = "unreviewed".into();
    cases.push(wrong_profile);
    let mut wrong_hardware = evidence();
    wrong_hardware.hardware_profile = "apple-m3-max-64gb".into();
    cases.push(wrong_hardware);
    let mut no_output = evidence();
    no_output.generated_rgb_sha256.clear();
    cases.push(no_output);
    let mut no_worker = evidence();
    no_worker.worker_byte_size = 0;
    cases.push(no_worker);
    let mut oversized_worker = evidence();
    oversized_worker.worker_byte_size = 4 * GIB + 1;
    cases.push(oversized_worker);
    let mut no_bundle = evidence();
    no_bundle.worker_bundle_byte_size = 0;
    cases.push(no_bundle);
    let mut oversized_bundle = evidence();
    oversized_bundle.worker_bundle_byte_size = 8 * GIB + 1;
    cases.push(oversized_bundle);
    let mut no_memory = evidence();
    no_memory.peak_memory_bytes = 0;
    cases.push(no_memory);
    let mut slow_cancel = evidence();
    slow_cancel.cancellation_latency_ms = 3_001;
    cases.push(slow_cancel);
    let mut no_network_sandbox = evidence();
    no_network_sandbox.network_sandbox_proved = false;
    cases.push(no_network_sandbox);
    let mut no_visual_review = evidence();
    no_visual_review.visual_reviewed = false;
    cases.push(no_visual_review);

    for invalid in cases {
        assert_eq!(
            candidate.accept(invalid).unwrap_err(),
            PackageEvidenceError::InvalidEvidence
        );
    }
}

#[test]
fn accepted_evidence_builds_the_exact_manifest_and_hugging_face_source_plan() {
    let selected = qwen_image_2512_q4_candidate().accept(evidence()).unwrap();
    let manifest = selected.manifest();

    assert_eq!(manifest.model_id, "Qwen/Qwen-Image-2512");
    assert_eq!(
        manifest.package_id,
        "AbstractFramework/qwen-image-2512-4bit"
    );
    assert_eq!(
        manifest.runtime_id,
        format!("mlx-gen@{MLX_GEN_RUNTIME_REVISION}")
    );
    assert_eq!(manifest.expected_memory_bytes, 24 * GIB);
    assert_eq!(selected.source_plan().manifest(), manifest);
    assert_eq!(selected.evidence().worker_sha256, "2".repeat(64));
    assert_eq!(selected.evidence().worker_byte_size, 1_024);
    assert_eq!(selected.evidence().worker_bundle_sha256, "3".repeat(64));
    assert_eq!(manifest.files.len(), 18);
    assert_eq!(
        manifest
            .files
            .iter()
            .map(|file| file.byte_size)
            .sum::<u64>(),
        manifest.expected_disk_bytes
    );
    assert!(manifest.files.iter().all(|file| file.sha256.len() == 64));
}

#[cfg(feature = "local-image-runtime-proof")]
#[test]
fn approved_runtime_proof_can_acquire_without_selecting_the_candidate() {
    let candidate = qwen_image_2512_q4_candidate();
    let plan = candidate.proof_source_plan(128 * GIB).unwrap();

    assert_eq!(plan.manifest().model_id, candidate.model_id());
    assert_eq!(
        plan.manifest().expected_disk_bytes,
        candidate.expected_disk_bytes()
    );
    assert_eq!(plan.manifest().expected_memory_bytes, 128 * GIB);
}

#[test]
fn reviewed_runtime_proof_selects_the_exact_measured_package() {
    let selected = selected_qwen_image_2512_q4_package().unwrap();

    assert_eq!(selected.manifest().expected_memory_bytes, 29_526_129_448);
    assert_eq!(
        selected.evidence().generated_rgb_sha256,
        "4cd2921c3cf0a43f791cd725cf72da1ff0be04fe97883a9a4b32332cc9cfc0a5"
    );
    assert_eq!(selected.evidence().worker_byte_size, 56_008_496);
    assert_eq!(selected.evidence().worker_bundle_byte_size, 1_107_880_778);
    assert_eq!(selected.evidence().cancellation_latency_ms, 110);
    assert!(selected.evidence().network_sandbox_proved);
    assert!(selected.evidence().visual_reviewed);
}
