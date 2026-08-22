//! Native Anthropic-compatible model discovery and Messages API streaming.

use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use url::Url;

use super::{
    InferenceProvider, StreamSink,
    settings::{CONNECT_TIMEOUT, DISCOVERY_TIMEOUT, STREAM_IDLE_TIMEOUT, validate_remote_base_url},
    sse::SseDecoder,
    types::{
        ChatRequest, ModelInfo, ModelLoadState, ProviderCapabilities, ProviderError,
        ProviderErrorCode, Usage,
    },
};
use crate::tool_contract::{memory_tool_definitions, web_search_tool_definition};

use self::protocol::{
    AnthropicChatRequest, AnthropicResponseAccumulator, AnthropicToolRound, DecodedEvent,
    decode_stream_payload,
};

mod protocol;

pub(crate) use protocol::{AnthropicToolCall, AnthropicToolResult};

const PROVIDER_ID: &str = "anthropic";
const PROVIDER_NAME: &str = "Anthropic-compatible";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// One provider-native Messages history spanning repeated memory-tool rounds.
pub(crate) struct AnthropicToolSession {
    request: AnthropicChatRequest,
}

impl AnthropicToolSession {
    /// Starts a session with exactly the closed native tools enabled for this request.
    pub(crate) fn new(request: ChatRequest) -> Result<Self, ProviderError> {
        validate_request(&request)?;
        let mut definitions = request
            .memory_enabled
            .then(memory_tool_definitions)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        if request.web_enabled {
            definitions.push(web_search_tool_definition());
        }
        Ok(Self {
            request: AnthropicChatRequest::with_tools(request, definitions),
        })
    }

    /// Appends one accumulated assistant block sequence and its exact correlated native results.
    pub(crate) fn append_results(
        &mut self,
        round: AnthropicToolRound,
        results: Vec<AnthropicToolResult>,
    ) -> Result<(), ProviderError> {
        self.request.append_tool_exchange(round, results)
    }
}

/// Rust-owned adapter for Anthropic's Messages protocol.
#[derive(Clone)]
pub struct AnthropicProvider {
    client: Client,
    base_url: Url,
    api_key: String,
}

impl AnthropicProvider {
    /// Builds an authenticated adapter for an HTTPS Anthropic-compatible API root.
    pub(crate) fn new(base_url: &str, api_key: String) -> Result<Self, ProviderError> {
        let base_url = validate_remote_base_url(PROVIDER_NAME, base_url)?;
        if api_key.trim().is_empty() {
            return Err(ProviderError::invalid_request(
                "Add an Anthropic-compatible API key in Settings.",
            ));
        }
        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(STREAM_IDLE_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| {
                ProviderError::internal(
                    "Could not initialize Anthropic-compatible inference.",
                    None,
                )
            })?;
        Ok(Self {
            client,
            base_url,
            api_key,
        })
    }

    #[cfg(test)]
    /// Builds a test-only adapter for an isolated loopback HTTP fixture.
    pub(crate) fn for_loopback_fixture(base_url: &str) -> Result<Self, ProviderError> {
        let base_url = Url::parse(base_url).map_err(|_| {
            ProviderError::internal("Could not construct the Anthropic test endpoint.", None)
        })?;
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| ProviderError::internal("Could not initialize Anthropic tests.", None))?;
        Ok(Self {
            client,
            base_url,
            api_key: "fixture-secret".into(),
        })
    }

    /// Returns the normalized HTTPS API root without credential material.
    pub(crate) fn base_url(&self) -> &str {
        self.base_url.as_str()
    }

    fn endpoint(&self, path: &str) -> Result<Url, ProviderError> {
        self.base_url.join(path).map_err(|_| {
            ProviderError::internal(
                "Could not construct the Anthropic-compatible endpoint.",
                None,
            )
        })
    }

    fn authenticated(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        request
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
    }

    /// Streams one tool-capable Messages round without exposing provider JSON.
    pub(crate) async fn stream_tool_round(
        &self,
        session: &AnthropicToolSession,
        sink: impl StreamSink + Send + Sync,
    ) -> Result<AnthropicToolRound, ProviderError> {
        self.stream_request(&session.request, sink, false).await
    }

    /// Streams one concrete request while reconstructing blocks required by a tool follow-up.
    async fn stream_request(
        &self,
        request: &AnthropicChatRequest,
        sink: impl StreamSink + Send + Sync,
        emit_usage: bool,
    ) -> Result<AnthropicToolRound, ProviderError> {
        let request = self.client.post(self.endpoint("messages")?).json(request);
        let response = self
            .authenticated(request)
            .send()
            .await
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(response_error(response).await);
        }

        let mut bytes = response.bytes_stream();
        let mut decoder = SseDecoder::default();
        let mut round = AnthropicResponseAccumulator::default();
        while let Some(chunk) = bytes.next().await {
            let chunk = chunk.map_err(map_request_error)?;
            for payload in decoder.push(&chunk)? {
                if process_stream_event(
                    decode_stream_payload(&payload)?,
                    &sink,
                    &mut round,
                    emit_usage,
                )? {
                    return round.finish();
                }
            }
        }
        for payload in decoder.finish()? {
            if process_stream_event(
                decode_stream_payload(&payload)?,
                &sink,
                &mut round,
                emit_usage,
            )? {
                return round.finish();
            }
        }
        Err(ProviderError::malformed(
            "The Anthropic-compatible response ended before completion.",
            Some("SSE stream did not contain a message_stop event".into()),
        ))
    }
}

