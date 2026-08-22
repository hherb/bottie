use std::sync::{Arc, Mutex};

use futures_util::{FutureExt, future::Abortable};

use super::*;
use crate::inference::types::{
    ChatSettings, ChatTurn, ContentBlock, ImageMediaType, ReasoningEffort,
};

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

    fn reasoning_delta(&self, _delta: String) -> Result<(), ProviderError> {
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
        memory_enabled: false,
        settings: ChatSettings {
            temperature: Some(0.0),
            max_output_tokens: Some(80),
            reasoning_effort: ReasoningEffort::Off,
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
    let fixture = concat!(
        r#"{"object":"list","data":[{"id":"Qwen3.6-35B-A3B-8bit","#,
        r#""object":"model","max_model_len":262144,"capabilities":["vision"]}]}"#,
    );
    let models = decode_model_list(fixture.as_bytes()).expect("model list should decode");
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].model_id, "Qwen3.6-35B-A3B-8bit");
    assert_eq!(models[0].max_context_tokens, Some(262_144));
    assert!(models[0].capabilities.streaming);
    assert!(models[0].capabilities.vision);
}

#[test]
fn enriches_catalogue_from_explicit_vlm_status_metadata() {
    let catalogue = br#"{
        "object":"list",
        "data":[
            {"id":"Qwen3.8-27B-8bit","max_model_len":262144},
            {"id":"gemma-4-26b-a4b-it-4bit","max_model_len":262144},
            {"id":"LFM2.5-8B-A1B-MLX-8bit","max_model_len":128000}
        ]
    }"#;
    let status = br#"{
        "models":[
            {
                "id":"Qwen3.8-27B-8bit",
                "loaded":true,
                "engine_type":"vlm",
                "model_type":"vlm"
            },
            {
                "id":"gemma-4-26b-a4b-it-4bit",
                "loaded":false,
                "engine_type":"vlm",
                "model_type":"vlm"
            },
            {
                "id":"LFM2.5-8B-A1B-MLX-8bit",
                "loaded":false,
                "engine_type":"batched",
                "model_type":"llm"
            }
        ]
    }"#;

    let mut models = decode_model_list(catalogue).expect("catalogue should decode");
    let statuses = decode_model_status(status).expect("status should decode");
    enrich_models(&mut models, &statuses);

    assert!(models[0].capabilities.vision);
    assert_eq!(models[0].load_state, ModelLoadState::Loaded);
    assert!(models[1].capabilities.vision);
    assert_eq!(models[1].load_state, ModelLoadState::Unloaded);
    assert!(!models[2].capabilities.vision);
    assert_eq!(models[2].load_state, ModelLoadState::Unloaded);
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
        DecodedEvent::TextDelta(ref delta) if delta == "hello"
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
            output_tokens: Some(7),
            cost_usd: None
        })
    ));
}

#[test]
fn decodes_reasoning_separately_from_answer_text() {
    let event = decode_stream_payload(
        r#"{"choices":[{"delta":{"reasoning_content":"checking assumptions"}}]}"#,
    )
    .unwrap();
    assert!(matches!(
        event,
        DecodedEvent::ReasoningDelta(ref delta) if delta == "checking assumptions"
    ));
}

#[test]
fn sends_explicit_off_and_low_reasoning_controls() {
    let mut request = live_request("model".into(), "hello");
    let off = serde_json::to_value(OmlxChatRequest::from(request.clone())).unwrap();
    assert_eq!(off["chat_template_kwargs"]["enable_thinking"], false);
    assert!(off.get("reasoning_effort").is_none());

    request.settings.reasoning_effort = ReasoningEffort::Low;
    let low = serde_json::to_value(OmlxChatRequest::from(request)).unwrap();
    assert_eq!(low["chat_template_kwargs"]["enable_thinking"], true);
    assert_eq!(low["reasoning_effort"], "low");
}

#[test]
fn serializes_normalized_images_as_openai_content_parts() {
    let mut request = live_request("vision-model".into(), "describe this");
    request.messages[0].content.push(ContentBlock::Image {
        media_type: ImageMediaType::Jpeg,
        bytes: b"normalized-jpeg".to_vec(),
    });

    let body = serde_json::to_value(OmlxChatRequest::from(request)).unwrap();

    assert_eq!(body["messages"][0]["content"][0]["type"], "text");
    assert_eq!(body["messages"][0]["content"][1]["type"], "image_url");
    assert_eq!(
        body["messages"][0]["content"][1]["image_url"]["url"],
        "data:image/jpeg;base64,bm9ybWFsaXplZC1qcGVn"
    );
}

#[test]
fn default_generation_settings_are_bounded_and_disable_reasoning() {
    let settings = ChatSettings::default();
    assert_eq!(settings.max_output_tokens, Some(4_096));
    assert_eq!(settings.reasoning_effort, ReasoningEffort::Off);
}

#[test]
fn reasoning_only_ipc_settings_keep_safe_generation_defaults() {
    let request: ChatRequest = serde_json::from_str(
        r#"{
            "providerId":"omlx",
            "modelId":"model",
            "messages":[{"role":"user","content":[{"type":"text","text":"hello"}]}],
            "settings":{"reasoningEffort":"low"}
        }"#,
    )
    .unwrap();
    assert_eq!(request.settings.max_output_tokens, Some(4_096));
    assert_eq!(request.settings.reasoning_effort, ReasoningEffort::Low);
}

#[test]
fn rejects_malformed_event() {
    let error = decode_stream_payload("not json").unwrap_err();
    assert_eq!(error.code, ProviderErrorCode::MalformedResponse);
}

#[test]
fn decodes_provider_error_body() {
    let body: OmlxErrorResponse =
        serde_json::from_str(r#"{"error":{"message":"Model was not found","type":"not_found"}}"#)
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
