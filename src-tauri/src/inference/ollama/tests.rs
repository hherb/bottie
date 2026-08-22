use std::sync::{Arc, Mutex};

use futures_util::{FutureExt, future::Abortable};

use super::protocol::{
    NdjsonDecoder, OllamaChatRequest, OllamaShowResponse, OllamaToolResult, capability_map,
    decode_model_list, decode_stream_line, model_info,
};
use super::*;
use crate::inference::types::{
    ChatRole, ChatSettings, ChatTurn, ContentBlock, ImageMediaType, ModelLoadState, ReasoningEffort,
};
use crate::tool_contract::{
    memory_tool_definitions, web_fetch_tool_definition, web_search_tool_definition,
};

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

    fn reasoning_delta(&self, _delta: String) -> Result<(), ProviderError> {
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
        memory_enabled: false,
        web_enabled: false,
        settings: ChatSettings {
            temperature: Some(0.0),
            max_output_tokens: Some(80),
            reasoning_effort: ReasoningEffort::Off,
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
    let fixture = concat!(
        r#"{"models":[{"name":"gemma3:4b","model":"gemma3:4b","capabilities":["#,
        r#""completion","tools","vision","embedding"],"details":{"context_length":131072}}]}"#,
    );
    let listed = decode_model_list(fixture.as_bytes()).unwrap();
    assert_eq!(listed.models.len(), 1);
    let details: OllamaShowResponse = serde_json::from_str(
        r#"{"capabilities":[],"model_info":{"gemma3.context_length":131072}}"#,
    )
    .unwrap();
    let running_context = Some(4_096);
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
        .push(
            b"lo\"},\"done\":false}\n{\"message\":{\"content\":\"\"},\"done\":true,\
              \"prompt_eval_count\":12,\"eval_count\":7}\n",
        )
        .unwrap();
    assert_eq!(lines.len(), 2);
    let delta = decode_stream_line(&lines[0]).unwrap();
    assert_eq!(delta.text_delta, "hello");
    let completed = decode_stream_line(&lines[1]).unwrap();
    assert!(completed.done);
    assert_eq!(completed.prompt_eval_count, Some(12));
    assert_eq!(completed.eval_count, Some(7));
}

#[test]
fn decodes_thinking_separately_from_answer_text() {
    let event = decode_stream_line(
        r#"{"message":{"thinking":"checking assumptions","content":""},"done":false}"#,
    )
    .unwrap();
    assert_eq!(event.reasoning_delta, "checking assumptions");
    assert!(event.text_delta.is_empty());
}

#[test]
fn sends_explicit_off_and_low_reasoning_controls() {
    let mut request = live_request("model".into(), "hello");
    let off = serde_json::to_value(OllamaChatRequest::from(request.clone())).unwrap();
    assert_eq!(off["think"], false);

    request.settings.reasoning_effort = ReasoningEffort::Low;
    let low = serde_json::to_value(OllamaChatRequest::from(request)).unwrap();
    assert_eq!(low["think"], "low");
}

#[test]
fn sends_normalized_image_bytes_only_on_the_owning_ollama_turn() {
    let mut request = live_request("vision-model".into(), "describe this image");
    request.messages[0].content.push(ContentBlock::Image {
        media_type: ImageMediaType::Png,
        bytes: b"normalized-png".to_vec(),
    });
    request.messages.push(ChatTurn {
        role: ChatRole::Assistant,
        content: vec![ContentBlock::Text {
            text: "Earlier answer".into(),
        }],
    });

    let body = serde_json::to_value(OllamaChatRequest::from(request)).unwrap();

    assert_eq!(body["messages"][0]["content"], "describe this image");
    assert_eq!(
        body["messages"][0]["images"],
        serde_json::json!(["bm9ybWFsaXplZC1wbmc="])
    );
    assert!(body["messages"][1].get("images").is_none());
}

#[test]
fn maps_closed_native_memory_definitions_into_ollama_tools() {
    let body = serde_json::to_value(OllamaChatRequest::with_tools(
        live_request("tool-model".into(), "find the release note"),
        memory_tool_definitions(),
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
fn maps_the_closed_web_definitions_after_memory_when_explicitly_enabled() {
    let definitions = memory_tool_definitions()
        .into_iter()
        .chain([web_search_tool_definition(), web_fetch_tool_definition()])
        .collect::<Vec<_>>();
    let body = serde_json::to_value(OllamaChatRequest::with_tools(
        live_request("tool-model".into(), "find today's release note"),
        definitions,
    ))
    .unwrap();

    assert_eq!(body["tools"].as_array().map(Vec::len), Some(5));
    assert_eq!(body["tools"][3]["function"]["name"], "web_search");
    assert_eq!(
        body["tools"][3]["function"]["parameters"]["additionalProperties"],
        false
    );
    assert_eq!(body["tools"][4]["function"]["name"], "web_fetch");
    assert_eq!(
        body["tools"][4]["function"]["parameters"]["additionalProperties"],
        false
    );
}

#[test]
fn decodes_streamed_calls_and_maps_ordered_results_into_follow_up_messages() {
    let event = decode_stream_line(
        r#"{"message":{"thinking":"checking memory","content":"","tool_calls":[{"type":"function","function":{"index":0,"name":"search_memory","arguments":{"query":"release"}}},{"type":"function","function":{"index":1,"name":"search_attached_files","arguments":{"query":"checklist"}}}]},"done":true}"#,
    )
    .unwrap();
    assert_eq!(event.tool_calls.len(), 2);
    assert_eq!(event.tool_calls[0].tool_name(), "search_memory");

    let mut request = OllamaChatRequest::with_tools(
        live_request("tool-model".into(), "find the release note"),
        memory_tool_definitions(),
    );
    request
        .append_tool_exchange(
            event.reasoning_delta,
            event.text_delta,
            event.tool_calls,
            vec![
                OllamaToolResult {
                    tool_name: "search_memory".into(),
                    content: r#"{"ok":true,"result":{"matches":[]}}"#.into(),
                },
                OllamaToolResult {
                    tool_name: "search_attached_files".into(),
                    content: r#"{"ok":false,"error":{"code":"unavailable"}}"#.into(),
                },
            ],
        )
        .expect("matching results should append");
    let body = serde_json::to_value(request).unwrap();

    assert_eq!(body["messages"][1]["role"], "assistant");
    assert_eq!(body["messages"][1]["thinking"], "checking memory");
    assert_eq!(
        body["messages"][1]["tool_calls"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(body["messages"][2]["role"], "tool");
    assert_eq!(body["messages"][2]["tool_name"], "search_memory");
    assert_eq!(body["messages"][3]["tool_name"], "search_attached_files");
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
