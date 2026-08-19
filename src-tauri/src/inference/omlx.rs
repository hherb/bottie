use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use url::Url;

use super::{
    InferenceProvider,
    provider::StreamSink,
    settings::{CONNECT_TIMEOUT, DISCOVERY_TIMEOUT, STREAM_IDLE_TIMEOUT, validate_local_base_url},
    types::{
        ChatRequest, ChatRole, ContentBlock, ModelInfo, ModelLoadState, ProviderCapabilities,
        ProviderError, ProviderErrorCode, ReasoningEffort, Usage,
    },
};

const PROVIDER_ID: &str = "omlx";
const PROVIDER_NAME: &str = "oMLX";
use super::settings::DEFAULT_OMLX_BASE_URL;

/// A Rust-owned adapter for the fixed, loopback oMLX endpoint.
#[derive(Clone)]
pub struct OmlxProvider {
    client: Client,
    base_url: Url,
}

impl OmlxProvider {
    /// Builds an oMLX adapter using the built-in loopback endpoint.
    pub fn new() -> Result<Self, ProviderError> {
        Self::with_base_url(DEFAULT_OMLX_BASE_URL)
    }

    /// Builds an oMLX adapter after validating a configurable loopback root.
    pub(crate) fn with_base_url(base_url: &str) -> Result<Self, ProviderError> {
        let base_url = validate_local_base_url(PROVIDER_NAME, base_url)?;

        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(STREAM_IDLE_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| {
                ProviderError::internal(
                    "Could not initialize local inference.",
                    Some(error.to_string()),
                )
            })?;

        Ok(Self { client, base_url })
    }

    /// Returns the normalized loopback root owned by this adapter.
    pub(crate) fn base_url(&self) -> &str {
        self.base_url.as_str()
    }

    /// Resolves one API path against the validated provider root.
    fn endpoint(&self, path: &str) -> Result<Url, ProviderError> {
        self.base_url.join(path).map_err(|error| {
            ProviderError::internal(
                "Could not construct the oMLX endpoint.",
                Some(error.to_string()),
            )
        })
    }
}

impl InferenceProvider for OmlxProvider {
    /// Discovers streaming text models through the oMLX model endpoint.
    async fn discover_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let response = self
            .client
            .get(self.endpoint("v1/models")?)
            .timeout(DISCOVERY_TIMEOUT)
            .send()
            .await
            .map_err(map_request_error)?;

        if !response.status().is_success() {
            return Err(response_error(response).await);
        }

        let bytes = response.bytes().await.map_err(map_request_error)?;
        decode_model_list(&bytes)
    }

    /// Streams one oMLX SSE chat response into the normalized sink.
    async fn stream_chat(
        &self,
        request: ChatRequest,
        sink: impl StreamSink + Send + Sync,
    ) -> Result<Option<Usage>, ProviderError> {
        validate_request(&request)?;
        let body = OmlxChatRequest::from(request);
        let response = self
            .client
            .post(self.endpoint("v1/chat/completions")?)
            .json(&body)
            .send()
            .await
            .map_err(map_request_error)?;

        if !response.status().is_success() {
            return Err(response_error(response).await);
        }

        let mut bytes = response.bytes_stream();
        let mut decoder = SseDecoder::default();
        let mut usage = None;
        let mut completed = false;

        while let Some(chunk) = bytes.next().await {
            let chunk = chunk.map_err(map_request_error)?;
            for payload in decoder.push(&chunk)? {
                match decode_stream_payload(&payload)? {
                    DecodedEvent::TextDelta(delta) if !delta.is_empty() => {
                        sink.text_delta(delta)?
                    }
                    DecodedEvent::ReasoningDelta(delta) if !delta.is_empty() => {
                        sink.reasoning_delta(delta)?
                    }
                    DecodedEvent::Usage(updated) => {
                        sink.usage_updated(updated.clone())?;
                        usage = Some(updated);
                    }
                    DecodedEvent::Done => completed = true,
                    DecodedEvent::TextDelta(_) | DecodedEvent::ReasoningDelta(_) => {}
                }
            }
        }

        for payload in decoder.finish()? {
            match decode_stream_payload(&payload)? {
                DecodedEvent::TextDelta(delta) if !delta.is_empty() => sink.text_delta(delta)?,
                DecodedEvent::ReasoningDelta(delta) if !delta.is_empty() => {
                    sink.reasoning_delta(delta)?
                }
                DecodedEvent::Usage(updated) => {
                    sink.usage_updated(updated.clone())?;
                    usage = Some(updated);
                }
                DecodedEvent::Done => completed = true,
                DecodedEvent::TextDelta(_) | DecodedEvent::ReasoningDelta(_) => {}
            }
        }

        if !completed {
            return Err(ProviderError::malformed(
                "oMLX ended the response before completion.",
                Some("SSE stream did not contain data: [DONE]".into()),
            ));
        }

        Ok(usage)
    }
}

