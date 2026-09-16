//! Tests for explicit local-image backend and runtime package selection.

use crate::local_image_worker::{
    model_package::selected_qwen_image_2512_q4_package,
    package_catalog::{
        LocalImageBackend, LocalImageBundleEvidenceStage, LocalImagePlatformProfile,
        LocalImageTargetArchitecture, LocalImageTargetOperatingSystem, RuntimeSelectionError,
        local_image_package_catalog, select_runtime_for_profile,
    },
};

#[test]
fn catalog_binds_the_apple_and_linux_candidates_to_exact_runtime_evidence() {
    let catalog = local_image_package_catalog();

    assert_eq!(catalog.len(), 2);
    let apple = catalog
        .iter()
        .find(|candidate| candidate.backend() == LocalImageBackend::MlxGen)
        .unwrap();
    assert_eq!(apple.worker_version(), "mlx-gen-0.18.2-proof-1");
    assert_eq!(
        apple.runtime_id(),
        "mlx-gen@fca64a283737c68b67a7bfd88d93f7aa9101a95c"
    );
    assert_eq!(apple.model_id(), "Qwen/Qwen-Image-2512");
    assert_eq!(
        apple.model_revision(),
        "423f1f5bf708c6e11eb78881ef9738422cea0814"
    );
    assert_eq!(
        apple.evidence_profile(),
        LocalImagePlatformProfile::AppleM3Max128Gb
    );
    assert_eq!(
        apple.accepted_profile(),
        Some(LocalImagePlatformProfile::AppleM3Max128Gb)
    );
    assert_eq!(
        apple.target_operating_system(),
        LocalImageTargetOperatingSystem::MacOs
    );
    assert_eq!(
        apple.target_architecture(),
        LocalImageTargetArchitecture::Aarch64
    );
    assert_eq!(
        apple.evidence_document(),
        "docs/local-image-model-package.md"
    );
    assert_eq!(
        apple.product_executable_name(),
        Some("bottie-local-image-mlx-worker")
    );
    assert_eq!(
        apple.bundle_evidence_stage(),
        LocalImageBundleEvidenceStage::AcceptedForProductImport
    );
    assert_eq!(
        apple.bundle_evidence_contract(),
        "src-tauri/src/local_image_worker/worker_bundle.rs"
    );

    let linux = catalog
        .iter()
        .find(|candidate| candidate.backend() == LocalImageBackend::Diffusers)
        .unwrap();
    assert_eq!(linux.worker_version(), "diffusers-0.40.0-ngc-25.11-proof-1");
    assert_eq!(linux.runtime_id(), "diffusers@0.40.0+ngc-25.11-arm64");
    assert_eq!(linux.model_id(), "Qwen/Qwen-Image-2512");
    assert_eq!(
        linux.model_revision(),
        "25468b98e3276ca6700de15c6628e51b7de54a26"
    );
    assert_eq!(
        linux.evidence_profile(),
        LocalImagePlatformProfile::LinuxNvidiaDgxSparkGb10
    );
    assert_eq!(linux.accepted_profile(), None);
    assert_eq!(
        linux.target_operating_system(),
        LocalImageTargetOperatingSystem::Linux
    );
    assert_eq!(
        linux.target_architecture(),
        LocalImageTargetArchitecture::Aarch64
    );
    assert_eq!(
        linux.evidence_document(),
        "docs/local-image-linux-nvidia-proof.md"
    );
    assert_eq!(linux.product_executable_name(), None);
    assert_eq!(
        linux.bundle_evidence_stage(),
        LocalImageBundleEvidenceStage::CandidatePreparationOnly
    );
    assert_eq!(
        linux.bundle_evidence_contract(),
        "local-image-worker/diffusers_bundle_candidate.py"
    );
}

#[test]
fn selection_is_explicit_for_apple_and_fails_closed_for_the_linux_proof() {
    let runtime = select_runtime_for_profile(LocalImagePlatformProfile::AppleM3Max128Gb).unwrap();
    let selected = selected_qwen_image_2512_q4_package().unwrap();

    assert_eq!(runtime.backend(), LocalImageBackend::MlxGen);
    assert_eq!(selected.runtime(), runtime);
    assert_eq!(selected.manifest().runtime_id, runtime.runtime_id());
    assert_eq!(selected.manifest().model_id, runtime.model_id());
    assert_eq!(
        selected.manifest().source_revision,
        runtime.model_revision()
    );
    assert_eq!(
        select_runtime_for_profile(LocalImagePlatformProfile::LinuxNvidiaDgxSparkGb10),
        Err(RuntimeSelectionError::NoAcceptedRuntime)
    );
}
