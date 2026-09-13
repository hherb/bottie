//! Path-free generated-image types and bounded PNG normalization.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
#[cfg(test)]
use sha2::{Digest, Sha256};

use super::super::{
    StorageError, image_codec::normalize_image, image_normalization::NormalizedImageFormat,
};

/// Terminal or in-progress state for one requested generated image.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GeneratedAssetStatus {
    /// The provider or native downloader has not completed this output.
    Pending,
    /// Validated PNG bytes are retained in application-private storage.
    Completed,
    /// The user cancelled generation before durable bytes were committed.
    Cancelled,
    /// Provider, download, validation, or storage work failed.
    Failed,
}

impl GeneratedAssetStatus {
    /// Returns the stable SQLite representation.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    /// Parses one status constrained by the generated-assets schema.
    pub(super) fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "pending" => Ok(Self::Pending),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            "failed" => Ok(Self::Failed),
            _ => Err(StorageError::internal()),
        }
    }
}

/// Explicit backend class retained for every generated image.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GeneratedAssetExecution {
    /// A separately disclosed remote provider produced the temporary result.
    Cloud,
    /// A future native-owned local worker produced the result without cloud fallback.
    Local,
}

impl GeneratedAssetExecution {
    /// Returns the stable SQLite representation.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Cloud => "cloud",
            Self::Local => "local",
        }
    }

    /// Parses one execution class constrained by the generated-assets schema.
    pub(super) fn from_database(value: &str) -> Result<Self, StorageError> {
        match value {
            "cloud" => Ok(Self::Cloud),
            "local" => Ok(Self::Local),
            _ => Err(StorageError::internal()),
        }
    }
}

/// Exact provider provenance applied to all outputs in one image generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedImageProvenance {
    /// Stable Bottie provider identity.
    pub(crate) provider_id: String,
    /// Exact provider-owned model or checkpoint identity.
    pub(crate) model_id: String,
    /// Explicit local or cloud execution choice.
    pub(crate) execution: GeneratedAssetExecution,
    /// Deterministic seed when the selected backend supports one.
    pub(crate) seed: Option<i64>,
}

/// Native-only accepted request options retained for exact image retry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedImageRequestOptions {
    /// Requested output width in pixels.
    pub(crate) width: u32,
    /// Requested output height in pixels.
    pub(crate) height: u32,
    /// Whether the provider may enhance the durable user prompt.
    pub(crate) prompt_extend: bool,
}

impl GeneratedImageRequestOptions {
    /// Creates one non-zero request-option record for durable storage.
    pub(crate) fn new(width: u32, height: u32, prompt_extend: bool) -> Result<Self, StorageError> {
        if width == 0 || height == 0 {
            return Err(StorageError::invalid(
                "The generated image dimensions are invalid.",
            ));
        }
        Ok(Self {
            width,
            height,
            prompt_extend,
        })
    }
}

/// Native-only durable prompt, options, provenance, and pending message for one accepted run.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StartedGeneratedImage {
    /// Newly inserted pending assistant message.
    pub(crate) message: super::super::StoredMessage,
    /// Exact validated output count retained independently of presentation reconstruction.
    pub(crate) output_count: u8,
    /// Exact preceding durable user prompt to validate and send natively.
    pub(crate) prompt: String,
    /// Exact accepted dimensions and provider option.
    pub(crate) options: GeneratedImageRequestOptions,
    /// Exact provider provenance retained for all requested outputs.
    pub(crate) provenance: GeneratedImageProvenance,
}

impl GeneratedImageProvenance {
    /// Creates non-empty exact provenance before any durable message is inserted.
    pub(crate) fn new(
        provider_id: impl Into<String>,
        model_id: impl Into<String>,
        execution: GeneratedAssetExecution,
        seed: Option<i64>,
    ) -> Result<Self, StorageError> {
        let provider_id = provider_id.into().trim().to_owned();
        let model_id = model_id.into().trim().to_owned();
        if provider_id.is_empty() || model_id.is_empty() {
            return Err(StorageError::invalid(
                "Generated images require exact provider and model provenance.",
            ));
        }
        Ok(Self {
            provider_id,
            model_id,
            execution,
            seed,
        })
    }
}

