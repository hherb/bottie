//! Anthropic protocol and request-shape tests.

use super::*;
use crate::inference::{ContentBlock, ImageMediaType};

#[test]
fn decodes_text_thinking_usage_and_completion() {
    let models =
        decode_model_list(br#"{"data":[{"id":"claude-example","capabilities":["vision"]}]}"#)
            .unwrap();
    assert!(models[0].capabilities.vision);
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