impl InferenceProvider for AnthropicProvider {
    async fn discover_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let request = self
            .client
            .get(self.endpoint("models")?)
            .timeout(DISCOVERY_TIMEOUT);
        let response = self
            .authenticated(request)
            .send()
            .await
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(response_error(response).await);
        }
        decode_model_list(&response.bytes().await.map_err(map_request_error)?)
    }

    async fn stream_chat(
        &self,
        request: ChatRequest,
        sink: impl StreamSink + Send + Sync,
    ) -> Result<Option<Usage>, ProviderError> {
        validate_request(&request)?;
        self.stream_request(&AnthropicChatRequest::from(request), sink, true)
            .await
            .map(|round| round.usage)
    }
}

/// Applies one decoded stream event and emits only normalized visible deltas and usage.
fn process_stream_event(
    event: DecodedEvent,
    sink: &(impl StreamSink + Send + Sync),
    round: &mut AnthropicResponseAccumulator,
    emit_usage: bool,
) -> Result<bool, ProviderError> {
    if let Some(delta) = event.text_delta().filter(|delta| !delta.is_empty()) {
        sink.text_delta(delta.into())?;
    }
    if let Some(delta) = event.reasoning_delta().filter(|delta| !delta.is_empty()) {
        sink.reasoning_delta(delta.into())?;
    }
    let usage_changed = event.has_usage();
    round.apply(event)?;
    if emit_usage
        && usage_changed
        && let Some(usage) = round.usage()
    {
        sink.usage_updated(usage)?;
    }
    Ok(round.is_complete())
}

fn validate_request(request: &ChatRequest) -> Result<(), ProviderError> {
    if request.model_id.trim().is_empty() || request.messages.is_empty() {
        return Err(ProviderError::invalid_request(
            "Choose an Anthropic-compatible model and include at least one message.",
        ));
    }
    Ok(())
}

fn map_request_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError {
            code: ProviderErrorCode::Timeout,
            message: "The Anthropic-compatible provider took too long to respond.".into(),
            retryable: true,
            diagnostic: Some(error.to_string()),
        }
    } else {
        ProviderError::unavailable(
            "The Anthropic-compatible provider could not be reached.",
            Some(error.to_string()),
        )
    }
}

async fn response_error(response: reqwest::Response) -> ProviderError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    normalize_response_error(status, &body)
}

fn normalize_response_error(status: StatusCode, body: &str) -> ProviderError {
    let provider_message = serde_json::from_str::<AnthropicErrorResponse>(body)
        .ok()
        .map(|response| response.error.message)
        .filter(|message| !message.trim().is_empty());
    let message = provider_message.unwrap_or_else(|| match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            "The Anthropic-compatible API key was rejected.".into()
        }
        StatusCode::TOO_MANY_REQUESTS => {
            "The Anthropic-compatible provider is rate limited.".into()
        }
        _ if status.is_server_error() => {
            "The Anthropic-compatible provider could not complete the request.".into()
        }
        _ => "The Anthropic-compatible provider rejected the request.".into(),
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
struct ModelList {
    data: Vec<ModelRecord>,
}

#[derive(Deserialize)]
struct ModelRecord {
    id: String,
    display_name: Option<String>,
    #[serde(default)]
    capabilities: Vec<String>,
}

fn decode_model_list(bytes: &[u8]) -> Result<Vec<ModelInfo>, ProviderError> {
    let response: ModelList = serde_json::from_slice(bytes).map_err(|error| {
        ProviderError::malformed(
            "The Anthropic-compatible provider returned an invalid model list.",
            Some(error.to_string()),
        )
    })?;
    let models = response
        .data
        .into_iter()
        .filter(|model| !model.id.trim().is_empty())
        .map(|model| {
            let vision = model
                .capabilities
                .iter()
                .any(|capability| capability == "vision");
            ModelInfo {
                provider_id: PROVIDER_ID.into(),
                provider_name: PROVIDER_NAME.into(),
                display_name: model.display_name.unwrap_or_else(|| model.id.clone()),
                model_id: model.id,
                max_context_tokens: None,
                load_state: ModelLoadState::Unknown,
                capabilities: ProviderCapabilities {
                    text: true,
                    streaming: true,
                    vision,
                    tools: model
                        .capabilities
                        .iter()
                        .any(|capability| capability == "tools"),
                    ..Default::default()
                },
            }
        })
        .collect::<Vec<_>>();
    if models.is_empty() {
        Err(ProviderError::unavailable(
            "The Anthropic-compatible provider reported no models.",
            None,
        ))
    } else {
        Ok(models)
    }
}

#[derive(Deserialize)]
struct AnthropicErrorResponse {
    error: AnthropicError,
}

#[derive(Deserialize)]
struct AnthropicError {
    message: String,
}

#[cfg(test)]
mod tests;
