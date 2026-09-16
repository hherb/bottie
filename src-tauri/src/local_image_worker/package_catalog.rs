//! Closed backend and runtime catalog for local image package selection.

/// Exact open-weight model shared by the reviewed Apple and Linux candidates.
pub(crate) const QWEN_IMAGE_2512_MODEL_ID: &str = "Qwen/Qwen-Image-2512";
/// Immutable mixed q4/q8 model revision used by the accepted Apple runtime.
pub(crate) const APPLE_MLX_MODEL_REVISION: &str = "423f1f5bf708c6e11eb78881ef9738422cea0814";
/// Immutable MLX-Gen commit embedded in the accepted Apple worker.
pub(crate) const MLX_GEN_RUNTIME_REVISION: &str = "fca64a283737c68b67a7bfd88d93f7aa9101a95c";
/// Exact executable basename inside the accepted Apple worker bundle.
pub(crate) const APPLE_MLX_WORKER_EXECUTABLE: &str = "bottie-local-image-mlx-worker";

const APPLE_MLX_WORKER_VERSION: &str = "mlx-gen-0.18.2-proof-1";
const APPLE_MLX_RUNTIME_ID: &str = "mlx-gen@fca64a283737c68b67a7bfd88d93f7aa9101a95c";
const APPLE_EVIDENCE_DOCUMENT: &str = "docs/local-image-model-package.md";
const APPLE_BUNDLE_EVIDENCE_CONTRACT: &str = "src-tauri/src/local_image_worker/worker_bundle.rs";
const LINUX_DIFFUSERS_WORKER_VERSION: &str = "diffusers-0.40.0-ngc-25.11-proof-1";
const LINUX_DIFFUSERS_RUNTIME_ID: &str = "diffusers@0.40.0+ngc-25.11-arm64";
const LINUX_DIFFUSERS_MODEL_REVISION: &str = "25468b98e3276ca6700de15c6628e51b7de54a26";
const LINUX_EVIDENCE_DOCUMENT: &str = "docs/local-image-linux-nvidia-proof.md";
const LINUX_BUNDLE_EVIDENCE_CONTRACT: &str = "local-image-worker/diffusers_bundle_candidate.py";

/// Runtime family responsible for loading and generating with one local image package.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LocalImageBackend {
    /// The pinned MLX-Gen worker accepted on one Apple silicon profile.
    MlxGen,
    /// The pinned Diffusers CUDA worker exercised only by the Linux feasibility proof.
    Diffusers,
}

/// Closed platform profiles named by reviewed local-image evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LocalImagePlatformProfile {
    /// Apple M3 Max with exactly 128 GiB unified memory.
    AppleM3Max128Gb,
    /// NVIDIA DGX Spark GB10 with 128 GB coherent unified memory on Linux ARM64.
    LinuxNvidiaDgxSparkGb10,
}

/// Closed operating systems named by reviewed local-image runtime candidates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LocalImageTargetOperatingSystem {
    /// Apple's macOS desktop operating system.
    MacOs,
    /// A Linux operating system with the candidate's exact runtime constraints.
    Linux,
}

/// Closed processor architectures named by reviewed local-image runtime candidates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LocalImageTargetArchitecture {
    /// 64-bit ARM.
    Aarch64,
}

/// Review stage reached by one runtime candidate's exact worker-bundle evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LocalImageBundleEvidenceStage {
    /// Exact bytes are accepted for the existing explicit app-owned import path.
    AcceptedForProductImport,
    /// A deterministic candidate record can be prepared, but no produced bytes are accepted.
    CandidatePreparationOnly,
}

/// One immutable backend, worker, model, and evidence-document binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LocalImageRuntimeCandidate {
    backend: LocalImageBackend,
    worker_version: &'static str,
    runtime_id: &'static str,
    model_id: &'static str,
    model_revision: &'static str,
    evidence_profile: LocalImagePlatformProfile,
    accepted_profile: Option<LocalImagePlatformProfile>,
    target_operating_system: LocalImageTargetOperatingSystem,
    target_architecture: LocalImageTargetArchitecture,
    evidence_document: &'static str,
    bundle_evidence_stage: LocalImageBundleEvidenceStage,
    bundle_evidence_contract: &'static str,
    product_executable_name: Option<&'static str>,
}

impl LocalImageRuntimeCandidate {
    /// Returns the implementation family used by this exact worker.
    pub(crate) const fn backend(self) -> LocalImageBackend {
        self.backend
    }

