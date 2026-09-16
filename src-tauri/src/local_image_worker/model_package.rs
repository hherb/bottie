//! Immutable Qwen Image package facts and native-only runtime evidence acceptance.

use super::{
    model_acquisition::{ModelFileContract, ModelPackageManifest},
    model_download::{ModelFileSource, ModelSourcePlan},
    package_catalog::{
        APPLE_MLX_MODEL_REVISION, LocalImagePlatformProfile, LocalImageRuntimeCandidate,
        QWEN_IMAGE_2512_MODEL_ID, select_runtime_for_profile,
    },
};

/// Immutable Hugging Face revision for the reviewed mixed q4/q8 MLX-Gen package.
pub(crate) const QWEN_IMAGE_2512_PACKAGE_REVISION: &str = APPLE_MLX_MODEL_REVISION;
/// Immutable MLX-Gen commit behind the package's declared minimum generated runtime.
pub(crate) use super::package_catalog::MLX_GEN_RUNTIME_REVISION;
/// Reproducible generation profile required before this package can be selected.
pub(crate) const QWEN_IMAGE_2512_GENERATION_PROFILE: &str = "qwen-image-2512-512x512-15-step-v1";
/// Exact Apple-silicon target measured by the initial package acceptance run.
pub(crate) const QWEN_IMAGE_2512_HARDWARE_PROFILE: &str = "apple-m3-max-128gb";

const MODEL_ID: &str = QWEN_IMAGE_2512_MODEL_ID;
const PACKAGE_ID: &str = "AbstractFramework/qwen-image-2512-4bit";
const LICENSE: &str = "Apache-2.0";
const EXPECTED_DISK_BYTES: u64 = 17_442_350_812;
const MAX_CANCELLATION_LATENCY_MS: u64 = 3_000;
const MAX_WORKER_BYTES: u64 = 4 * 1_024 * 1_024 * 1_024;
const MAX_WORKER_BUNDLE_BYTES: u64 = 8 * 1_024 * 1_024 * 1_024;
const SHA256_HEX_BYTES: usize = 64;
const ACCEPTED_GENERATED_RGB_SHA256: &str =
    "4cd2921c3cf0a43f791cd725cf72da1ff0be04fe97883a9a4b32332cc9cfc0a5";
const ACCEPTED_WORKER_SHA256: &str =
    "187bfc58b30328278e52500c2b28999f2ff56cf510dd0790a3e90100aa81464b";
const ACCEPTED_WORKER_BYTES: u64 = 56_008_496;
const ACCEPTED_WORKER_BUNDLE_SHA256: &str =
    "7a5db3e6c1c59c5d9264d8fed1f40f9a160bc8c4d64c642464588504856f0bf1";
const ACCEPTED_WORKER_BUNDLE_BYTES: u64 = 1_107_880_778;
const ACCEPTED_PEAK_MEMORY_BYTES: u64 = 29_526_129_448;
const ACCEPTED_CANCELLATION_LATENCY_MS: u64 = 110;

#[derive(Clone, Copy)]
struct ReviewedFile {
    relative_path: &'static str,
    byte_size: u64,
    sha256: &'static str,
    strong_etag: &'static str,
}

