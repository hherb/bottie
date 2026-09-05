//! Native OpenAI-compatible discovery and chat-completion streaming.

use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use url::Url;

use super::{
    InferenceProvider, StreamSink,
    settings::{CONNECT_TIMEOUT, DISCOVERY_TIMEOUT, STREAM_IDLE_TIMEOUT, validate_remote_base_url},
    sse::SseDecoder,
    types::{ChatRequest, ModelInfo, ProviderError, ProviderErrorCode, Usage},
};
use crate::tool_contract::python_enabled_native_tool_definitions;

use self::protocol::{
    DecodedStreamEvent, OpenAiChatRequest, OpenAiToolCallAccumulator, decode_stream_payload,
};

#[cfg(test)]
mod audio_tests;
mod discovery;
pub(crate) mod protocol;
#[cfg(test)]
mod python_tests;

use discovery::decode_model_list;

pub(crate) use protocol::{OpenAiToolCall, OpenAiToolResult};

const PROVIDER_ID: &str = "openai";
const PROVIDER_NAME: &str = "OpenAI-compatible";

/// One provider-native Chat Completions history spanning repeated native-tool rounds.
pub(crate) struct OpenAiToolSession {
    request: OpenAiChatRequest,
}

impl OpenAiToolSession {
    /// Starts a session with exactly the closed native tools enabled for this request.
    pub(crate) fn new(request: ChatRequest, python_available: bool) -> Result<Self, ProviderError> {
        validate_request(&request)?;
        let definitions = python_enabled_native_tool_definitions(
            request.memory_enabled,
            request.web_enabled,
            request.email_enabled,
            python_available,
        );
        Ok(Self {
            request: OpenAiChatRequest::with_tools(request, definitions),
        })
    }

    /// Appends one accumulated assistant call batch and its exact correlated native results.
    pub(crate) fn append_results(
        &mut self,
        round: OpenAiToolRound,
        results: Vec<OpenAiToolResult>,
    ) -> Result<(), ProviderError> {
        self.request
            .append_tool_exchange(round.reasoning, round.content, round.tool_calls, results)
    }
}

/// One complete streamed Chat Completions assistant round before optional native execution.
pub(crate) struct OpenAiToolRound {
    /// Accumulated separate reasoning required in the next provider request when reported.
    pub(crate) reasoning: String,
    /// Accumulated assistant answer text required in the next provider request.
    pub(crate) content: String,
    /// Ordered complete function calls reconstructed from streamed fragments.
    pub(crate) tool_calls: Vec<OpenAiToolCall>,
    /// Provider-reported usage for this Chat Completions request.
    pub(crate) usage: Option<Usage>,
}

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

    #[cfg(test)]
    /// Builds a test-only adapter for an isolated loopback HTTP fixture.
    pub(crate) fn for_loopback_fixture(base_url: &str) -> Result<Self, ProviderError> {
        let base_url = Url::parse(base_url).map_err(|_| {
            ProviderError::internal("Could not construct the OpenAI test endpoint.", None)
        })?;
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| ProviderError::internal("Could not initialize OpenAI tests.", None))?;
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
            ProviderError::internal("Could not construct the OpenAI-compatible endpoint.", None)
        })
    }

    /// Streams one tool-capable Chat Completions round without exposing provider JSON.
    pub(crate) async fn stream_tool_round(
        &self,
        session: &OpenAiToolSession,
        sink: impl StreamSink + Send + Sync,
    ) -> Result<OpenAiToolRound, ProviderError> {
        self.stream_request(&session.request, sink, false).await
    }

    /// Streams one concrete request while accumulating fields required by a tool follow-up.
    async fn stream_request(
        &self,
        request: &OpenAiChatRequest,
        sink: impl StreamSink + Send + Sync,
        emit_usage: bool,
    ) -> Result<OpenAiToolRound, ProviderError> {
        let response = self
            .client
            .post(self.endpoint("chat/completions")?)
            .bearer_auth(&self.api_key)
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
        let mut round = OpenAiToolRound {
            reasoning: String::new(),
            content: String::new(),
            tool_calls: Vec::new(),
            usage: None,
        };
        while let Some(chunk) = bytes.next().await {
            let chunk = chunk.map_err(map_request_error)?;
            for payload in decoder.push(&chunk)? {
                if process_stream_event(
                    decode_stream_payload(&payload)?,
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
                decode_stream_payload(&payload)?,
                &sink,
                &mut round,
                &mut calls,
                emit_usage,
            )? {
                return Ok(round);
            }
        }
        Err(ProviderError::malformed(
            "The OpenAI-compatible response ended before completion.",
            Some("SSE stream did not contain data: [DONE]".into()),
        ))
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
        self.stream_request(&OpenAiChatRequest::from(request), sink, true)
            .await
            .map(|round| round.usage)
    }
}