/// Validates the provider-neutral request invariants required by oMLX.
fn validate_request(request: &ChatRequest) -> Result<(), ProviderError> {
    if request.model_id.trim().is_empty() {
        return Err(ProviderError::invalid_request(
            "Choose an oMLX model before sending.",
        ));
    }
    if request.messages.is_empty() {
        return Err(ProviderError::invalid_request(
            "A chat request needs at least one message.",
        ));
    }
    if request.messages.iter().any(|turn| turn.content.is_empty()) {
        return Err(ProviderError::invalid_request(
            "Chat messages cannot have empty content.",
        ));
    }
    Ok(())
}

/// Maps a native HTTP client failure into the provider-neutral error shape.
fn map_request_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError {
            code: ProviderErrorCode::Timeout,
            message: "oMLX took too long to respond.".into(),
            retryable: true,
            diagnostic: Some(error.to_string()),
        }
    } else if error.is_connect() {
        ProviderError::unavailable(
            "oMLX is offline. Check its configured loopback endpoint and try again.",
            Some(error.to_string()),
        )
    } else {
        ProviderError::unavailable(
            "The connection to oMLX was interrupted.",
            Some(error.to_string()),
        )
    }
}

/// Reads and normalizes a non-success oMLX HTTP response.
async fn response_error(response: reqwest::Response) -> ProviderError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    normalize_response_error(status, &body)
}

/// Normalizes an oMLX HTTP status and optional provider error body.
fn normalize_response_error(status: StatusCode, body: &str) -> ProviderError {
    let provider_message = serde_json::from_str::<OmlxErrorResponse>(&body)
        .ok()
        .map(|value| value.error.message)
        .filter(|message| !message.trim().is_empty());
    let message = provider_message.unwrap_or_else(|| match status {
        StatusCode::NOT_FOUND => "The oMLX API endpoint was not found.".into(),
        StatusCode::TOO_MANY_REQUESTS => "oMLX is busy. Try again shortly.".into(),
        _ if status.is_server_error() => "oMLX could not complete the request.".into(),
        _ => "oMLX rejected the request.".into(),
    });
    let diagnostic = Some(format!("HTTP {}", status.as_u16()));
    if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
        ProviderError::server(message, diagnostic)
    } else {
        let mut error = ProviderError::invalid_request(message);
        error.diagnostic = diagnostic;
        error
    }
}

#[derive(Deserialize)]
struct OmlxModelList {
    data: Vec<OmlxModel>,
}

#[derive(Deserialize)]
struct OmlxModel {
    id: String,
    max_model_len: Option<u64>,
}

/// Decodes and normalizes the oMLX model-list response.
fn decode_model_list(bytes: &[u8]) -> Result<Vec<ModelInfo>, ProviderError> {
    let response: OmlxModelList = serde_json::from_slice(bytes).map_err(|error| {
        ProviderError::malformed(
            "oMLX returned an invalid model list.",
            Some(error.to_string()),
        )
    })?;
    let models = response
        .data
        .into_iter()
        .filter(|model| !model.id.trim().is_empty())
        .map(|model| ModelInfo {
            provider_id: PROVIDER_ID.into(),
            provider_name: PROVIDER_NAME.into(),
            display_name: model.id.replace("--", "/"),
            model_id: model.id,
            max_context_tokens: model.max_model_len,
            load_state: ModelLoadState::Unknown,
            capabilities: ProviderCapabilities {
                text: true,
                streaming: true,
                ..ProviderCapabilities::default()
            },
        })
        .collect::<Vec<_>>();
    if models.is_empty() {
        return Err(ProviderError::unavailable(
            "oMLX is running but has no models available.",
            None,
        ));
    }
    Ok(models)
}

#[derive(Serialize)]
struct OmlxChatRequest {
    model: String,
    messages: Vec<OmlxChatTurn>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    chat_template_kwargs: OmlxChatTemplateSettings,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'static str>,
    stream_options: OmlxStreamOptions,
}

#[derive(Serialize)]
/// Template controls used to turn model thinking on or off explicitly.
struct OmlxChatTemplateSettings {
    enable_thinking: bool,
}

#[derive(Serialize)]
struct OmlxStreamOptions {
    include_usage: bool,
}

#[derive(Serialize)]
struct OmlxChatTurn {
    role: &'static str,
    content: String,
}