const REVIEWED_FILES: &[ReviewedFile] = &[
    reviewed_file(
        ".gitattributes",
        1_580,
        "caf3ca02d5e883f643e939cbed54f94237f94c171797f0dfea5e25c7e33315b3",
        "\"f1ca65ce6a5b0774360fde1ec05157d9f937aaa2\"",
    ),
    reviewed_file(
        "LICENSE.md",
        11_358,
        "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30",
        "\"d645695673349e3947e8e5ae42332d0ac3164cd7\"",
    ),
    reviewed_file(
        "README.md",
        2_218,
        "c53e2448113dbd9575608c69f640233b439312b23cbb462634bc3e41e46ff439",
        "\"a58fbb535f175749d3670b4b0ad19f424cee7bcf\"",
    ),
    reviewed_file(
        "text_encoder/0.safetensors",
        2_142_182_659,
        "b17f5c5e9a67d4e1e99661429afe277cacc6dfa1b29d505bf38d8b5f1e4ad0da",
        "\"b17f5c5e9a67d4e1e99661429afe277cacc6dfa1b29d505bf38d8b5f1e4ad0da\"",
    ),
    reviewed_file(
        "text_encoder/1.safetensors",
        1_835_614_566,
        "c379ed1c78714f3aacbd5a6e3ad708f0cc33c4a4f6689ef3010e294dbfd93d54",
        "\"c379ed1c78714f3aacbd5a6e3ad708f0cc33c4a4f6689ef3010e294dbfd93d54\"",
    ),
    reviewed_file(
        "text_encoder/model.safetensors.index.json",
        49_256,
        "67c9299cddc8eba6a9e8f6367346ac938cfce9af14e56048b3c2632c54d367a1",
        "\"d876abafe0142384f7283ae648453245bdb4d113\"",
    ),
    reviewed_file(
        "tokenizer/tokenizer.json",
        11_417_006,
        "444a80c37c7f34f485c050cf374595a0cb7d88735459cf0505fd204453c33178",
        "\"444a80c37c7f34f485c050cf374595a0cb7d88735459cf0505fd204453c33178\"",
    ),
    reviewed_file(
        "tokenizer/tokenizer_config.json",
        269,
        "ebf4e7d1ac441e3d84a44228a1a7392977b375a34c8fbc8b87c68e6a832e98ba",
        "\"3299687e615af5b239e5000b69cb0cb62ee2008f\"",
    ),
    reviewed_file(
        "transformer/0.safetensors",
        2_133_769_869,
        "bfbf470e8f82255b91c09cedb70c691b262f1ab54c233818b6b91185bbce71b1",
        "\"bfbf470e8f82255b91c09cedb70c691b262f1ab54c233818b6b91185bbce71b1\"",
    ),
    reviewed_file(
        "transformer/1.safetensors",
        2_142_940_100,
        "0dccd27b42a47929d1c3d22bce3666dcfda98ef8c95a1e866f5a249d701ee1f5",
        "\"0dccd27b42a47929d1c3d22bce3666dcfda98ef8c95a1e866f5a249d701ee1f5\"",
    ),
    reviewed_file(
        "transformer/2.safetensors",
        2_103_978_106,
        "74db3805640dc9d9c625ee6f5cd944f2850159b0a105bdd85ee2d850c8727108",
        "\"74db3805640dc9d9c625ee6f5cd944f2850159b0a105bdd85ee2d850c8727108\"",
    ),
    reviewed_file(
        "transformer/3.safetensors",
        2_121_682_265,
        "9ba68067e9412f2a0cce2174f06fd907aaf2723b4762a1db615571ac44f81a0b",
        "\"9ba68067e9412f2a0cce2174f06fd907aaf2723b4762a1db615571ac44f81a0b\"",
    ),
    reviewed_file(
        "transformer/4.safetensors",
        2_142_940_118,
        "937f9a01dba1e116511b83c8ca8c3d27b239d5c2a25a8ebac71dff19c9c0d7f5",
        "\"937f9a01dba1e116511b83c8ca8c3d27b239d5c2a25a8ebac71dff19c9c0d7f5\"",
    ),
    reviewed_file(
        "transformer/5.safetensors",
        2_103_978_210,
        "017c1a3b4afc6bc12f287e33d368df67e949528c6f48847bbf2a5d5e151613af",
        "\"017c1a3b4afc6bc12f287e33d368df67e949528c6f48847bbf2a5d5e151613af\"",
    ),
    reviewed_file(
        "transformer/6.safetensors",
        449_941_880,
        "a463c3ad6caec4f7ed524af88da2b92d5247ff410dbe55a208838dafdb6c6ec0",
        "\"a463c3ad6caec4f7ed524af88da2b92d5247ff410dbe55a208838dafdb6c6ec0\"",
    ),
    reviewed_file(
        "transformer/model.safetensors.index.json",
        241_693,
        "f9f03fac574131598f16cdee75d60ffa15dc0fd5e60c37d89bcc74aa3653366c",
        "\"30943ca4517231bf5b17be2dce3f7c6cb7f70731\"",
    ),
    reviewed_file(
        "vae/0.safetensors",
        253_586_147,
        "2c2db147e652a0e011c1c5f40345ab84e9a95819ee9c7b29eb29f245cccd8f64",
        "\"2c2db147e652a0e011c1c5f40345ab84e9a95819ee9c7b29eb29f245cccd8f64\"",
    ),
    reviewed_file(
        "vae/model.safetensors.index.json",
        13_512,
        "feda700a9485ff767135f5ca4ea92be31a0b8465c624959814b505a827428f3c",
        "\"b0196601c3c7ed7381ccb7fa2a085b402241130a\"",
    ),
];

const fn reviewed_file(
    relative_path: &'static str,
    byte_size: u64,
    sha256: &'static str,
    strong_etag: &'static str,
) -> ReviewedFile {
    ReviewedFile {
        relative_path,
        byte_size,
        sha256,
        strong_etag,
    }
}

/// Stable rejection for package evidence that does not match every reviewed acceptance fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PackageEvidenceError {
    /// At least one immutable identity, measurement, or review gate was absent or mismatched.
    InvalidEvidence,
}

