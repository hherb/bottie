//! Native Anthropic-compatible model discovery and Messages API streaming.

use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use url::Url;

use super::{
    InferenceProvider, StreamSink,
    settings::{CONNECT_TIMEOUT, DISCOVERY_TIMEOUT, STREAM_IDLE_TIMEOUT, validate_remote_base_url},
    sse::SseDecoder,
    types::{
        ChatRequest, ChatRole, ContentBlock, ModelInfo, ModelLoadState, ProviderCapabilities,
        ProviderError, ProviderErrorCode, ReasoningEffort, Usage,
    },
};

const PROVIDER_ID: &str = "anthropic";
const PROVIDER_NAME: &str = "Anthropic-compatible";
const ANTHROPIC_VERSION: &str = "2023-06-01";

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
        let request = self
            .client
            .post(self.endpoint("messages")?)
            .json(&AnthropicChatRequest::from(request));
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
        let mut usage = Usage::default();
        let mut saw_usage = false;
        let mut completed = false;
        while let Some(chunk) = bytes.next().await {
            let chunk = chunk.map_err(map_request_error)?;
            for payload in decoder.push(&chunk)? {
                process_payload(&payload, &sink, &mut usage, &mut saw_usage, &mut completed)?;
            }
        }
        for payload in decoder.finish()? {
            process_payload(&payload, &sink, &mut usage, &mut saw_usage, &mut completed)?;
        }
        if !completed {
            return Err(ProviderError::malformed(
                "The Anthropic-compatible response ended before completion.",
                Some("SSE stream did not contain a message_stop event".into()),
            ));
        }
        Ok(saw_usage.then_some(usage))
    }
}

