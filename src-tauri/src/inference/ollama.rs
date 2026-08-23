use futures_util::{StreamExt, stream};
use reqwest::{Client, StatusCode};
use url::Url;

use super::{
    InferenceProvider,
    provider::StreamSink,
    settings::{CONNECT_TIMEOUT, DISCOVERY_TIMEOUT, STREAM_IDLE_TIMEOUT, validate_local_base_url},
    types::{ChatRequest, ModelInfo, ProviderError, ProviderErrorCode, Usage},
};

use self::protocol::{
    NdjsonDecoder, OllamaChatRequest, OllamaErrorResponse, OllamaShowRequest, OllamaShowResponse,
    decode_model_list, decode_running_models, decode_stream_line, model_info, normalize_usage,
};
use crate::tool_contract::enabled_native_tool_definitions;

mod protocol;

pub(crate) use protocol::{OllamaToolCall, OllamaToolResult};

/// One provider-native Ollama request history spanning repeated native-tool rounds.
pub(crate) struct OllamaToolSession {
    request: OllamaChatRequest,
}

impl OllamaToolSession {
    /// Starts a session with exactly the closed native tools enabled for this request.
    pub(crate) fn new(request: ChatRequest) -> Result<Self, ProviderError> {
        validate_request(&request)?;
        let definitions =
            enabled_native_tool_definitions(request.memory_enabled, request.web_enabled);
        Ok(Self {
            request: OllamaChatRequest::with_tools(request, definitions),
        })
    }

    /// Appends one accumulated assistant call batch and the exact ordered native results.
    pub(crate) fn append_results(
        &mut self,
        round: OllamaToolRound,
        results: Vec<OllamaToolResult>,
    ) -> Result<(), ProviderError> {
        self.request
            .append_tool_exchange(round.thinking, round.content, round.tool_calls, results)
    }
}

/// One complete streamed Ollama assistant round before optional native tool execution.
pub(crate) struct OllamaToolRound {
    /// Accumulated assistant reasoning required in the next provider request.
    pub(crate) thinking: String,
    /// Accumulated assistant answer text required in the next provider request.
    pub(crate) content: String,
    /// Ordered complete function calls accumulated across stream chunks.
    pub(crate) tool_calls: Vec<OllamaToolCall>,
    /// Provider-reported usage for this chat request.
    pub(crate) usage: Option<Usage>,
}

const PROVIDER_ID: &str = "ollama";
const PROVIDER_NAME: &str = "Ollama";
use super::settings::DEFAULT_OLLAMA_BASE_URL;
const DETAIL_CONCURRENCY: usize = 4;

/// A Rust-owned adapter for Ollama's native loopback API.
#[derive(Clone)]
pub struct OllamaProvider {
    client: Client,
    base_url: Url,
}

impl OllamaProvider {
    /// Builds an Ollama adapter using the built-in loopback endpoint.
    pub fn new() -> Result<Self, ProviderError> {
        Self::with_base_url(DEFAULT_OLLAMA_BASE_URL)
    }

