use std::collections::HashMap;

use futures_util::{StreamExt, stream};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use super::{
    InferenceProvider,
    provider::StreamSink,
    settings::{CONNECT_TIMEOUT, DISCOVERY_TIMEOUT, STREAM_IDLE_TIMEOUT, validate_local_base_url},
    types::{
        ChatRequest, ChatRole, ContentBlock, ModelInfo, ModelLoadState, ProviderCapabilities,
        ProviderError, ProviderErrorCode, Usage,
    },
};

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
    pub fn new() -> Result<Self, ProviderError> {
        Self::with_base_url(DEFAULT_OLLAMA_BASE_URL)
    }

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

    pub(crate) fn base_url(&self) -> &str {
        self.base_url.as_str()
    }

    fn endpoint(&self, path: &str) -> Result<Url, ProviderError> {
        self.base_url.join(path).map_err(|error| {
            ProviderError::internal(
                "Could not construct the Ollama endpoint.",
                Some(error.to_string()),
            )
        })
    }

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
}

impl InferenceProvider for OllamaProvider {
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

    async fn stream_chat(
        &self,
        request: ChatRequest,
        sink: impl StreamSink + Send + Sync,
    ) -> Result<Option<Usage>, ProviderError> {
        validate_request(&request)?;
        let response = self
            .client
            .post(self.endpoint("api/chat")?)
            .json(&OllamaChatRequest::from(request))
            .send()
            .await
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(response_error(response).await);
        }

        let mut bytes = response.bytes_stream();
        let mut decoder = NdjsonDecoder::default();
        while let Some(chunk) = bytes.next().await {
            let chunk = chunk.map_err(map_request_error)?;
            for line in decoder.push(&chunk)? {
                if let Some(usage) = process_stream_line(&line, &sink)? {
                    return Ok(usage);
                }
            }
        }
        for line in decoder.finish()? {
            if let Some(usage) = process_stream_line(&line, &sink)? {
                return Ok(usage);
            }
        }
        Err(ProviderError::malformed(
            "Ollama ended the response before completion.",
            Some("NDJSON stream did not contain a completed event".into()),
        ))
    }
}

fn process_stream_line(
    line: &str,
    sink: &(impl StreamSink + Send + Sync),
) -> Result<Option<Option<Usage>>, ProviderError> {
    let event = decode_stream_line(line)?;
    if !event.delta.is_empty() {
        sink.text_delta(event.delta)?;
    }
    if !event.done {
        return Ok(None);
    }
    let usage = normalize_usage(event.prompt_eval_count, event.eval_count);
    if let Some(usage) = &usage {
        sink.usage_updated(usage.clone())?;
    }
    Ok(Some(usage))
}

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

async fn response_error(response: reqwest::Response) -> ProviderError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    normalize_response_error(status, &body)
}

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

#[derive(Deserialize)]
struct OllamaModelList {
    #[serde(default)]
    models: Vec<OllamaListedModel>,
}

#[derive(Deserialize)]
struct OllamaListedModel {
    #[serde(default)]
    name: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    details: OllamaListedDetails,
}

#[derive(Default, Deserialize)]
struct OllamaListedDetails {
    context_length: Option<u64>,
}

fn decode_model_list(bytes: &[u8]) -> Result<OllamaModelList, ProviderError> {
    serde_json::from_slice(bytes).map_err(|error| {
        ProviderError::malformed(
            "Ollama returned an invalid model list.",
            Some(error.to_string()),
        )
    })
}

#[derive(Clone, Deserialize)]
struct OllamaRunningList {
    #[serde(default)]
    models: Vec<OllamaRunningModel>,
}

#[derive(Clone, Deserialize)]
struct OllamaRunningModel {
    #[serde(default)]
    name: String,
    #[serde(default)]
    model: String,
    context_length: Option<u64>,
}

fn decode_running_models(bytes: &[u8]) -> Result<HashMap<String, Option<u64>>, ProviderError> {
    let response: OllamaRunningList = serde_json::from_slice(bytes).map_err(|error| {
        ProviderError::malformed(
            "Ollama returned an invalid running-model list.",
            Some(error.to_string()),
        )
    })?;
    Ok(response
        .models
        .into_iter()
        .map(|model| {
            let id = if model.model.trim().is_empty() {
                model.name
            } else {
                model.model
            };
            (id, model.context_length)
        })
        .collect())
}

