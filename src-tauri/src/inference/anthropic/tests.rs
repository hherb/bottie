//! Anthropic protocol and request-shape tests.

use super::*;
use crate::{
    inference::{ContentBlock, ImageMediaType},
    tool_contract::{memory_tool_definitions, web_search_tool_definition},
};

#[test]
fn decodes_text_thinking_usage_and_completion() {
    let models = decode_model_list(
        br#"{"data":[{"id":"claude-example","capabilities":["vision","tools"]}]}"#,
    )
    .unwrap();
    assert!(models[0].capabilities.vision);
    assert!(models[0].capabilities.tools);
    let text = decode_stream_payload(concat!(
        r#"{"type":"content_block_delta","index":0,"delta":{"#,
        r#""type":"text_delta","text":"Hi"}}"#,
    ))
    .unwrap();
    assert_eq!(text.text_delta(), Some("Hi"));
    let reasoning = decode_stream_payload(concat!(
        r#"{"type":"content_block_delta","index":0,"delta":{"#,
        r#""type":"thinking_delta","thinking":"Check"}}"#,
    ))
    .unwrap();
    assert_eq!(reasoning.reasoning_delta(), Some("Check"));
    assert!(matches!(
        decode_stream_payload(r#"{"type":"message_stop"}"#).unwrap(),
        DecodedEvent::Done
    ));
}

#[test]
fn maps_closed_tools_and_fragmented_calls_into_correlated_messages() {
    let mut request = AnthropicChatRequest::with_tools(
        text_request("Recall the release note"),
        memory_tool_definitions().into(),
    );
    let initial = serde_json::to_value(&request).unwrap();
    assert_eq!(initial["tools"].as_array().map(Vec::len), Some(3));
    assert_eq!(initial["tools"][0]["name"], "search_memory");
    assert_eq!(
        initial["tools"][0]["input_schema"]["additionalProperties"],
        false
    );

    let round = fragmented_tool_round();
    assert_eq!(round.tool_calls.len(), 1);
    assert_eq!(round.tool_calls[0].call_id(), "toolu_1");
    assert_eq!(round.tool_calls[0].tool_name(), "search_memory");
    assert_eq!(round.tool_calls[0].arguments()["query"], "release");

    request
        .append_tool_exchange(
            round,
            vec![AnthropicToolResult {
                tool_use_id: "toolu_1".into(),
                content: r#"{"ok":true}"#.into(),
                is_error: false,
            }],
        )
        .unwrap();
    let follow_up = serde_json::to_value(request).unwrap();
    assert_eq!(follow_up["messages"][1]["role"], "assistant");
    assert_eq!(follow_up["messages"][1]["content"][0]["type"], "thinking");
    assert_eq!(
        follow_up["messages"][1]["content"][0]["signature"],
        "opaque-signature"
    );
    assert_eq!(follow_up["messages"][1]["content"][1]["type"], "tool_use");
    assert_eq!(follow_up["messages"][2]["role"], "user");
    assert_eq!(
        follow_up["messages"][2]["content"][0]["type"],
        "tool_result"
    );
    assert_eq!(
        follow_up["messages"][2]["content"][0]["tool_use_id"],
        "toolu_1"
    );
}

#[test]
fn maps_web_search_after_memory_only_when_explicitly_enabled() {
    let mut web_only = text_request("Find the current release");
    web_only.web_enabled = true;
    let web_only =
        serde_json::to_value(AnthropicToolSession::new(web_only).unwrap().request).unwrap();
    assert_eq!(web_only["tools"].as_array().map(Vec::len), Some(1));
    assert_eq!(web_only["tools"][0]["name"], "web_search");
    assert_eq!(
        web_only["tools"][0]["input_schema"],
        web_search_tool_definition().input_schema
    );

    let mut combined = text_request("Recall context and check the web");
    combined.memory_enabled = true;
    combined.web_enabled = true;
    let combined =
        serde_json::to_value(AnthropicToolSession::new(combined).unwrap().request).unwrap();
    assert_eq!(combined["tools"].as_array().map(Vec::len), Some(4));
    assert_eq!(combined["tools"][0]["name"], "search_memory");
    assert_eq!(combined["tools"][3]["name"], "web_search");
}

#[test]
fn rejects_non_object_arguments_and_mismatched_result_identity() {
    let mut malformed = AnthropicResponseAccumulator::default();
    for payload in [
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_1","name":"search_memory","input":{}}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"[]"}}"#,
    ] {
        malformed
            .apply(decode_stream_payload(payload).unwrap())
            .unwrap();
    }
    let error = malformed
        .apply(decode_stream_payload(r#"{"type":"content_block_stop","index":0}"#).unwrap())
        .expect_err("non-object arguments must fail");
    assert_eq!(error.code.as_str(), "malformed_response");

    let mut request = AnthropicChatRequest::from(text_request("Recall the release note"));
    let error = request
        .append_tool_exchange(
            fragmented_tool_round(),
            vec![AnthropicToolResult {
                tool_use_id: "different_call".into(),
                content: r#"{"ok":true}"#.into(),
                is_error: false,
            }],
        )
        .expect_err("mismatched provider identity must fail");
    assert_eq!(error.code.as_str(), "internal");
}

/// Reconstructs one signed-thinking plus fragmented-tool-use response round.
fn fragmented_tool_round() -> AnthropicToolRound {
    let mut response = AnthropicResponseAccumulator::default();
    for payload in [
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"checking"}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"opaque-signature"}}"#,
        r#"{"type":"content_block_stop","index":0}"#,
        r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"search_memory","input":{}}}"#,
        r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"query\":"}}"#,
        r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"\"release\"}"}}"#,
        r#"{"type":"content_block_stop","index":1}"#,
        r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":7}}"#,
        r#"{"type":"message_stop"}"#,
    ] {
        response
            .apply(decode_stream_payload(payload).unwrap())
            .unwrap();
    }
    response.finish().unwrap()
}

/// Builds one text-only Anthropic request for pure request-shape tests.
fn text_request(text: &str) -> ChatRequest {
    serde_json::from_value(serde_json::json!({
        "providerId": "anthropic",
        "modelId": "claude-example",
        "messages": [{"role": "user", "content": [{"type": "text", "text": text}]}]
    }))
    .unwrap()
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

#[test]
fn request_serializes_normalized_images_as_anthropic_source_blocks() {
    let mut request: ChatRequest = serde_json::from_str(concat!(
        r#"{"providerId":"anthropic","modelId":"claude-example","messages":["#,
        r#"{"role":"user","content":[{"type":"text","text":"describe this"}]}]}"#,
    ))
    .unwrap();
    request.messages[0].content.push(ContentBlock::Image {
        media_type: ImageMediaType::Jpeg,
        bytes: b"normalized-jpeg".to_vec(),
    });

    let body = serde_json::to_value(AnthropicChatRequest::from(request)).unwrap();

    assert_eq!(body["messages"][0]["content"][0]["type"], "text");
    assert_eq!(body["messages"][0]["content"][1]["type"], "image");
    assert_eq!(
        body["messages"][0]["content"][1]["source"]["media_type"],
        "image/jpeg"
    );
    assert_eq!(
        body["messages"][0]["content"][1]["source"]["data"],
        "bm9ybWFsaXplZC1qcGVn"
    );
}
