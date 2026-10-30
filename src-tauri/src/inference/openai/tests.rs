//! Focused OpenAI-compatible request mapping, error normalization, and stream limit tests.

use super::*;
use crate::inference::ContentBlock;
use crate::tool_contract::memory_tool_definitions;

#[test]
fn long_provider_error_messages_are_truncated_before_reaching_the_webview() {
    let body = serde_json::json!({"error": {"message": "k".repeat(5_000)}}).to_string();
    let error = normalize_response_error(StatusCode::UNAUTHORIZED, &body);
    assert_eq!(error.message.chars().count(), 301);
    assert!(error.message.ends_with('…'));
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
    assert_eq!(body["max_completion_tokens"], 8192);
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

/// Accepts every stream callback; the limit tests only inspect the returned error.
struct NoopSink;

impl StreamSink for NoopSink {
    fn text_delta(&self, _delta: String) -> Result<(), ProviderError> {
        Ok(())
    }

    fn reasoning_delta(&self, _delta: String) -> Result<(), ProviderError> {
        Ok(())
    }

    fn usage_updated(&self, _usage: Usage) -> Result<(), ProviderError> {
        Ok(())
    }
}

fn text_event(reasoning: String, text: String) -> DecodedStreamEvent {
    DecodedStreamEvent {
        text_delta: text,
        reasoning_delta: reasoning,
        tool_call_deltas: Vec::new(),
        usage: None,
        done: false,
    }
}

#[test]
fn rejects_a_round_whose_reasoning_and_text_together_pass_the_text_limit() {
    use crate::inference::limits::MAX_ROUND_TEXT_BYTES;

    let mut round = OpenAiToolRound {
        reasoning: String::new(),
        content: String::new(),
        tool_calls: Vec::new(),
        usage: None,
    };
    let mut calls = OpenAiToolCallAccumulator::default();
    let half = MAX_ROUND_TEXT_BYTES / 2;
    process_stream_event(
        text_event("x".repeat(half), String::new()),
        &NoopSink,
        &mut round,
        &mut calls,
        false,
    )
    .unwrap();

    let error = process_stream_event(
        text_event(String::new(), "x".repeat(MAX_ROUND_TEXT_BYTES - half + 1)),
        &NoopSink,
        &mut round,
        &mut calls,
        false,
    )
    .expect_err("one byte past the round limit must fail");
    assert_eq!(error.code.as_str(), "malformed_response");
}
