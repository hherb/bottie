use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use url::Url;

use super::{
    InferenceProvider,
    openai::protocol::{
        DecodedStreamEvent, OpenAiToolCall, OpenAiToolCallAccumulator, OpenAiToolResult,
        decode_stream_payload,
    },
    provider::StreamSink,
    settings::{CONNECT_TIMEOUT, DISCOVERY_TIMEOUT, STREAM_IDLE_TIMEOUT, validate_local_base_url},
    sse::SseDecoder,
    types::{ChatRequest, ModelInfo, ProviderError, ProviderErrorCode, Usage},
};
use crate::tool_contract::enabled_native_tool_definitions;

use self::protocol::OmlxChatRequest;

mod discovery;
mod protocol;

#[cfg(test)]
use self::discovery::{
    decode_model_list, decode_model_status, decode_openapi_tool_support, enrich_models,
    enrich_tool_capabilities,
};

/// One provider-native oMLX history spanning repeated Bottie-owned tool rounds.
pub(crate) struct OmlxToolSession {
    request: OmlxChatRequest,
}

impl OmlxToolSession {
    /// Starts a session with exactly the closed tools enabled for this request.
    pub(crate) fn new(request: ChatRequest) -> Result<Self, ProviderError> {
        validate_request(&request)?;
        let definitions =
            enabled_native_tool_definitions(request.memory_enabled, request.web_enabled);
        Ok(Self {
            request: OmlxChatRequest::with_tools(request, definitions),
        })
    }

    /// Appends one complete assistant call batch and its exact correlated native results.
    pub(crate) fn append_results(
        &mut self,
        round: OmlxToolRound,
        results: Vec<OpenAiToolResult>,
    ) -> Result<(), ProviderError> {
        self.request
            .append_tool_exchange(round.reasoning, round.content, round.tool_calls, results)
    }
}

/// One complete streamed oMLX assistant round before optional native execution.
pub(crate) struct OmlxToolRound {
    /// Separate accumulated reasoning retained for the next provider request.
    pub(crate) reasoning: String,
    /// Accumulated assistant answer content retained for the next provider request.
    pub(crate) content: String,
    /// Ordered calls reconstructed from streamed OpenAI-shaped fragments.
    pub(crate) tool_calls: Vec<OpenAiToolCall>,
    /// Provider-reported usage for this request round.
    pub(crate) usage: Option<Usage>,
}

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

    /// Performs one timeout-bounded discovery GET request.
    async fn get(&self, path: &str) -> Result<Vec<u8>, ProviderError> {
        let response = self
            .client
            .get(self.endpoint(path)?)
            .timeout(DISCOVERY_TIMEOUT)
            .send()
            .await
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(response_error(response).await);
        }
        response
            .bytes()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(map_request_error)
    }

    /// Reads one fixed discovery resource without allowing an unbounded response allocation.
    async fn get_bounded(
        &self,
        path: &str,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, ProviderError> {
        let response = self
            .client
            .get(self.endpoint(path)?)
            .timeout(DISCOVERY_TIMEOUT)
            .send()
            .await
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(response_error(response).await);
        }
        if response
            .content_length()
            .is_some_and(|length| length > maximum_bytes as u64)
        {
            return Err(ProviderError::malformed(
                "oMLX returned invalid endpoint capability metadata.",
                Some("OpenAPI response exceeded its byte limit".into()),
            ));
        }
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(map_request_error)?;
            if bytes.len().saturating_add(chunk.len()) > maximum_bytes {
                return Err(ProviderError::malformed(
                    "oMLX returned invalid endpoint capability metadata.",
                    Some("OpenAPI response exceeded its byte limit".into()),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }

    /// Streams one tool-capable oMLX round without delegating execution to the endpoint.
    pub(crate) async fn stream_tool_round(
        &self,
        session: &OmlxToolSession,
        sink: impl StreamSink + Send + Sync,
    ) -> Result<OmlxToolRound, ProviderError> {
        self.stream_request(&session.request, sink, false).await
    }

    /// Streams one request while accumulating exact follow-up fields for a native tool exchange.
    async fn stream_request(
        &self,
        request: &OmlxChatRequest,
        sink: impl StreamSink + Send + Sync,
        emit_usage: bool,
    ) -> Result<OmlxToolRound, ProviderError> {
        let response = self
            .client
            .post(self.endpoint("v1/chat/completions")?)
            .json(request)
            .send()
            .await
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(response_error(response).await);
        }
        let mut bytes = response.bytes_stream();
        let mut decoder = SseDecoder::default();
        let mut calls = OpenAiToolCallAccumulator::default();
        let mut round = OmlxToolRound {
            reasoning: String::new(),
            content: String::new(),
            tool_calls: Vec::new(),
            usage: None,
        };
        while let Some(chunk) = bytes.next().await {
            let chunk = chunk.map_err(map_request_error)?;
            for payload in decoder.push(&chunk)? {
                if process_stream_event(
                    decode_omlx_stream_payload(&payload)?,
                    &sink,
                    &mut round,
                    &mut calls,
                    emit_usage,
                )? {
                    return Ok(round);
                }
            }
        }
        for payload in decoder.finish()? {
            if process_stream_event(
                decode_omlx_stream_payload(&payload)?,
                &sink,
                &mut round,
                &mut calls,
                emit_usage,
            )? {
                return Ok(round);
            }
        }
        Err(ProviderError::malformed(
            "oMLX ended the response before completion.",
            Some("SSE stream did not contain data: [DONE]".into()),
        ))
    }
}

impl InferenceProvider for OmlxProvider {
    /// Discovers oMLX models and enriches them with explicit VLM and residency metadata.
    async fn discover_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        discovery::discover_models(self).await
    }

    /// Streams one oMLX SSE chat response into the normalized sink.
    async fn stream_chat(
        &self,
        request: ChatRequest,
        sink: impl StreamSink + Send + Sync,
    ) -> Result<Option<Usage>, ProviderError> {
        validate_request(&request)?;
        self.stream_request(&OmlxChatRequest::from(request), sink, true)
            .await
            .map(|round| round.usage)
    }
}

