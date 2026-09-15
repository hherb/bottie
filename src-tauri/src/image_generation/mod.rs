//! Provider-neutral image-generation contracts and concrete native adapters.

mod controller;
mod dashscope;
mod download;
mod local;
mod runs;

#[cfg(test)]
mod editing_controller_tests;
#[cfg(test)]
mod editing_tests;
#[cfg(test)]
mod live_tests;
#[cfg(test)]
mod tests;

use crate::inference::ProviderError;
use crate::local_image_worker::execution::{
    LOCAL_GENERATION_DIMENSIONS, LOCAL_GENERATION_OUTPUT_COUNT,
};
use crate::storage::{
    GeneratedAssetExecution, GeneratedImageProvenance, ValidatedGeneratedImageSource,
};
use url::Url;

pub(crate) use controller::{
    cancel_image_generation, retry_image_generation, start_image_editing, start_image_generation,
};
pub(crate) use dashscope::DashScopeQwenImageProvider;
pub(crate) use download::GeneratedImageDownloader;
pub(crate) use local::LocalImageGenerator;
pub(crate) use runs::ImageGenerationRuns;

/// Stable Bottie provider identity for the hosted Qwen Image route.
pub(crate) const QWEN_IMAGE_PROVIDER_ID: &str = "qwen-image";
/// Pinned exact Qwen-Image-2.0 snapshot used instead of the mutable provider alias.
pub(crate) const DASHSCOPE_QWEN_IMAGE_MODEL_ID: &str = "qwen-image-2.0-2026-03-03";
/// Maximum image count documented for one Qwen-Image-2.0 request.
pub(crate) const MAX_IMAGE_OUTPUTS: u8 = 6;
/// Smallest total pixel area accepted by the hosted Qwen-Image-2.0 API.
const MIN_IMAGE_PIXELS: u64 = 512 * 512;
/// Largest total pixel area accepted by the hosted Qwen-Image-2.0 API.
pub(crate) const MAX_IMAGE_PIXELS: u64 = 2_048 * 2_048;
/// Conservative per-axis guard that still admits every documented recommended aspect ratio.
const MAX_IMAGE_AXIS: u32 = 4_096;
/// Conservative native character guard ahead of the provider's 1,000-token instruction limit.
const MAX_IMAGE_PROMPT_CHARACTERS: usize = 1_000;
/// Minimum number of reference images accepted by one hosted editing request.
#[cfg_attr(not(test), allow(dead_code))]
const MIN_IMAGE_EDIT_SOURCES: usize = 1;
/// Maximum number of reference images accepted by exact hosted Qwen-Image-2.0.
#[cfg_attr(not(test), allow(dead_code))]
const MAX_IMAGE_EDIT_SOURCES: usize = 3;

/// Accepts only official DashScope or Model Studio API roots for the exact adapter.
pub(crate) fn validate_qwen_image_base_url(candidate: &str) -> Result<Url, ProviderError> {
    let url = crate::inference::validate_remote_base_url("Qwen Image", candidate)?;
    let host = url.host_str().unwrap_or_default();
    let is_legacy_host = matches!(
        host,
        "dashscope-intl.aliyuncs.com" | "dashscope.aliyuncs.com"
    );
    let is_workspace_host = [
        ".ap-southeast-1.maas.aliyuncs.com",
        ".cn-beijing.maas.aliyuncs.com",
    ]
    .into_iter()
    .any(|suffix| host.ends_with(suffix) && host.len() > suffix.len());
    if (!is_legacy_host && !is_workspace_host) || url.path() != "/api/v1/" {
        return Err(ProviderError::invalid_request(
            "Use an official Qwen Image DashScope or Model Studio workspace API root ending in /api/v1/.",
        ));
    }
    Ok(url)
}

/// Exact capabilities exposed without performing or billing a provider request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ImageGenerationCapabilities {
    /// Stable Bottie provider identity.
    pub(crate) provider_id: &'static str,
    /// Exact provider-owned checkpoint identity.
    pub(crate) model_id: &'static str,
    /// Whether text-to-image generation is implemented.
    pub(crate) generation: bool,
    /// Whether instruction-based image editing is implemented by the model family.
    pub(crate) editing: bool,
    /// Maximum output count accepted in one request.
    pub(crate) max_outputs: u8,
    /// Maximum total output pixel area accepted by the provider.
    pub(crate) max_pixels: u64,
    /// Whether this concrete adapter executes locally or through a cloud service.
    pub(crate) execution: &'static str,
}

/// One validated provider-neutral text-to-image request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ImageGenerationRequest {
    prompt: String,
    width: u32,
    height: u32,
    count: u8,
    prompt_extend: bool,
}

impl ImageGenerationRequest {
    /// Validates a bounded request while preserving meaningful prompt formatting.
    pub(crate) fn new(
        prompt: impl Into<String>,
        width: u32,
        height: u32,
        count: u8,
    ) -> Result<Self, ProviderError> {
        Self::new_with_prompt_extension(prompt, width, height, count, true)
    }

    /// Validates a local request without allowing provider-side prompt enhancement.
    pub(crate) fn new_local(
        prompt: impl Into<String>,
        width: u32,
        height: u32,
        count: u8,
    ) -> Result<Self, ProviderError> {
        if (width, height) != LOCAL_GENERATION_DIMENSIONS || count != LOCAL_GENERATION_OUTPUT_COUNT
        {
            return Err(ProviderError::invalid_request(
                "The verified local image route currently supports one 512x512 image.",
            ));
        }
        Self::new_with_prompt_extension(prompt, width, height, count, false)
    }