#[derive(Serialize)]
struct OllamaShowRequest<'a> {
    model: &'a str,
    verbose: bool,
}

#[derive(Deserialize)]
struct OllamaShowResponse {
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    model_info: HashMap<String, Value>,
}

fn model_info(
    model_id: String,
    listed_capabilities: &[String],
    listed_context: Option<u64>,
    details: Option<&OllamaShowResponse>,
    running_known: bool,
    running_context: Option<&Option<u64>>,
) -> ModelInfo {
    let capabilities = details
        .map(|details| details.capabilities.as_slice())
        .filter(|capabilities| !capabilities.is_empty())
        .or((!listed_capabilities.is_empty()).then_some(listed_capabilities))
        .map(capability_map)
        .unwrap_or_default();
    let max_context_tokens = details
        .and_then(|details| context_length(&details.model_info))
        .or(listed_context)
        .or(running_context.copied().flatten());
    ModelInfo {
        provider_id: PROVIDER_ID.into(),
        provider_name: PROVIDER_NAME.into(),
        display_name: model_id.clone(),
        model_id,
        max_context_tokens,
        load_state: if running_context.is_some() {
            ModelLoadState::Loaded
        } else if running_known {
            ModelLoadState::Unloaded
        } else {
            ModelLoadState::Unknown
        },
        capabilities,
    }
}

fn capability_map(capabilities: &[String]) -> ProviderCapabilities {
    let has = |name: &str| capabilities.iter().any(|capability| capability == name);
    ProviderCapabilities {
        text: has("completion"),
        streaming: has("completion"),
        tools: has("tools"),
        vision: has("vision"),
        embeddings: has("embedding") || has("embeddings"),
    }
}