/// Native-only measurements required before the reviewed model candidate becomes selectable.
#[derive(Clone, Debug)]
pub(crate) struct PackageAcceptanceEvidence {
    /// Immutable package revision used by the measured run.
    pub(crate) package_revision: String,
    /// Immutable MLX-Gen revision embedded in the measured worker.
    pub(crate) runtime_revision: String,
    /// Exact chip and unified-memory profile used by the measured run.
    pub(crate) hardware_profile: String,
    /// Exact bounded generation profile used to produce the reviewed PNG.
    pub(crate) generation_profile: String,
    /// SHA-256 digest of the decoded, visually reviewed RGB pixel bytes.
    pub(crate) generated_rgb_sha256: String,
    /// SHA-256 digest of the exact worker executable used by the run.
    pub(crate) worker_sha256: String,
    /// Exact worker executable byte length used by the run.
    pub(crate) worker_byte_size: u64,
    /// SHA-256 digest of the canonical regular-file inventory for the exact worker runtime bundle.
    pub(crate) worker_bundle_sha256: String,
    /// Sum of regular-file bytes represented by the canonical worker bundle inventory.
    pub(crate) worker_bundle_byte_size: u64,
    /// Measured whole-process peak memory for the bounded generation profile.
    pub(crate) peak_memory_bytes: u64,
    /// Measured cooperative cancellation latency at an active generation step.
    pub(crate) cancellation_latency_ms: u64,
    /// Whether the worker's operating-system network sandbox was exercised and denied a connection.
    pub(crate) network_sandbox_proved: bool,
    /// Whether a human reviewed the decoded output for usable image content.
    pub(crate) visual_reviewed: bool,
}

/// Reviewed immutable repository facts that remain unavailable until runtime evidence passes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ModelPackageCandidate;

impl ModelPackageCandidate {
    /// Returns the exact upstream base-model identity.
    pub(crate) fn model_id(self) -> &'static str {
        MODEL_ID
    }

    /// Returns the exact MLX-Gen package repository identity.
    pub(crate) fn package_id(self) -> &'static str {
        PACKAGE_ID
    }

    /// Returns the immutable package repository revision.
    pub(crate) fn source_revision(self) -> &'static str {
        QWEN_IMAGE_2512_PACKAGE_REVISION
    }

    /// Returns the reviewed SPDX license identifier.
    pub(crate) fn license(self) -> &'static str {
        LICENSE
    }

    /// Returns the number of files required for exact activation.
    pub(crate) fn file_count(self) -> usize {
        REVIEWED_FILES.len()
    }

    /// Returns the exact sum of every reviewed repository file.
    pub(crate) fn expected_disk_bytes(self) -> u64 {
        EXPECTED_DISK_BYTES
    }

    /// Builds the exact acquisition plan used only by the explicitly enabled runtime-proof tool.
    #[cfg(feature = "local-image-runtime-proof")]
    pub(crate) fn proof_source_plan(
        self,
        expected_memory_bytes: u64,
    ) -> Result<ModelSourcePlan, PackageEvidenceError> {
        ModelSourcePlan::for_hugging_face(
            model_manifest(expected_memory_bytes),
            PACKAGE_ID,
            QWEN_IMAGE_2512_PACKAGE_REVISION,
            source_contracts(),
        )
        .map_err(|_| PackageEvidenceError::InvalidEvidence)
    }

    /// Accepts only measurements tied to this exact package, runtime, hardware, and profile.
    pub(crate) fn accept(
        self,
        evidence: PackageAcceptanceEvidence,
    ) -> Result<SelectedModelPackage, PackageEvidenceError> {
        let runtime = select_runtime_for_profile(LocalImagePlatformProfile::AppleM3Max128Gb)
            .map_err(|_| PackageEvidenceError::InvalidEvidence)?;
        if evidence.package_revision != QWEN_IMAGE_2512_PACKAGE_REVISION
            || evidence.runtime_revision != MLX_GEN_RUNTIME_REVISION
            || evidence.hardware_profile != QWEN_IMAGE_2512_HARDWARE_PROFILE
            || evidence.generation_profile != QWEN_IMAGE_2512_GENERATION_PROFILE
            || !is_lowercase_sha256(&evidence.generated_rgb_sha256)
            || !is_lowercase_sha256(&evidence.worker_sha256)
            || evidence.worker_byte_size == 0
            || evidence.worker_byte_size > MAX_WORKER_BYTES
            || !is_lowercase_sha256(&evidence.worker_bundle_sha256)
            || evidence.worker_bundle_byte_size == 0
            || evidence.worker_bundle_byte_size > MAX_WORKER_BUNDLE_BYTES
            || evidence.peak_memory_bytes == 0
            || evidence.cancellation_latency_ms == 0
            || evidence.cancellation_latency_ms > MAX_CANCELLATION_LATENCY_MS
            || !evidence.network_sandbox_proved
            || !evidence.visual_reviewed
        {
            return Err(PackageEvidenceError::InvalidEvidence);
        }
        let manifest = model_manifest(evidence.peak_memory_bytes);
        let sources = source_contracts();
        let source_plan = ModelSourcePlan::for_hugging_face(
            manifest.clone(),
            PACKAGE_ID,
            QWEN_IMAGE_2512_PACKAGE_REVISION,
            sources,
        )
        .map_err(|_| PackageEvidenceError::InvalidEvidence)?;
        if runtime.model_id() != manifest.model_id
            || runtime.model_revision() != manifest.source_revision
            || runtime.runtime_id() != manifest.runtime_id
        {
            return Err(PackageEvidenceError::InvalidEvidence);
        }
        Ok(SelectedModelPackage {
            runtime,
            manifest,
            source_plan,
            evidence,
        })
    }
}

