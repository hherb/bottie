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
        ProviderError, ProviderErrorCode, Usage,
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
    pub fn new() -> Result<Self, ProviderError> {
        Self::with_base_url(DEFAULT_OMLX_BASE_URL)
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
                    "Could not initialize local inference.",
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
                "Could not construct the oMLX endpoint.",
                Some(error.to_string()),
            )
        })
    }
}

impl InferenceProvider for OmlxProvider {
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
                    DecodedEvent::Delta(delta) if !delta.is_empty() => sink.text_delta(delta)?,
                    DecodedEvent::Usage(updated) => {
                        sink.usage_updated(updated.clone())?;
                        usage = Some(updated);
                    }
                    DecodedEvent::Done => completed = true,
                    DecodedEvent::Delta(_) => {}
                }
            }
        }

        for payload in decoder.finish()? {
            match decode_stream_payload(&payload)? {
                DecodedEvent::Delta(delta) if !delta.is_empty() => sink.text_delta(delta)?,
                DecodedEvent::Usage(updated) => {
                    sink.usage_updated(updated.clone())?;
                    usage = Some(updated);
                }
                DecodedEvent::Done => completed = true,
                DecodedEvent::Delta(_) => {}
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

async fn response_error(response: reqwest::Response) -> ProviderError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    normalize_response_error(status, &body)
}

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
    stream_options: OmlxStreamOptions,
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
    fn from(request: ChatRequest) -> Self {
        let settings = request.settings;
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
    Delta(String),
    Usage(Usage),
    Done,
}

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
    Ok(DecodedEvent::Delta(
        chunk
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.delta.content)
            .unwrap_or_default(),
    ))
}

#[derive(Default)]
struct SseDecoder {
    buffer: Vec<u8>,
}

impl SseDecoder {
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, ProviderError> {
        self.buffer.extend_from_slice(bytes);
        self.drain(false)
    }

    fn finish(&mut self) -> Result<Vec<String>, ProviderError> {
        self.drain(true)
    }

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
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use futures_util::{FutureExt, future::Abortable};

    use crate::inference::types::{ChatSettings, ChatTurn, ContentBlock};

    #[derive(Clone, Default)]
    struct RecordingSink {
        text: Arc<Mutex<String>>,
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

        fn usage_updated(&self, _usage: Usage) -> Result<(), ProviderError> {
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

    async fn smallest_live_model(provider: &OmlxProvider) -> String {
        let models = provider
            .discover_models()
            .await
            .expect("local oMLX must be running for this ignored test");
        models
            .iter()
            .find(|model| model.model_id.contains("1.2B"))
            .unwrap_or(&models[0])
            .model_id
            .clone()
    }

    #[test]
    fn accepts_only_loopback_endpoints() {
        assert!(OmlxProvider::with_base_url("http://127.0.0.1:8000/").is_ok());
        assert!(OmlxProvider::with_base_url("http://localhost:8000/").is_ok());
        assert!(OmlxProvider::with_base_url("https://example.com/").is_err());
        assert!(OmlxProvider::with_base_url("file:///tmp/models").is_err());
    }

    #[test]
    fn decodes_live_model_list_shape() {
        let models = decode_model_list(
            br#"{"object":"list","data":[{"id":"Qwen3.6-35B-A3B-8bit","object":"model","max_model_len":262144}]}"#,
        )
        .expect("model list should decode");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].model_id, "Qwen3.6-35B-A3B-8bit");
        assert_eq!(models[0].max_context_tokens, Some(262_144));
        assert!(models[0].capabilities.streaming);
    }

    #[test]
    fn decodes_fragmented_sse_and_completion() {
        let mut decoder = SseDecoder::default();
        assert!(
            decoder
                .push(b"data: {\"choices\":[{\"delta\":{\"con")
                .unwrap()
                .is_empty()
        );
        let payloads = decoder
            .push(b"tent\":\"hello\"}}]}\r\n\r\ndata: [DONE]\n\n")
            .unwrap();
        assert_eq!(payloads.len(), 2);
        assert!(matches!(
            decode_stream_payload(&payloads[0]).unwrap(),
            DecodedEvent::Delta(ref delta) if delta == "hello"
        ));
        assert!(matches!(
            decode_stream_payload(&payloads[1]).unwrap(),
            DecodedEvent::Done
        ));
    }

    #[test]
    fn decodes_usage_update() {
        let event = decode_stream_payload(
            r#"{"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":7}}"#,
        )
        .unwrap();
        assert!(matches!(
            event,
            DecodedEvent::Usage(Usage {
                input_tokens: Some(12),
                output_tokens: Some(7)
            })
        ));
    }

    #[test]
    fn rejects_malformed_event() {
        let error = decode_stream_payload("not json").unwrap_err();
        assert_eq!(error.code, ProviderErrorCode::MalformedResponse);
    }

    #[test]
    fn decodes_provider_error_body() {
        let body: OmlxErrorResponse = serde_json::from_str(
            r#"{"error":{"message":"Model was not found","type":"not_found"}}"#,
        )
        .unwrap();
        assert_eq!(body.error.message, "Model was not found");
    }

    #[test]
    fn normalizes_provider_http_errors() {
        let invalid = normalize_response_error(
            StatusCode::BAD_REQUEST,
            r#"{"error":{"message":"Model was not found"}}"#,
        );
        assert_eq!(invalid.code, ProviderErrorCode::InvalidRequest);
        assert_eq!(invalid.message, "Model was not found");
        assert!(!invalid.retryable);

        let server = normalize_response_error(StatusCode::SERVICE_UNAVAILABLE, "");
        assert_eq!(server.code, ProviderErrorCode::Server);
        assert!(server.retryable);
    }

    #[test]
    fn maps_an_unavailable_loopback_provider() {
        tauri::async_runtime::block_on(async {
            let provider = OmlxProvider::with_base_url("http://127.0.0.1:9/").unwrap();
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
    #[ignore = "requires a running oMLX server on 127.0.0.1:8000"]
    fn live_omlx_stream_completes() {
        tauri::async_runtime::block_on(async {
            let provider = OmlxProvider::new().unwrap();
            let model = smallest_live_model(&provider).await;
            let sink = RecordingSink::default();
            let recorded = sink.text.clone();
            provider
                .stream_chat(
                    live_request(model, "Reply with exactly: bottie live stream ready"),
                    sink,
                )
                .await
                .expect("live stream should complete");
            assert!(!recorded.lock().unwrap().trim().is_empty());
        });
    }

    #[test]
    #[ignore = "requires a running oMLX server on 127.0.0.1:8000"]
    fn live_omlx_stream_can_be_aborted_after_a_delta() {
        tauri::async_runtime::block_on(async {
            let provider = OmlxProvider::new().unwrap();
            let model = smallest_live_model(&provider).await;
            let (handle, registration) = futures_util::future::AbortHandle::new_pair();
            let sink = RecordingSink {
                text: Arc::new(Mutex::new(String::new())),
                abort_after_delta: Some(handle),
            };
            let recorded = sink.text.clone();
            let result = Abortable::new(
                provider.stream_chat(
                    live_request(
                        model,
                        "Write a detailed paragraph about why cancellation matters in streaming UI.",
                    ),
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