fn context_length(model_info: &HashMap<String, Value>) -> Option<u64> {
    model_info
        .iter()
        .filter(|(key, _)| key.ends_with(".context_length"))
        .filter_map(|(_, value)| value.as_u64())
        .max()
}

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaChatTurn>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Serialize)]
struct OllamaChatTurn {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

impl From<ChatRequest> for OllamaChatRequest {
    fn from(request: ChatRequest) -> Self {
        let options = (request.settings.temperature.is_some()
            || request.settings.max_output_tokens.is_some())
        .then_some(OllamaOptions {
            temperature: request.settings.temperature,
            num_predict: request.settings.max_output_tokens,
        });
        Self {
            model: request.model_id,
            messages: request
                .messages
                .into_iter()
                .map(|turn| OllamaChatTurn {
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
            options,
        }
    }
}

#[derive(Deserialize)]
struct OllamaStreamChunk {
    message: Option<OllamaStreamMessage>,
    #[serde(default)]
    done: bool,
    prompt_eval_count: Option<u64>,
    eval_count: Option<u64>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct OllamaStreamMessage {
    #[serde(default)]
    content: String,
}

#[derive(Debug)]
struct DecodedStreamEvent {
    delta: String,
    done: bool,
    prompt_eval_count: Option<u64>,
    eval_count: Option<u64>,
}

fn decode_stream_line(line: &str) -> Result<DecodedStreamEvent, ProviderError> {
    let chunk: OllamaStreamChunk = serde_json::from_str(line).map_err(|error| {
        ProviderError::malformed(
            "Ollama sent a malformed stream event.",
            Some(error.to_string()),
        )
    })?;
    if let Some(message) = chunk.error.filter(|message| !message.trim().is_empty()) {
        return Err(ProviderError::server(
            message,
            Some("Ollama stream error".into()),
        ));
    }
    Ok(DecodedStreamEvent {
        delta: chunk
            .message
            .map(|message| message.content)
            .unwrap_or_default(),
        done: chunk.done,
        prompt_eval_count: chunk.prompt_eval_count,
        eval_count: chunk.eval_count,
    })
}

fn normalize_usage(input_tokens: Option<u64>, output_tokens: Option<u64>) -> Option<Usage> {
    (input_tokens.is_some() || output_tokens.is_some()).then_some(Usage {
        input_tokens,
        output_tokens,
    })
}

#[derive(Default)]
struct NdjsonDecoder {
    buffer: Vec<u8>,
}

impl NdjsonDecoder {
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, ProviderError> {
        self.buffer.extend_from_slice(bytes);
        self.drain(false)
    }

    fn finish(&mut self) -> Result<Vec<String>, ProviderError> {
        self.drain(true)
    }

    fn drain(&mut self, finish: bool) -> Result<Vec<String>, ProviderError> {
        let mut lines = Vec::new();
        while let Some(index) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let line = self.buffer.drain(..index).collect::<Vec<_>>();
            self.buffer.drain(..1);
            if let Some(line) = decode_ndjson_line(&line)? {
                lines.push(line);
            }
        }
        if finish && !self.buffer.is_empty() {
            let line = std::mem::take(&mut self.buffer);
            if let Some(line) = decode_ndjson_line(&line)? {
                lines.push(line);
            }
        }
        Ok(lines)
    }
}

fn decode_ndjson_line(bytes: &[u8]) -> Result<Option<String>, ProviderError> {
    let text = std::str::from_utf8(bytes).map_err(|error| {
        ProviderError::malformed(
            "Ollama sent invalid text in its stream.",
            Some(error.to_string()),
        )
    })?;
    let line = text.trim();
    Ok((!line.is_empty()).then(|| line.to_owned()))
}

#[derive(Deserialize)]
struct OllamaErrorResponse {
    error: String,
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::inference::types::{ChatSettings, ChatTurn, ContentBlock};
    use futures_util::{FutureExt, future::Abortable};

    #[derive(Clone, Default)]
    struct RecordingSink {
        text: Arc<Mutex<String>>,
        usage: Arc<Mutex<Option<Usage>>>,
        abort_after_delta: Option<futures_util::future::AbortHandle>,
    }

    impl StreamSink for RecordingSink {
        fn text_delta(&self, delta: String) -> Result<(), ProviderError> {
            self.text.lock().unwrap().push_str(&delta);
            if let Some(handle) = &self.abort_after_delta {
                handle.abort();
            }
            Ok(())
        }

        fn usage_updated(&self, usage: Usage) -> Result<(), ProviderError> {
            *self.usage.lock().unwrap() = Some(usage);
            Ok(())
        }
    }

    fn live_request(model_id: String, prompt: &str) -> ChatRequest {
        ChatRequest {
            provider_id: PROVIDER_ID.into(),
            model_id,
            messages: vec![ChatTurn {
                role: ChatRole::User,
                content: vec![ContentBlock::Text {
                    text: prompt.into(),
                }],
            }],
            settings: ChatSettings {
                temperature: Some(0.0),
                max_output_tokens: Some(80),
            },
        }
    }

    async fn smallest_live_model(provider: &OllamaProvider) -> String {
        provider
            .discover_models()
            .await
            .expect("local Ollama must be running for this ignored test")
            .into_iter()
            .filter(|model| model.capabilities.text)
            .min_by_key(|model| model.max_context_tokens.unwrap_or(u64::MAX))
            .expect("Ollama must have a chat model")
            .model_id
    }

    #[test]
    fn accepts_only_loopback_endpoints() {
        assert!(OllamaProvider::with_base_url("http://127.0.0.1:11434/").is_ok());
        assert!(OllamaProvider::with_base_url("http://localhost:11434/").is_ok());
        assert!(OllamaProvider::with_base_url("https://example.com/").is_err());
        assert!(OllamaProvider::with_base_url("file:///tmp/models").is_err());
    }

    #[test]
    fn decodes_model_capabilities_context_and_load_state() {
        let listed = decode_model_list(
            br#"{"models":[{"name":"gemma3:4b","model":"gemma3:4b","capabilities":["completion","tools","vision","embedding"],"details":{"context_length":131072}}]}"#,
        )
        .unwrap();
        assert_eq!(listed.models.len(), 1);
        let details: OllamaShowResponse = serde_json::from_str(
            r#"{"capabilities":[],"model_info":{"gemma3.context_length":131072}}"#,
        )
        .unwrap();
        let running_context = Some(4096);
        let model = model_info(
            "gemma3:4b".into(),
            &listed.models[0].capabilities,
            listed.models[0].details.context_length,
            Some(&details),
            true,
            Some(&running_context),
        );
        assert!(model.capabilities.text);
        assert!(model.capabilities.tools);
        assert!(model.capabilities.vision);
        assert!(model.capabilities.embeddings);
        assert_eq!(model.max_context_tokens, Some(131_072));
        assert_eq!(model.load_state, ModelLoadState::Loaded);
    }

    #[test]
    fn keeps_embedding_only_models_out_of_the_chat_catalogue() {
        let capabilities = capability_map(&["embedding".into()]);
        assert!(capabilities.embeddings);
        assert!(!capabilities.text);
        assert!(!capabilities.streaming);
    }

    #[test]
    fn decodes_fragmented_ndjson_and_completion_usage() {
        let mut decoder = NdjsonDecoder::default();
        assert!(
            decoder
                .push(br#"{"message":{"content":"hel"#)
                .unwrap()
                .is_empty()
        );
        let lines = decoder
            .push(b"lo\"},\"done\":false}\n{\"message\":{\"content\":\"\"},\"done\":true,\"prompt_eval_count\":12,\"eval_count\":7}\n")
            .unwrap();
        assert_eq!(lines.len(), 2);
        let delta = decode_stream_line(&lines[0]).unwrap();
        assert_eq!(delta.delta, "hello");
        let completed = decode_stream_line(&lines[1]).unwrap();
        assert!(completed.done);
        assert_eq!(completed.prompt_eval_count, Some(12));
        assert_eq!(completed.eval_count, Some(7));
    }

    #[test]
    fn rejects_malformed_and_provider_error_events() {
        let malformed = decode_stream_line("not json").unwrap_err();
        assert_eq!(malformed.code, ProviderErrorCode::MalformedResponse);
        let provider = decode_stream_line(r#"{"error":"model not found"}"#).unwrap_err();
        assert_eq!(provider.code, ProviderErrorCode::Server);
        assert_eq!(provider.message, "model not found");
    }

    #[test]
    fn normalizes_provider_http_errors() {
        let invalid = normalize_response_error(
            StatusCode::NOT_FOUND,
            r#"{"error":"model 'missing' not found"}"#,
        );
        assert_eq!(invalid.code, ProviderErrorCode::InvalidRequest);
        assert_eq!(invalid.message, "model 'missing' not found");
        let server = normalize_response_error(StatusCode::SERVICE_UNAVAILABLE, "");
        assert_eq!(server.code, ProviderErrorCode::Server);
        assert!(server.retryable);
    }

    #[test]
    fn maps_an_unavailable_loopback_provider() {
        tauri::async_runtime::block_on(async {
            let provider = OllamaProvider::with_base_url("http://127.0.0.1:9/").unwrap();
            let error = provider.discover_models().await.unwrap_err();
            assert_eq!(error.code, ProviderErrorCode::Unavailable);
            assert!(error.retryable);
        });
    }

    #[test]
    fn abort_handle_cancels_an_in_flight_stream_future() {
        let (handle, registration) = futures_util::future::AbortHandle::new_pair();
        let future = Abortable::new(futures_util::future::pending::<()>(), registration);
        handle.abort();
        assert!(matches!(future.now_or_never(), Some(Err(_))));
    }

    #[test]
    #[ignore = "requires a running Ollama server on 127.0.0.1:11434"]
    fn live_ollama_stream_completes() {
        tauri::async_runtime::block_on(async {
            let provider = OllamaProvider::new().unwrap();
            let model = smallest_live_model(&provider).await;
            let sink = RecordingSink::default();
            let recorded = sink.text.clone();
            provider
                .stream_chat(
                    live_request(model, "Reply with exactly: bottie Ollama stream ready"),
                    sink,
                )
                .await
                .expect("live stream should complete");
            assert!(!recorded.lock().unwrap().trim().is_empty());
        });
    }

    #[test]
    #[ignore = "requires a running Ollama server on 127.0.0.1:11434"]
    fn live_ollama_stream_can_be_aborted_after_a_delta() {
        tauri::async_runtime::block_on(async {
            let provider = OllamaProvider::new().unwrap();
            let model = smallest_live_model(&provider).await;
            let (handle, registration) = futures_util::future::AbortHandle::new_pair();
            let sink = RecordingSink {
                text: Arc::new(Mutex::new(String::new())),
                usage: Arc::new(Mutex::new(None)),
                abort_after_delta: Some(handle),
            };
            let recorded = sink.text.clone();
            let result = Abortable::new(
                provider.stream_chat(
                    live_request(model, "Write a paragraph about streaming cancellation."),
                    sink,
                ),
                registration,
            )
            .await;
            assert!(result.is_err(), "the stream future should be aborted");
            assert!(!recorded.lock().unwrap().is_empty());
        });
    }
}