/// Applies one decoded event and accumulates provider fields needed for a follow-up request.
fn process_stream_event(
    event: DecodedStreamEvent,
    sink: &(impl StreamSink + Send + Sync),
    round: &mut OpenAiToolRound,
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
    calls.extend(event.tool_call_deltas)?;
    if let Some(usage) = event.usage {
        if emit_usage {
            sink.usage_updated(usage.clone())?;
        }
        round.usage = Some(usage);
    }
    if event.done {
        round.tool_calls = std::mem::take(calls).finish()?;
    }
    Ok(event.done)
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
struct OpenAiErrorResponse {
    error: OpenAiError,
}

#[derive(Deserialize)]
struct OpenAiError {
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::ContentBlock;
    use crate::tool_contract::memory_tool_definitions;

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

    #[test]
    fn request_serializes_normalized_images_as_content_parts() {
        let mut request: ChatRequest = serde_json::from_str(concat!(
            r#"{"providerId":"openai","modelId":"gpt-example","messages":["#,
            r#"{"role":"user","content":[{"type":"text","text":"describe this"}]}]}"#,
        ))
        .unwrap();
        request.messages[0].content.push(ContentBlock::Image {
            media_type: crate::inference::types::ImageMediaType::Png,
            bytes: b"normalized-png".to_vec(),
        });

        let body = serde_json::to_value(OpenAiChatRequest::from(request)).unwrap();

        assert_eq!(body["messages"][0]["content"][0]["type"], "text");
        assert_eq!(body["messages"][0]["content"][1]["type"], "image_url");
        assert_eq!(
            body["messages"][0]["content"][1]["image_url"]["url"],
            "data:image/png;base64,bm9ybWFsaXplZC1wbmc="
        );
    }

    #[test]
    fn request_maps_closed_native_memory_definitions_into_openai_tools() {
        let request: ChatRequest = serde_json::from_value(serde_json::json!({
            "providerId": "openai",
            "modelId": "gpt-example",
            "messages": [{"role": "user", "content": [{"type": "text", "text": "recall this"}]}]
        }))
        .unwrap();

        let body = serde_json::to_value(OpenAiChatRequest::with_tools(
            request,
            memory_tool_definitions().into(),
        ))
        .unwrap();

        assert_eq!(body["tools"].as_array().map(Vec::len), Some(3));
        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["function"]["name"], "search_memory");
        assert_eq!(
            body["tools"][0]["function"]["parameters"]["additionalProperties"],
            false
        );
    }

    #[test]
    fn request_maps_the_clock_when_memory_and_web_are_both_disabled() {
        let request: ChatRequest = serde_json::from_value(serde_json::json!({
            "providerId": "openai",
            "modelId": "gpt-example",
            "messages": [{"role": "user", "content": [{"type": "text", "text": "time?"}]}]
        }))
        .unwrap();
        let body =
            serde_json::to_value(OpenAiToolSession::new(request, false).unwrap().request).unwrap();

        assert_eq!(body["tools"].as_array().map(Vec::len), Some(1));
        assert_eq!(body["tools"][0]["function"]["name"], "current_time");
    }

    #[test]
    fn request_maps_web_tools_after_memory_when_explicitly_enabled() {
        let mut request: ChatRequest = serde_json::from_value(serde_json::json!({
            "providerId": "openai",
            "modelId": "gpt-example",
            "messages": [{"role": "user", "content": [{"type": "text", "text": "search"}]}]
        }))
        .unwrap();
        request.memory_enabled = true;
        request.web_enabled = true;

        let body =
            serde_json::to_value(OpenAiToolSession::new(request, false).unwrap().request).unwrap();

        assert_eq!(body["tools"].as_array().map(Vec::len), Some(6));
        assert_eq!(body["tools"][3]["function"]["name"], "web_search");
        assert_eq!(body["tools"][4]["function"]["name"], "web_fetch");
        assert_eq!(body["tools"][5]["function"]["name"], "current_time");
        assert_eq!(
            body["tools"][3]["function"]["parameters"]["additionalProperties"],
            false
        );
        assert_eq!(
            body["tools"][4]["function"]["parameters"]["additionalProperties"],
            false
        );
    }

    #[test]
    fn reconstructs_fragmented_openai_calls_and_correlates_follow_up_results() {
        let mut calls = OpenAiToolCallAccumulator::default();
        for payload in [
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"search_memory","arguments":"{\"query\":"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"release\"}"}}]}}]}"#,
        ] {
            let event = decode_stream_payload(payload).unwrap();
            calls.extend(event.tool_call_deltas).unwrap();
        }
        let calls = calls.finish().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].call_id(), "call_1");
        assert_eq!(calls[0].tool_name(), "search_memory");
        assert_eq!(
            calls[0].arguments(),
            &serde_json::json!({"query": "release"})
        );

        let mut request: ChatRequest = serde_json::from_value(serde_json::json!({
            "providerId": "openai",
            "modelId": "gpt-example",
            "messages": [{"role": "user", "content": [{"type": "text", "text": "recall this"}]}]
        }))
        .unwrap();
        request.memory_enabled = true;
        let mut session = OpenAiToolSession::new(request, false).unwrap();
        session
            .append_results(
                OpenAiToolRound {
                    reasoning: String::new(),
                    content: String::new(),
                    tool_calls: calls,
                    usage: None,
                },
                vec![OpenAiToolResult {
                    tool_call_id: "call_1".into(),
                    content: r#"{"ok":true,"result":{}}"#.into(),
                }],
            )
            .unwrap();
        let body = serde_json::to_value(&session.request).unwrap();
        assert_eq!(body["messages"][1]["role"], "assistant");
        assert_eq!(body["messages"][1]["tool_calls"][0]["id"], "call_1");
        assert_eq!(body["messages"][2]["role"], "tool");
        assert_eq!(body["messages"][2]["tool_call_id"], "call_1");
    }
}