    fn new_with_prompt_extension(
        prompt: impl Into<String>,
        width: u32,
        height: u32,
        count: u8,
        prompt_extend: bool,
    ) -> Result<Self, ProviderError> {
        let prompt = prompt.into().replace("\r\n", "\n");
        let prompt = prompt.trim().to_owned();
        if prompt.is_empty() {
            return Err(ProviderError::invalid_request(
                "Enter a non-empty image description.",
            ));
        }
        if prompt.chars().count() > MAX_IMAGE_PROMPT_CHARACTERS
            || prompt
                .chars()
                .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
        {
            return Err(ProviderError::invalid_request(
                "The image description is too long or contains unsupported control characters.",
            ));
        }
        let pixels = u64::from(width).saturating_mul(u64::from(height));
        if width == 0
            || height == 0
            || width > MAX_IMAGE_AXIS
            || height > MAX_IMAGE_AXIS
            || width % 16 != 0
            || height % 16 != 0
            || !(MIN_IMAGE_PIXELS..=MAX_IMAGE_PIXELS).contains(&pixels)
        {
            return Err(ProviderError::invalid_request(concat!(
                "Choose image dimensions divisible by 16 with a total area between ",
                "512x512 and 2048x2048 pixels."
            )));
        }
        if !(1..=MAX_IMAGE_OUTPUTS).contains(&count) {
            return Err(ProviderError::invalid_request(format!(
                "Generate between 1 and {MAX_IMAGE_OUTPUTS} images at a time."
            )));
        }
        Ok(Self {
            prompt,
            width,
            height,
            count,
            prompt_extend,
        })
    }

    /// Returns the native-only prompt sent to the selected image provider.
    pub(crate) fn prompt(&self) -> &str {
        &self.prompt
    }

    /// Returns the requested output dimensions.
    pub(crate) fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Returns the bounded number of requested outputs.
    pub(crate) fn count(&self) -> u8 {
        self.count
    }

    /// Returns whether the provider may enhance the supplied prompt.
    pub(crate) fn prompt_extend(&self) -> bool {
        self.prompt_extend
    }
}

/// One provider-neutral hosted edit request containing only Rust-owned validated source bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct ImageEditingRequest {
    generation: ImageGenerationRequest,
    sources: Vec<ValidatedGeneratedImageSource>,
    provenance: GeneratedImageProvenance,
}

#[cfg_attr(not(test), allow(dead_code))]
impl ImageEditingRequest {
    /// Validates bounded output options and pins editing to exact hosted Qwen-Image-2.0 provenance.
    pub(crate) fn new(
        prompt: impl Into<String>,
        width: u32,
        height: u32,
        count: u8,
        sources: Vec<ValidatedGeneratedImageSource>,
    ) -> Result<Self, ProviderError> {
        if !(MIN_IMAGE_EDIT_SOURCES..=MAX_IMAGE_EDIT_SOURCES).contains(&sources.len()) {
            return Err(ProviderError::invalid_request(
                "Choose between one and three validated image editing sources.",
            ));
        }
        let generation = ImageGenerationRequest::new(prompt, width, height, count)?;
        let provenance = GeneratedImageProvenance::new(
            QWEN_IMAGE_PROVIDER_ID,
            DASHSCOPE_QWEN_IMAGE_MODEL_ID,
            GeneratedAssetExecution::Cloud,
            None,
        )
        .map_err(|_| {
            ProviderError::internal(
                "The hosted image editing provenance contract is invalid.",
                None,
            )
        })?;
        Ok(Self {
            generation,
            sources,
            provenance,
        })
    }

    /// Returns the exact durable user editing instruction.
    pub(crate) fn prompt(&self) -> &str {
        self.generation.prompt()
    }

    /// Returns the requested output dimensions.
    pub(crate) fn dimensions(&self) -> (u32, u32) {
        self.generation.dimensions()
    }

    /// Returns the bounded requested output count.
    pub(crate) fn count(&self) -> u8 {
        self.generation.count()
    }

    /// Returns whether exact hosted prompt enhancement is enabled.
    pub(crate) fn prompt_extend(&self) -> bool {
        self.generation.prompt_extend()
    }

    /// Returns exact ordered native source bytes for a provider-specific serializer.
    pub(crate) fn sources(&self) -> &[ValidatedGeneratedImageSource] {
        &self.sources
    }

    /// Returns exact hosted provider/model/backend provenance for durable storage.
    pub(crate) fn provenance(&self) -> &GeneratedImageProvenance {
        &self.provenance
    }
}

/// One provider-returned image location retained only behind the native boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedImageReference {
    url: url::Url,
    width: u32,
    height: u32,
}

impl GeneratedImageReference {
    /// Returns the temporary HTTPS result location for native download only.
    pub(crate) fn url(&self) -> &str {
        self.url.as_str()
    }

    /// Returns requested dimensions for later native download validation.
    pub(crate) fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// Provider-neutral image generation implemented by hosted and future local adapters.
pub(crate) trait ImageGenerationProvider: Clone + Send + Sync + 'static {
    /// Returns fixed capabilities without provider I/O.
    fn capabilities(&self) -> ImageGenerationCapabilities;

    /// Generates bounded image references without exposing provider JSON.
    async fn generate(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<Vec<GeneratedImageReference>, ProviderError>;
}

/// Provider-neutral hosted image editing over exact Rust-owned source bytes.
pub(crate) trait ImageEditingProvider: Clone + Send + Sync + 'static {
    /// Edits one bounded request into temporary native-only image references.
    async fn edit(
        &self,
        request: ImageEditingRequest,
    ) -> Result<Vec<GeneratedImageReference>, ProviderError>;
}
