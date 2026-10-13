//! Embedded immutable source contracts for model acquisition, separate from runtime qualification.

use serde::Deserialize;

use super::ManagedInstallError;
use crate::local_image_worker::{
    model_acquisition::{ModelFileContract, ModelPackageManifest},
    model_download::{ModelFileSource, ModelSourcePlan},
    server::{IMAGE_MODEL_ID, IMAGE_PACKAGE_ID},
};

/// Upstream v26.10.2 source commit selected for the future managed macOS ARM64 runtime candidate.
/// This pin is a dependency binding, not accepted executable bytes or runtime-readiness evidence.
pub(crate) const RUNTIME_SOURCE_REVISION: &str = "ccedc6a4820f48e576633062232ed0cba1139616";
/// Exact quantized pack revision acquired by this first installer slice.
const MODEL_SOURCE_REVISION: &str = "fbda4caa0b4b1e17b5a29633e8deb600386f1eaf";
/// Conservative internal acquisition ceiling required by the generic manifest contract.
/// It is never disclosed as measured memory, nor used to qualify hardware or runtime readiness.
const ACQUISITION_MEMORY_CEILING_BYTES: u64 = 128 * 1_024 * 1_024 * 1_024;
const MODEL_LICENSE: &str = "LicenseRef-Qwen-Research";

/// Native source-only record; paths and validators never cross the WebView boundary.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PackageSourceRecord {
    package_id: String,
    source_revision: String,
    expected_disk_bytes: u64,
    files: Vec<FileSourceRecord>,
}

/// One reviewed immutable byte contract and its strong resumable HTTP validator.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FileSourceRecord {
    relative_path: String,
    byte_size: u64,
    sha256: String,
    strong_etag: String,
}

/// Validates the complete embedded file inventory against the closed preferred-model identity.
pub(super) fn source_plan() -> Result<ModelSourcePlan, ManagedInstallError> {
    let record: PackageSourceRecord = serde_json::from_str(include_str!("qwen21-source.json"))
        .map_err(|_| ManagedInstallError::InvalidPackage)?;
    if record.package_id != IMAGE_PACKAGE_ID || record.source_revision != MODEL_SOURCE_REVISION {
        return Err(ManagedInstallError::InvalidPackage);
    }
    let sources = record
        .files
        .iter()
        .map(|file| ModelFileSource {
            relative_path: file.relative_path.clone(),
            strong_etag: file.strong_etag.clone(),
        })
        .collect();
    let manifest = ModelPackageManifest {
        model_id: IMAGE_MODEL_ID.into(),
        package_id: record.package_id,
        runtime_id: format!("mlx-serve@{RUNTIME_SOURCE_REVISION}"),
        license: MODEL_LICENSE.into(),
        source_revision: record.source_revision,
        expected_disk_bytes: record.expected_disk_bytes,
        expected_memory_bytes: ACQUISITION_MEMORY_CEILING_BYTES,
        files: record
            .files
            .into_iter()
            .map(|file| ModelFileContract {
                relative_path: file.relative_path,
                byte_size: file.byte_size,
                sha256: file.sha256,
            })
            .collect(),
    };
    ModelSourcePlan::for_hugging_face(manifest, IMAGE_PACKAGE_ID, MODEL_SOURCE_REVISION, sources)
        .map_err(|_| ManagedInstallError::InvalidPackage)
}