    /// Returns the exact private-protocol worker version.
    pub(crate) const fn worker_version(self) -> &'static str {
        self.worker_version
    }

    /// Returns the exact runtime identity negotiated with the worker.
    pub(crate) const fn runtime_id(self) -> &'static str {
        self.runtime_id
    }

    /// Returns the exact upstream model identity.
    pub(crate) const fn model_id(self) -> &'static str {
        self.model_id
    }

    /// Returns the exact model revision the worker may load.
    pub(crate) const fn model_revision(self) -> &'static str {
        self.model_revision
    }

    /// Returns the named platform profile on which this candidate was exercised.
    pub(crate) const fn evidence_profile(self) -> LocalImagePlatformProfile {
        self.evidence_profile
    }

    /// Returns the product-accepted profile, or none for feasibility-only evidence.
    pub(crate) const fn accepted_profile(self) -> Option<LocalImagePlatformProfile> {
        self.accepted_profile
    }

    /// Returns the operating system required by this runtime candidate.
    pub(crate) const fn target_operating_system(self) -> LocalImageTargetOperatingSystem {
        self.target_operating_system
    }

    /// Returns the processor architecture required by this runtime candidate.
    pub(crate) const fn target_architecture(self) -> LocalImageTargetArchitecture {
        self.target_architecture
    }

    /// Returns the repository-relative evidence document reviewed for this candidate.
    pub(crate) const fn evidence_document(self) -> &'static str {
        self.evidence_document
    }

    /// Returns the review stage reached by this candidate's exact bundle bytes.
    pub(crate) const fn bundle_evidence_stage(self) -> LocalImageBundleEvidenceStage {
        self.bundle_evidence_stage
    }

    /// Returns the repository-relative contract used to produce or re-verify bundle evidence.
    pub(crate) const fn bundle_evidence_contract(self) -> &'static str {
        self.bundle_evidence_contract
    }

    /// Returns the importable product executable basename, absent for proof-only runtimes.
    pub(crate) const fn product_executable_name(self) -> Option<&'static str> {
        self.product_executable_name
    }
}

/// Stable failure when no product-accepted runtime exists for a platform profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeSelectionError {
    /// The catalog has no accepted runtime for the requested profile.
    NoAcceptedRuntime,
}

const LOCAL_IMAGE_PACKAGE_CATALOG: [LocalImageRuntimeCandidate; 2] = [
    LocalImageRuntimeCandidate {
        backend: LocalImageBackend::MlxGen,
        worker_version: APPLE_MLX_WORKER_VERSION,
        runtime_id: APPLE_MLX_RUNTIME_ID,
        model_id: QWEN_IMAGE_2512_MODEL_ID,
        model_revision: APPLE_MLX_MODEL_REVISION,
        evidence_profile: LocalImagePlatformProfile::AppleM3Max128Gb,
        accepted_profile: Some(LocalImagePlatformProfile::AppleM3Max128Gb),
        target_operating_system: LocalImageTargetOperatingSystem::MacOs,
        target_architecture: LocalImageTargetArchitecture::Aarch64,
        evidence_document: APPLE_EVIDENCE_DOCUMENT,
        bundle_evidence_stage: LocalImageBundleEvidenceStage::AcceptedForProductImport,
        bundle_evidence_contract: APPLE_BUNDLE_EVIDENCE_CONTRACT,
        product_executable_name: Some(APPLE_MLX_WORKER_EXECUTABLE),
    },
    LocalImageRuntimeCandidate {
        backend: LocalImageBackend::Diffusers,
        worker_version: LINUX_DIFFUSERS_WORKER_VERSION,
        runtime_id: LINUX_DIFFUSERS_RUNTIME_ID,
        model_id: QWEN_IMAGE_2512_MODEL_ID,
        model_revision: LINUX_DIFFUSERS_MODEL_REVISION,
        evidence_profile: LocalImagePlatformProfile::LinuxNvidiaDgxSparkGb10,
        accepted_profile: None,
        target_operating_system: LocalImageTargetOperatingSystem::Linux,
        target_architecture: LocalImageTargetArchitecture::Aarch64,
        evidence_document: LINUX_EVIDENCE_DOCUMENT,
        bundle_evidence_stage: LocalImageBundleEvidenceStage::CandidatePreparationOnly,
        bundle_evidence_contract: LINUX_BUNDLE_EVIDENCE_CONTRACT,
        product_executable_name: None,
    },
];

/// Returns every closed local-image runtime candidate without selecting one.
pub(crate) const fn local_image_package_catalog() -> &'static [LocalImageRuntimeCandidate] {
    &LOCAL_IMAGE_PACKAGE_CATALOG
}

/// Selects only a runtime whose reviewed profile is explicitly accepted for product use.
pub(crate) fn select_runtime_for_profile(
    profile: LocalImagePlatformProfile,
) -> Result<LocalImageRuntimeCandidate, RuntimeSelectionError> {
    local_image_package_catalog()
        .iter()
        .copied()
        .find(|candidate| candidate.accepted_profile() == Some(profile))
        .ok_or(RuntimeSelectionError::NoAcceptedRuntime)
}