fn process_payload(
    payload: &str,
    sink: &(impl StreamSink + Send + Sync),
    usage: &mut Usage,
    saw_usage: &mut bool,
    completed: &mut bool,
) -> Result<(), ProviderError> {
    match decode_stream_payload(payload)? {
        DecodedEvent::Text(delta) if !delta.is_empty() => sink.text_delta(delta)?,
        DecodedEvent::Reasoning(delta) if !delta.is_empty() => sink.reasoning_delta(delta)?,
        DecodedEvent::Usage(updated) => {
            if updated.input_tokens.is_some() {
                usage.input_tokens = updated.input_tokens;
            }
            if updated.output_tokens.is_some() {
                usage.output_tokens = updated.output_tokens;
            }
            if updated.cost_usd.is_some() {
                usage.cost_usd = updated.cost_usd;
            }
            *saw_usage = true;
            sink.usage_updated(usage.clone())?;
        }
        DecodedEvent::Done => *completed = true,
        DecodedEvent::Ignored | DecodedEvent::Text(_) | DecodedEvent::Reasoning(_) => {}
    }
    Ok(())
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
        .map(|model| ModelInfo {
            provider_id: PROVIDER_ID.into(),
            provider_name: PROVIDER_NAME.into(),
            display_name: model.display_name.unwrap_or_else(|| model.id.clone()),
            model_id: model.id,
            max_context_tokens: None,
            load_state: ModelLoadState::Unknown,
            capabilities: ProviderCapabilities {
                text: true,
                streaming: true,
                ..Default::default()
            },
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

#[derive(Serialize)]
struct AnthropicChatRequest {
    model: String,
    messages: Vec<AnthropicTurn>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    thinking: ThinkingConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_config: Option<OutputConfig>,
}

#[derive(Serialize)]
struct AnthropicTurn {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ThinkingConfig {
    Disabled,
    Adaptive,
}

#[derive(Serialize)]
struct OutputConfig {
    effort: &'static str,
}

impl From<ChatRequest> for AnthropicChatRequest {
    fn from(request: ChatRequest) -> Self {
        let reasoning_enabled = request.settings.reasoning_effort == ReasoningEffort::Low;
        let mut system = Vec::new();
        let mut messages = Vec::new();
        for turn in request.messages {
            let content = turn
                .content
                .into_iter()
                .map(|block| match block {
                    ContentBlock::Text { text } => text,
                })
                .collect::<Vec<_>>()
                .join("\n");
            match turn.role {
                ChatRole::System => system.push(content),
                ChatRole::User => messages.push(AnthropicTurn {
                    role: "user",
                    content,
                }),
                ChatRole::Assistant => messages.push(AnthropicTurn {
                    role: "assistant",
                    content,
                }),
            }
        }
        Self {
            model: request.model_id,
            messages,
            system: (!system.is_empty()).then(|| system.join("\n\n")),
            max_tokens: request.settings.max_output_tokens.unwrap_or(4_096),
            stream: true,
            temperature: (!reasoning_enabled)
                .then_some(request.settings.temperature)
                .flatten(),
            thinking: if reasoning_enabled {
                ThinkingConfig::Adaptive
            } else {
                ThinkingConfig::Disabled
            },
            output_config: reasoning_enabled.then_some(OutputConfig { effort: "low" }),
        }
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

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamPayload {
    MessageStart {
        message: StartMessage,
    },
    ContentBlockDelta {
        delta: ContentDelta,
    },
    MessageDelta {
        usage: WireUsage,
    },
    MessageStop,
    Error {
        error: AnthropicError,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize)]
struct StartMessage {
    usage: WireUsage,
}

#[derive(Default, Deserialize)]
struct WireUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cost_usd: Option<f64>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentDelta {
    TextDelta {
        text: String,
    },
    ThinkingDelta {
        thinking: String,
    },
    #[serde(other)]
    Unknown,
}

enum DecodedEvent {
    Text(String),
    Reasoning(String),
    Usage(Usage),
    Done,
    Ignored,
}

fn decode_stream_payload(payload: &str) -> Result<DecodedEvent, ProviderError> {
    let event: StreamPayload = serde_json::from_str(payload).map_err(|error| {
        ProviderError::malformed(
            "The Anthropic-compatible provider sent a malformed stream event.",
            Some(error.to_string()),
        )
    })?;
    match event {
        StreamPayload::MessageStart { message } => Ok(DecodedEvent::Usage(message.usage.into())),
        StreamPayload::MessageDelta { usage } => Ok(DecodedEvent::Usage(usage.into())),
        StreamPayload::ContentBlockDelta {
            delta: ContentDelta::TextDelta { text },
        } => Ok(DecodedEvent::Text(text)),
        StreamPayload::ContentBlockDelta {
            delta: ContentDelta::ThinkingDelta { thinking },
        } => Ok(DecodedEvent::Reasoning(thinking)),
        StreamPayload::MessageStop => Ok(DecodedEvent::Done),
        StreamPayload::Error { error } => Err(ProviderError::server(error.message, None)),
        StreamPayload::Unknown
        | StreamPayload::ContentBlockDelta {
            delta: ContentDelta::Unknown,
        } => Ok(DecodedEvent::Ignored),
    }
}

impl From<WireUsage> for Usage {
    fn from(value: WireUsage) -> Self {
        Self {
            input_tokens: value.input_tokens,
            output_tokens: value.output_tokens,
            cost_usd: value.cost_usd,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_text_thinking_usage_and_completion() {
        assert!(matches!(
            decode_stream_payload(concat!(
                r#"{"type":"content_block_delta","index":0,"delta":{"#,
                r#""type":"text_delta","text":"Hi"}}"#,
            ))
            .unwrap(),
            DecodedEvent::Text(value) if value == "Hi"
        ));
        assert!(matches!(
            decode_stream_payload(concat!(
                r#"{"type":"content_block_delta","index":0,"delta":{"#,
                r#""type":"thinking_delta","thinking":"Check"}}"#,
            ))
            .unwrap(),
            DecodedEvent::Reasoning(value) if value == "Check"
        ));
        assert!(matches!(
            decode_stream_payload(r#"{"type":"message_stop"}"#).unwrap(),
            DecodedEvent::Done
        ));
    }

    #[test]
    fn request_separates_system_turn_and_maps_reasoning() {
        let request: ChatRequest = serde_json::from_str(concat!(
            r#"{"providerId":"anthropic","modelId":"claude-example","messages":["#,
            r#"{"role":"system","content":[{"type":"text","text":"Be brief"}]},"#,
            r#"{"role":"user","content":[{"type":"text","text":"Hi"}]}],"#,
            r#""settings":{"reasoningEffort":"low"}}"#,
        ))
        .unwrap();
        let body = serde_json::to_value(AnthropicChatRequest::from(request)).unwrap();
        assert_eq!(body["system"], "Be brief");
        assert_eq!(body["thinking"]["type"], "adaptive");
        assert_eq!(body["output_config"]["effort"], "low");
        assert!(body.get("temperature").is_none());
    }
}
