//! Native OpenAI-compatible discovery and chat-completion streaming.

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

const PROVIDER_ID: &str = "openai";
const PROVIDER_NAME: &str = "OpenAI-compatible";

/// Rust-owned adapter for OpenAI's chat-completion protocol.
#[derive(Clone)]
pub struct OpenAiProvider {
    client: Client,
    base_url: Url,
    api_key: String,
}

impl OpenAiProvider {
    /// Builds an authenticated adapter for an HTTPS API root.
    pub(crate) fn new(base_url: &str, api_key: String) -> Result<Self, ProviderError> {
        let base_url = validate_remote_base_url(PROVIDER_NAME, base_url)?;
        if api_key.trim().is_empty() {
            return Err(ProviderError::invalid_request(
                "Add an OpenAI-compatible API key in Settings.",
            ));
        }
        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(STREAM_IDLE_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| {
                ProviderError::internal("Could not initialize OpenAI-compatible inference.", None)
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
            ProviderError::internal("Could not construct the OpenAI-compatible endpoint.", None)
        })
    }
}

impl InferenceProvider for OpenAiProvider {
    async fn discover_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let response = self
            .client
            .get(self.endpoint("models")?)
            .bearer_auth(&self.api_key)
            .timeout(DISCOVERY_TIMEOUT)
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
        let response = self
            .client
            .post(self.endpoint("chat/completions")?)
            .bearer_auth(&self.api_key)
            .json(&OpenAiChatRequest::from(request))
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
                process_payload(&payload, &sink, &mut usage, &mut completed)?;
            }
        }
        for payload in decoder.finish()? {
            process_payload(&payload, &sink, &mut usage, &mut completed)?;
        }
        if !completed {
            return Err(ProviderError::malformed(
                "The OpenAI-compatible response ended before completion.",
                Some("SSE stream did not contain data: [DONE]".into()),
            ));
        }
        Ok(usage)
    }
}

fn process_payload(
    payload: &str,
    sink: &(impl StreamSink + Send + Sync),
    usage: &mut Option<Usage>,
    completed: &mut bool,
) -> Result<(), ProviderError> {
    match decode_stream_payload(payload)? {
        DecodedEvent::Text(delta) if !delta.is_empty() => sink.text_delta(delta)?,
        DecodedEvent::Reasoning(delta) if !delta.is_empty() => sink.reasoning_delta(delta)?,
        DecodedEvent::Usage(updated) => {
            sink.usage_updated(updated.clone())?;
            *usage = Some(updated);
        }
        DecodedEvent::Done => *completed = true,
        DecodedEvent::Text(_) | DecodedEvent::Reasoning(_) => {}
    }
    Ok(())
}

fn validate_request(request: &ChatRequest) -> Result<(), ProviderError> {
    if request.model_id.trim().is_empty() || request.messages.is_empty() {
        return Err(ProviderError::invalid_request(
            "Choose an OpenAI-compatible model and include at least one message.",
        ));
    }
    Ok(())
}

fn map_request_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError {
            code: ProviderErrorCode::Timeout,
            message: "The OpenAI-compatible provider took too long to respond.".into(),
            retryable: true,
            diagnostic: Some(error.to_string()),
        }
    } else {
        ProviderError::unavailable(
            "The OpenAI-compatible provider could not be reached.",
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
    let provider_message = serde_json::from_str::<OpenAiErrorResponse>(body)
        .ok()
        .map(|response| response.error.message)
        .filter(|message| !message.trim().is_empty());
    let message = provider_message.unwrap_or_else(|| match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            "The OpenAI-compatible API key was rejected.".into()
        }
        StatusCode::TOO_MANY_REQUESTS => "The OpenAI-compatible provider is rate limited.".into(),
        _ if status.is_server_error() => {
            "The OpenAI-compatible provider could not complete the request.".into()
        }
        _ => "The OpenAI-compatible provider rejected the request.".into(),
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
}

fn decode_model_list(bytes: &[u8]) -> Result<Vec<ModelInfo>, ProviderError> {
    let response: ModelList = serde_json::from_slice(bytes).map_err(|error| {
        ProviderError::malformed(
            "The OpenAI-compatible provider returned an invalid model list.",
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
            display_name: model.id.clone(),
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
            "The OpenAI-compatible provider reported no models.",
            None,
        ))
    } else {
        Ok(models)
    }
}

#[derive(Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<OpenAiTurn>,
    stream: bool,
    stream_options: OpenAiStreamOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'static str>,
}