/// Applies one shared OpenAI-shaped stream event while retaining oMLX-specific errors.
fn process_stream_event(
    event: DecodedStreamEvent,
    sink: &(impl StreamSink + Send + Sync),
    round: &mut OmlxToolRound,
    calls: &mut OpenAiToolCallAccumulator,
    emit_usage: bool,
) -> Result<bool, ProviderError> {
    if !event.reasoning_delta.is_empty() {
        round.reasoning.push_str(&event.reasoning_delta);
        sink.reasoning_delta(event.reasoning_delta)?;
    }
    if !event.text_delta.is_empty() {
        round.content.push_str(&event.text_delta);
        sink.text_delta(event.text_delta)?;
    }
    calls
        .extend(event.tool_call_deltas)
        .map_err(map_omlx_protocol_error)?;
    if let Some(usage) = event.usage {
        if emit_usage {
            sink.usage_updated(usage.clone())?;
        }
        round.usage = Some(usage);
    }
    if event.done {
        round.tool_calls = std::mem::take(calls)
            .finish()
            .map_err(map_omlx_protocol_error)?;
    }
    Ok(event.done)
}

/// Decodes an OpenAI-shaped oMLX event while keeping its provider identity in user-facing errors.
fn decode_omlx_stream_payload(payload: &str) -> Result<DecodedStreamEvent, ProviderError> {
    decode_stream_payload(payload).map_err(map_omlx_protocol_error)
}

/// Rewords shared protocol failures without exposing provider payload content.
fn map_omlx_protocol_error(error: ProviderError) -> ProviderError {
    ProviderError::malformed(
        "oMLX sent an invalid streaming response or native tool call.",
        error.diagnostic,
    )
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
struct OmlxErrorResponse {
    error: OmlxErrorBody,
}

#[derive(Deserialize)]
struct OmlxErrorBody {
    message: String,
}

#[cfg(test)]
mod tests;