/// Returns the one reviewed MLX-Gen package candidate without making it available for download.
pub(crate) fn qwen_image_2512_q4_candidate() -> ModelPackageCandidate {
    ModelPackageCandidate
}

/// Returns the exact package selected by the reviewed Apple M3 Max runtime proof.
pub(crate) fn selected_qwen_image_2512_q4_package()
-> Result<SelectedModelPackage, PackageEvidenceError> {
    qwen_image_2512_q4_candidate().accept(PackageAcceptanceEvidence {
        package_revision: QWEN_IMAGE_2512_PACKAGE_REVISION.into(),
        runtime_revision: MLX_GEN_RUNTIME_REVISION.into(),
        hardware_profile: QWEN_IMAGE_2512_HARDWARE_PROFILE.into(),
        generation_profile: QWEN_IMAGE_2512_GENERATION_PROFILE.into(),
        generated_rgb_sha256: ACCEPTED_GENERATED_RGB_SHA256.into(),
        worker_sha256: ACCEPTED_WORKER_SHA256.into(),
        worker_byte_size: ACCEPTED_WORKER_BYTES,
        worker_bundle_sha256: ACCEPTED_WORKER_BUNDLE_SHA256.into(),
        worker_bundle_byte_size: ACCEPTED_WORKER_BUNDLE_BYTES,
        peak_memory_bytes: ACCEPTED_PEAK_MEMORY_BYTES,
        cancellation_latency_ms: ACCEPTED_CANCELLATION_LATENCY_MS,
        network_sandbox_proved: true,
        visual_reviewed: true,
    })
}

/// Exact manifest and source plan produced only after native runtime evidence acceptance.
#[derive(Clone, Debug)]
pub(crate) struct SelectedModelPackage {
    runtime: LocalImageRuntimeCandidate,
    manifest: ModelPackageManifest,
    source_plan: ModelSourcePlan,
    evidence: PackageAcceptanceEvidence,
}

impl SelectedModelPackage {
    /// Returns the exact backend and worker identity selected for this package.
    pub(crate) fn runtime(&self) -> LocalImageRuntimeCandidate {
        self.runtime
    }

    /// Returns the accepted immutable manifest.
    pub(crate) fn manifest(&self) -> &ModelPackageManifest {
        &self.manifest
    }

    /// Returns the accepted Hugging Face source plan.
    pub(crate) fn source_plan(&self) -> &ModelSourcePlan {
        &self.source_plan
    }

    /// Returns the native evidence retained for later worker-byte re-verification.
    pub(crate) fn evidence(&self) -> &PackageAcceptanceEvidence {
        &self.evidence
    }
}

fn model_manifest(expected_memory_bytes: u64) -> ModelPackageManifest {
    let files = REVIEWED_FILES
        .iter()
        .map(|file| ModelFileContract {
            relative_path: file.relative_path.into(),
            byte_size: file.byte_size,
            sha256: file.sha256.into(),
        })
        .collect();
    ModelPackageManifest {
        model_id: MODEL_ID.into(),
        package_id: PACKAGE_ID.into(),
        runtime_id: format!("mlx-gen@{MLX_GEN_RUNTIME_REVISION}"),
        license: LICENSE.into(),
        source_revision: QWEN_IMAGE_2512_PACKAGE_REVISION.into(),
        expected_disk_bytes: EXPECTED_DISK_BYTES,
        expected_memory_bytes,
        files,
    }
}

fn source_contracts() -> Vec<ModelFileSource> {
    REVIEWED_FILES
        .iter()
        .map(|file| ModelFileSource {
            relative_path: file.relative_path.into(),
            strong_etag: file.strong_etag.into(),
        })
        .collect()
}

fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == SHA256_HEX_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