#[derive(Serialize)]
struct OpenAiStreamOptions {
    include_usage: bool,
}

#[derive(Serialize)]
struct OpenAiTurn {
    role: &'static str,
    content: String,
}

impl From<ChatRequest> for OpenAiChatRequest {
    fn from(request: ChatRequest) -> Self {
        Self {
            model: request.model_id,
            messages: request
                .messages
                .into_iter()
                .map(|turn| OpenAiTurn {
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
            stream_options: OpenAiStreamOptions {
                include_usage: true,
            },
            max_completion_tokens: request.settings.max_output_tokens,
            reasoning_effort: (request.settings.reasoning_effort == ReasoningEffort::Low)
                .then_some("low"),
        }
    }
}

#[derive(Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<Choice>,
    usage: Option<WireUsage>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Delta,
}

#[derive(Default, Deserialize)]
struct Delta {
    content: Option<String>,
    reasoning_content: Option<String>,
}

#[derive(Deserialize)]
struct WireUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    #[serde(alias = "cost")]
    cost_usd: Option<f64>,
}

#[derive(Deserialize)]
struct OpenAiErrorResponse {
    error: OpenAiError,
}

#[derive(Deserialize)]
struct OpenAiError {
    message: String,
}

enum DecodedEvent {
    Text(String),
    Reasoning(String),
    Usage(Usage),
    Done,
}

fn decode_stream_payload(payload: &str) -> Result<DecodedEvent, ProviderError> {
    if payload.trim() == "[DONE]" {
        return Ok(DecodedEvent::Done);
    }
    let chunk: StreamChunk = serde_json::from_str(payload).map_err(|error| {
        ProviderError::malformed(
            "The OpenAI-compatible provider sent a malformed stream event.",
            Some(error.to_string()),
        )
    })?;
    if let Some(usage) = chunk.usage {
        return Ok(DecodedEvent::Usage(Usage {
            input_tokens: usage.prompt_tokens,
            output_tokens: usage.completion_tokens,
            cost_usd: usage.cost_usd,
        }));
    }
    let delta = chunk
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.delta)
        .unwrap_or_default();
    if let Some(reasoning) = delta.reasoning_content {
        Ok(DecodedEvent::Reasoning(reasoning))
    } else {
        Ok(DecodedEvent::Text(delta.content.unwrap_or_default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_models_usage_cost_and_reasoning() {
        let models = decode_model_list(br#"{"data":[{"id":"gpt-example"}]}"#).unwrap();
        assert_eq!(models[0].provider_id, "openai");
        let usage = decode_stream_payload(
            r#"{"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":7,"cost":0.004}}"#,
        )
        .unwrap();
        assert!(
            matches!(usage, DecodedEvent::Usage(Usage { cost_usd: Some(cost), .. }) if cost == 0.004)
        );
        let reasoning =
            decode_stream_payload(r#"{"choices":[{"delta":{"reasoning_content":"checking"}}]}"#)
                .unwrap();
        assert!(matches!(reasoning, DecodedEvent::Reasoning(value) if value == "checking"));
    }

    #[test]
    fn request_keeps_reasoning_explicit_and_bounded() {
        let request: ChatRequest = serde_json::from_str(concat!(
            r#"{"providerId":"openai","modelId":"gpt-example","messages":["#,
            r#"{"role":"user","content":[{"type":"text","text":"hi"}]}],"#,
            r#""settings":{"reasoningEffort":"low"}}"#,
        ))
        .unwrap();
        let body = serde_json::to_value(OpenAiChatRequest::from(request)).unwrap();
        assert_eq!(body["reasoning_effort"], "low");
        assert_eq!(body["max_completion_tokens"], 4096);
        assert_eq!(body["stream_options"]["include_usage"], true);
        assert!(body.get("temperature").is_none());
    }
}