impl From<ChatRequest> for OmlxChatRequest {
    /// Converts a provider-neutral request into the oMLX wire shape.
    fn from(request: ChatRequest) -> Self {
        let settings = request.settings;
        let reasoning_enabled = settings.reasoning_effort == ReasoningEffort::Low;
        Self {
            model: request.model_id,
            messages: request
                .messages
                .into_iter()
                .map(|turn| OmlxChatTurn {
                    role: match turn.role {
                        ChatRole::System => "system",
                        ChatRole::User => "user",
                        ChatRole::Assistant => "assistant",
                    },
                    content: turn
                        .content
                        .into_iter()
                        .map(|block| match block {
                            ContentBlock::Text { text } => text,
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                })
                .collect(),
            stream: true,
            temperature: settings.temperature,
            max_tokens: settings.max_output_tokens,
            chat_template_kwargs: OmlxChatTemplateSettings {
                enable_thinking: reasoning_enabled,
            },
            reasoning_effort: reasoning_enabled.then_some("low"),
            stream_options: OmlxStreamOptions {
                include_usage: true,
            },
        }
    }
}

#[derive(Deserialize)]
struct OmlxStreamChunk {
    #[serde(default)]
    choices: Vec<OmlxChoice>,
    usage: Option<OmlxUsage>,
}

#[derive(Deserialize)]
struct OmlxChoice {
    delta: OmlxDelta,
}

#[derive(Default, Deserialize)]
struct OmlxDelta {
    content: Option<String>,
    reasoning_content: Option<String>,
}

#[derive(Deserialize)]
struct OmlxUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct OmlxErrorResponse {
    error: OmlxErrorBody,
}

#[derive(Deserialize)]
struct OmlxErrorBody {
    message: String,
}

#[derive(Debug)]
enum DecodedEvent {
    TextDelta(String),
    ReasoningDelta(String),
    Usage(Usage),
    Done,
}

/// Decodes one oMLX SSE data payload into a normalized event.
fn decode_stream_payload(payload: &str) -> Result<DecodedEvent, ProviderError> {
    if payload.trim() == "[DONE]" {
        return Ok(DecodedEvent::Done);
    }
    let chunk: OmlxStreamChunk = serde_json::from_str(payload).map_err(|error| {
        ProviderError::malformed(
            "oMLX sent a malformed stream event.",
            Some(error.to_string()),
        )
    })?;
    if let Some(usage) = chunk.usage {
        return Ok(DecodedEvent::Usage(Usage {
            input_tokens: usage.prompt_tokens,
            output_tokens: usage.completion_tokens,
        }));
    }
    let delta = chunk
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.delta)
        .unwrap_or_default();
    if let Some(reasoning) = delta.reasoning_content {
        return Ok(DecodedEvent::ReasoningDelta(reasoning));
    }
    Ok(DecodedEvent::TextDelta(delta.content.unwrap_or_default()))
}

#[derive(Default)]
struct SseDecoder {
    buffer: Vec<u8>,
}

impl SseDecoder {
    /// Appends bytes and returns every newly completed SSE data payload.
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, ProviderError> {
        self.buffer.extend_from_slice(bytes);
        self.drain(false)
    }

    /// Flushes a final unterminated SSE frame when the stream closes.
    fn finish(&mut self) -> Result<Vec<String>, ProviderError> {
        self.drain(true)
    }

    /// Drains complete SSE frames from the internal byte buffer.
    fn drain(&mut self, finish: bool) -> Result<Vec<String>, ProviderError> {
        let mut payloads = Vec::new();
        while let Some((index, separator_len)) = find_event_boundary(&self.buffer) {
            let frame = self.buffer.drain(..index).collect::<Vec<_>>();
            self.buffer.drain(..separator_len);
            if let Some(payload) = decode_sse_frame(&frame)? {
                payloads.push(payload);
            }
        }
        if finish && !self.buffer.is_empty() {
            let frame = std::mem::take(&mut self.buffer);
            if let Some(payload) = decode_sse_frame(&frame)? {
                payloads.push(payload);
            }
        }
        Ok(payloads)
    }
}

/// Finds the earliest LF or CRLF SSE event boundary.
fn find_event_boundary(bytes: &[u8]) -> Option<(usize, usize)> {
    let lf = bytes
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|i| (i, 2));
    let crlf = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|i| (i, 4));
    match (lf, crlf) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (Some(boundary), None) | (None, Some(boundary)) => Some(boundary),
        (None, None) => None,
    }
}

/// Extracts and joins the data fields from one UTF-8 SSE frame.
fn decode_sse_frame(frame: &[u8]) -> Result<Option<String>, ProviderError> {
    let text = std::str::from_utf8(frame).map_err(|error| {
        ProviderError::malformed(
            "oMLX sent invalid text in its stream.",
            Some(error.to_string()),
        )
    })?;
    let data = text
        .lines()
        .filter_map(|line| line.strip_prefix("data:").map(str::trim_start))
        .collect::<Vec<_>>();
    Ok((!data.is_empty()).then(|| data.join("\n")))
}

#[cfg(test)]
mod tests;
