//! Exact DashScope Qwen-Image-2.0 adapter.

use std::time::Duration;

use futures_util::StreamExt;
use reqwest::{
    Client, Response, StatusCode,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderValue},
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::inference::{ProviderError, ProviderErrorCode};

use super::{
    DASHSCOPE_QWEN_IMAGE_MODEL_ID, GeneratedImageReference, ImageGenerationCapabilities,
    ImageGenerationProvider, ImageGenerationRequest, MAX_IMAGE_OUTPUTS, MAX_IMAGE_PIXELS,
    QWEN_IMAGE_PROVIDER_ID, validate_qwen_image_base_url,
};

const GENERATION_PATH: &str = "services/aigc/multimodal-generation/generation";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const GENERATION_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_RESPONSE_BYTES: usize = 256 * 1_024;
const MAX_API_KEY_BYTES: usize = 512;

/// Authenticated native adapter for one exact hosted Qwen-Image-2.0 checkpoint.
#[derive(Clone)]
pub(crate) struct DashScopeQwenImageProvider {
    client: Client,
    endpoint: Url,
    authorization: HeaderValue,
}

impl DashScopeQwenImageProvider {
    /// Builds the adapter from a credential-free HTTPS workspace root and API key.
    pub(crate) fn new(base_url: &str, api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let base_url = validate_qwen_image_base_url(base_url)?;
        Self::build(base_url, api_key.into())
    }

    /// Builds an isolated adapter used only for pure protocol tests.
    #[cfg(test)]
    pub(super) fn for_fixture(base_url: &str, api_key: &str) -> Result<Self, ProviderError> {
        Self::new(base_url, api_key)
    }

    /// Creates an authenticated client without performing provider I/O.
    fn build(base_url: Url, api_key: String) -> Result<Self, ProviderError> {
        let endpoint = base_url.join(GENERATION_PATH).map_err(|_| {
            ProviderError::invalid_request("The Qwen Image API root could not be used safely.")
        })?;
        let api_key = api_key.trim();
        if api_key.is_empty() || api_key.len() > MAX_API_KEY_BYTES {
            return Err(ProviderError::invalid_request(
                "Add a valid Model Studio API key before using Qwen Image.",
            ));
        }
        let mut authorization =
            HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|_| {
                ProviderError::invalid_request("The Model Studio API key is malformed.")
            })?;
        authorization.set_sensitive(true);
        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(GENERATION_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| {
                ProviderError::internal("The Qwen Image HTTP client could not be created.", None)
            })?;
        Ok(Self {
            client,
            endpoint,
            authorization,
        })
    }

    /// Builds one authenticated generation request with no credential in its URL or body.
    fn request_builder(&self, request: &ImageGenerationRequest) -> reqwest::RequestBuilder {
        self.client
            .post(self.endpoint.clone())
            .header(ACCEPT, "application/json")
            .header(AUTHORIZATION, self.authorization.clone())
            .json(&DashScopeImageRequest::from(request))
            .timeout(GENERATION_TIMEOUT)
    }

    /// Serializes one request for exact fixture assertions without sending it.
    #[cfg(test)]
    pub(super) fn fixture_request_json(
        &self,
        request: &ImageGenerationRequest,
    ) -> Result<serde_json::Value, ProviderError> {
        serde_json::to_value(DashScopeImageRequest::from(request)).map_err(|_| {
            ProviderError::internal("The Qwen Image request could not be serialized.", None)
        })
    }

    /// Decodes a provider fixture without retaining provider correlation identifiers.
    #[cfg(test)]
    pub(super) fn fixture_decode_response(
        bytes: &[u8],
        expected_dimensions: (u32, u32),
        expected_count: u8,
    ) -> Result<Vec<GeneratedImageReference>, ProviderError> {
        decode_response(bytes, expected_dimensions, expected_count)
    }
}

impl ImageGenerationProvider for DashScopeQwenImageProvider {
    fn capabilities(&self) -> ImageGenerationCapabilities {
        ImageGenerationCapabilities {
            provider_id: QWEN_IMAGE_PROVIDER_ID,
            model_id: DASHSCOPE_QWEN_IMAGE_MODEL_ID,
            generation: true,
            editing: true,
            max_outputs: MAX_IMAGE_OUTPUTS,
            max_pixels: MAX_IMAGE_PIXELS,
            execution: "cloud",
        }
    }

    async fn generate(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<Vec<GeneratedImageReference>, ProviderError> {
        let expected_dimensions = request.dimensions();
        let expected_count = request.count();
        let response = self
            .request_builder(&request)
            .send()
            .await
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(map_status(response.status()));
        }
        validate_content_type(&response)?;
        let bytes = read_bounded_body(response).await?;
        decode_response(&bytes, expected_dimensions, expected_count)
    }
}

#[derive(Serialize)]
struct DashScopeImageRequest<'a> {
    model: &'static str,
    input: DashScopeInput<'a>,
    parameters: DashScopeParameters,
}

#[derive(Serialize)]
struct DashScopeInput<'a> {
    messages: [DashScopeMessage<'a>; 1],
}