/// Path-free generated-image metadata safe to cross the native IPC boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredGeneratedAsset {
    /// Opaque durable asset identity.
    pub(crate) id: String,
    /// Stable zero-based output order within the assistant message.
    pub(crate) ordinal: u8,
    /// Current durable lifecycle state.
    pub(crate) status: GeneratedAssetStatus,
    /// Completed media type, absent until valid bytes are committed.
    pub(crate) media_type: Option<String>,
    /// Completed decoded width, absent for non-completed states.
    pub(crate) width: Option<u32>,
    /// Completed decoded height, absent for non-completed states.
    pub(crate) height: Option<u32>,
    /// Completed exact retained byte count, absent for non-completed states.
    pub(crate) byte_size: Option<u64>,
    /// Stable Bottie provider identity.
    pub(crate) provider_id: String,
    /// Exact provider-owned model or checkpoint identity.
    pub(crate) model_id: String,
    /// Explicit execution backend with no fallback inference.
    pub(crate) execution: GeneratedAssetExecution,
    /// Deterministic seed when supported by the backend.
    pub(crate) seed: Option<i64>,
    /// Stable path- and provider-payload-redacted terminal failure category.
    pub(crate) error_code: Option<String>,
    /// Native creation time.
    pub(crate) created_at_ms: i64,
    /// Native-only content identity deliberately omitted from serialization.
    #[serde(skip_serializing)]
    pub(crate) sha256: Option<String>,
}

/// One validated PNG staged for an atomic generated-message completion.
#[derive(Clone, Debug)]
pub(crate) struct PreparedGeneratedImage {
    /// Native temporary file containing metadata-free PNG bytes.
    pub(crate) temporary_path: PathBuf,
    /// Decoded PNG width verified before storage.
    pub(crate) width: u32,
    /// Decoded PNG height verified before storage.
    pub(crate) height: u32,
    /// Exact staged byte count.
    pub(crate) byte_size: u64,
    /// Lowercase SHA-256 identity of the staged PNG.
    pub(crate) sha256: String,
}

impl PreparedGeneratedImage {
    /// Captures exact file identity after native PNG validation has completed.
    #[cfg(test)]
    pub(crate) fn from_validated_png(
        temporary_path: PathBuf,
        width: u32,
        height: u32,
    ) -> Result<Self, StorageError> {
        if width == 0 || height == 0 {
            return Err(StorageError::invalid(
                "The generated image dimensions are invalid.",
            ));
        }
        let bytes = fs::read(&temporary_path).map_err(|_| StorageError::generated_image())?;
        if bytes.is_empty() {
            return Err(StorageError::generated_image());
        }
        Ok(Self {
            temporary_path,
            width,
            height,
            byte_size: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        })
    }
}

impl Drop for PreparedGeneratedImage {
    /// Removes uncommitted normalized bytes when a generation fails or is cancelled.
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.temporary_path);
    }
}

/// Decodes one downloaded PNG under native limits and stages a metadata-free exact-size PNG.
pub(crate) fn normalize_generated_png(
    source_path: &Path,
    destination_path: PathBuf,
    expected_dimensions: (u32, u32),
) -> Result<PreparedGeneratedImage, StorageError> {
    let normalized =
        match normalize_image(source_path, &destination_path, NormalizedImageFormat::Png) {
            Ok(normalized) => normalized,
            Err(_) => {
                let _ = fs::remove_file(&destination_path);
                return Err(StorageError::invalid(
                    "The generated image could not be decoded safely.",
                ));
            }
        };
    if (normalized.width, normalized.height) != expected_dimensions {
        let _ = fs::remove_file(&destination_path);
        return Err(StorageError::invalid(
            "The generated image dimensions did not match the accepted request.",
        ));
    }
    Ok(PreparedGeneratedImage {
        temporary_path: destination_path,
        width: normalized.width,
        height: normalized.height,
        byte_size: normalized.byte_size,
        sha256: normalized.sha256,
    })
}