    /// Builds an Ollama adapter after validating a configurable loopback root.
    pub(crate) fn with_base_url(base_url: &str) -> Result<Self, ProviderError> {
        let base_url = validate_local_base_url(PROVIDER_NAME, base_url)?;
        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(STREAM_IDLE_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| {
                ProviderError::internal(
                    "Could not initialize Ollama inference.",
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
                "Could not construct the Ollama endpoint.",
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

    /// Fetches detailed capability metadata for one installed model.
    async fn show_model(&self, model_id: &str) -> Result<OllamaShowResponse, ProviderError> {
        let response = self
            .client
            .post(self.endpoint("api/show")?)
            .timeout(DISCOVERY_TIMEOUT)
            .json(&OllamaShowRequest {
                model: model_id,
                verbose: false,
            })
            .send()
            .await
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(response_error(response).await);
        }
        let bytes = response.bytes().await.map_err(map_request_error)?;
        serde_json::from_slice(&bytes).map_err(|error| {
            ProviderError::malformed(
                "Ollama returned invalid model details.",
                Some(error.to_string()),
            )
        })
    }

    /// Streams one Ollama tool-capable assistant round without exposing provider JSON.
    pub(crate) async fn stream_tool_round(
        &self,
        session: &OllamaToolSession,
        sink: impl StreamSink + Send + Sync,
    ) -> Result<OllamaToolRound, ProviderError> {
        self.stream_request(&session.request, sink, false).await
    }

    /// Streams one concrete request while accumulating fields required by a tool follow-up.
    async fn stream_request(
        &self,
        request: &OllamaChatRequest,
        sink: impl StreamSink + Send + Sync,
        emit_usage: bool,
    ) -> Result<OllamaToolRound, ProviderError> {
        let response = self
            .client
            .post(self.endpoint("api/chat")?)
            .json(request)
            .send()
            .await
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(response_error(response).await);
        }

        let mut bytes = response.bytes_stream();
        let mut decoder = NdjsonDecoder::default();
        let mut round = OllamaToolRound {
            thinking: String::new(),
            content: String::new(),
            tool_calls: Vec::new(),
            usage: None,
        };
        while let Some(chunk) = bytes.next().await {
            let chunk = chunk.map_err(map_request_error)?;
            for line in decoder.push(&chunk)? {
                if process_stream_line(&line, &sink, &mut round, emit_usage)? {
                    return Ok(round);
                }
            }
        }
        for line in decoder.finish()? {
            if process_stream_line(&line, &sink, &mut round, emit_usage)? {
                return Ok(round);
            }
        }
        Err(ProviderError::malformed(
            "Ollama ended the response before completion.",
            Some("NDJSON stream did not contain a completed event".into()),
        ))
    }
}

impl InferenceProvider for OllamaProvider {
    /// Discovers installed models and enriches them with detail and residency data.
    async fn discover_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let installed = decode_model_list(&self.get("api/tags").await?)?;
        let running = self
            .get("api/ps")
            .await
            .ok()
            .and_then(|bytes| decode_running_models(&bytes).ok());

        let provider = self.clone();
        let installed_models = installed.models.into_iter().filter_map(|listed| {
            let model_id = if listed.model.trim().is_empty() {
                listed.name
            } else {
                listed.model
            };
            (!model_id.trim().is_empty()).then_some((
                model_id,
                listed.capabilities,
                listed.details.context_length,
            ))
        });
        let mut models = stream::iter(installed_models.map(
            move |(model_id, listed_capabilities, listed_context)| {
                let provider = provider.clone();
                let running = running.clone();
                async move {
                    let running_context = running.as_ref().and_then(|models| models.get(&model_id));
                    let details = if listed_capabilities.is_empty() || listed_context.is_none() {
                        provider.show_model(&model_id).await.ok()
                    } else {
                        None
                    };
                    model_info(
                        model_id,
                        &listed_capabilities,
                        listed_context,
                        details.as_ref(),
                        running.is_some(),
                        running_context,
                    )
                }
            },
        ))
        .buffer_unordered(DETAIL_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;

        models.sort_by(|left, right| left.display_name.cmp(&right.display_name));
        if models.is_empty() {
            return Err(ProviderError::unavailable(
                "Ollama is running but has no models installed.",
                None,
            ));
        }
        Ok(models)
    }

    /// Streams one native Ollama NDJSON chat response into the normalized sink.
    async fn stream_chat(
        &self,
        request: ChatRequest,
        sink: impl StreamSink + Send + Sync,
    ) -> Result<Option<Usage>, ProviderError> {
        validate_request(&request)?;
        self.stream_request(&OllamaChatRequest::from(request), sink, true)
            .await
            .map(|round| round.usage)
    }
}

/// Applies one decoded stream event and accumulates fields needed for a tool follow-up request.
fn process_stream_line(
    line: &str,
    sink: &(impl StreamSink + Send + Sync),
    round: &mut OllamaToolRound,
    emit_usage: bool,
) -> Result<bool, ProviderError> {
    let event = decode_stream_line(line)?;
    if !event.reasoning_delta.is_empty() {
        round.thinking.push_str(&event.reasoning_delta);
        sink.reasoning_delta(event.reasoning_delta)?;
    }
    if !event.text_delta.is_empty() {
        round.content.push_str(&event.text_delta);
        sink.text_delta(event.text_delta)?;
    }
    round.tool_calls.extend(event.tool_calls);
    if !event.done {
        return Ok(false);
    }
    round.usage = normalize_usage(event.prompt_eval_count, event.eval_count);
    if emit_usage && let Some(usage) = &round.usage {
        sink.usage_updated(usage.clone())?;
    }
    Ok(true)
}

/// Validates the provider-neutral request invariants required by Ollama.
fn validate_request(request: &ChatRequest) -> Result<(), ProviderError> {
    if request.model_id.trim().is_empty() {
        return Err(ProviderError::invalid_request(
            "Choose an Ollama model before sending.",
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
            message: "Ollama took too long to respond.".into(),
            retryable: true,
            diagnostic: Some(error.to_string()),
        }
    } else if error.is_connect() {
        ProviderError::unavailable(
            "Ollama is offline. Check its configured loopback endpoint and try again.",
            Some(error.to_string()),
        )
    } else {
        ProviderError::unavailable(
            "The connection to Ollama was interrupted.",
            Some(error.to_string()),
        )
    }
}

/// Reads and normalizes a non-success Ollama HTTP response.
async fn response_error(response: reqwest::Response) -> ProviderError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    normalize_response_error(status, &body)
}

/// Normalizes an Ollama HTTP status and optional provider error body.
fn normalize_response_error(status: StatusCode, body: &str) -> ProviderError {
    let provider_message = serde_json::from_str::<OllamaErrorResponse>(body)
        .ok()
        .map(|value| value.error)
        .filter(|message| !message.trim().is_empty());
    let message = provider_message.unwrap_or_else(|| match status {
        StatusCode::NOT_FOUND => "The Ollama API endpoint or model was not found.".into(),
        StatusCode::TOO_MANY_REQUESTS => "Ollama is busy. Try again shortly.".into(),
        _ if status.is_server_error() => "Ollama could not complete the request.".into(),
        _ => "Ollama rejected the request.".into(),
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

#[cfg(test)]
mod tests;