#[derive(Serialize)]
struct DashScopeMessage<'a> {
    role: &'static str,
    content: [DashScopeText<'a>; 1],
}

#[derive(Serialize)]
struct DashScopeText<'a> {
    text: &'a str,
}

#[derive(Serialize)]
struct DashScopeParameters {
    size: String,
    n: u8,
    prompt_extend: bool,
    watermark: bool,
}

impl<'a> From<&'a ImageGenerationRequest> for DashScopeImageRequest<'a> {
    fn from(request: &'a ImageGenerationRequest) -> Self {
        let (width, height) = request.dimensions();
        Self {
            model: DASHSCOPE_QWEN_IMAGE_MODEL_ID,
            input: DashScopeInput {
                messages: [DashScopeMessage {
                    role: "user",
                    content: [DashScopeText {
                        text: request.prompt(),
                    }],
                }],
            },
            parameters: DashScopeParameters {
                size: format!("{width}*{height}"),
                n: request.count(),
                prompt_extend: request.prompt_extend(),
                watermark: false,
            },
        }
    }
}

#[derive(Deserialize)]
struct DashScopeImageResponse {
    output: DashScopeOutput,
}

#[derive(Deserialize)]
struct DashScopeOutput {
    choices: Vec<DashScopeChoice>,
}

#[derive(Deserialize)]
struct DashScopeChoice {
    finish_reason: String,
    message: DashScopeResponseMessage,
}

#[derive(Deserialize)]
struct DashScopeResponseMessage {
    role: String,
    content: Vec<DashScopeResponseContent>,
}

#[derive(Deserialize)]
struct DashScopeResponseContent {
    image: Option<String>,
}

/// Accepts only the exact requested count of bounded HTTPS image references.
fn decode_response(
    bytes: &[u8],
    expected_dimensions: (u32, u32),
    expected_count: u8,
) -> Result<Vec<GeneratedImageReference>, ProviderError> {
    let decoded: DashScopeImageResponse = serde_json::from_slice(bytes)
        .map_err(|_| malformed_response("invalid JSON or missing required output metadata"))?;
    if decoded
        .output
        .choices
        .iter()
        .any(|choice| choice.finish_reason != "stop" || choice.message.role != "assistant")
    {
        return Err(malformed_response("incomplete output choice"));
    }
    let urls = decoded
        .output
        .choices
        .into_iter()
        .flat_map(|choice| choice.message.content)
        .filter_map(|content| content.image)
        .collect::<Vec<_>>();
    if urls.len() != usize::from(expected_count) {
        return Err(malformed_response("unexpected image count"));
    }
    urls.into_iter()
        .map(|value| {
            let url = Url::parse(&value).map_err(|_| malformed_response("invalid image URL"))?;
            if url.scheme() != "https"
                || url.host().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(malformed_response("unsafe image URL"));
            }
            Ok(GeneratedImageReference {
                url,
                width: expected_dimensions.0,
                height: expected_dimensions.1,
            })
        })
        .collect()
}

/// Requires JSON before interpreting any provider-controlled response bytes.
fn validate_content_type(response: &Response) -> Result<(), ProviderError> {
    let is_json = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"));
    if is_json {
        Ok(())
    } else {
        Err(malformed_response("unexpected content type"))
    }
}

/// Reads a small response envelope without allowing provider-controlled allocation growth.
async fn read_bounded_body(response: Response) -> Result<Vec<u8>, ProviderError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(malformed_response("response exceeds byte limit"));
    }
    let mut bytes = Vec::new();
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(map_request_error)?;
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(malformed_response("response exceeds byte limit"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Maps request transport failures without exposing request headers or provider content.
fn map_request_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError {
            code: ProviderErrorCode::Timeout,
            message: "Qwen Image took too long to respond.".into(),
            retryable: true,
            diagnostic: Some("image generation request timed out".into()),
        }
    } else {
        ProviderError::unavailable(
            "Qwen Image could not be reached.",
            Some("image generation transport failed".into()),
        )
    }
}

/// Maps HTTP status without reading or reflecting a provider-controlled response body.
fn map_status(status: StatusCode) -> ProviderError {
    let diagnostic = Some(format!("HTTP {}", status.as_u16()));
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            let mut error = ProviderError::invalid_request(
                "The Model Studio API key or workspace endpoint was rejected.",
            );
            error.diagnostic = diagnostic;
            error
        }
        StatusCode::TOO_MANY_REQUESTS => {
            ProviderError::server("Qwen Image is rate limited.", diagnostic)
        }
        _ if status.is_server_error() => {
            ProviderError::server("Qwen Image could not complete the request.", diagnostic)
        }
        _ => {
            let mut error =
                ProviderError::invalid_request("Qwen Image rejected the generation request.");
            error.diagnostic = diagnostic;
            error
        }
    }
}

/// Builds one fixed malformed-response error without provider payload content.
fn malformed_response(reason: &str) -> ProviderError {
    ProviderError::malformed(
        "Qwen Image returned an invalid response.",
        Some(reason.into()),
    )
}
