//! OpenAI-compatible one-shot native-audio request tests.

use super::*;
use crate::inference::types::{AudioMediaType, ContentBlock, ImageMediaType};

fn audio_request(prompt: &str) -> ChatRequest {
    let mut request: ChatRequest = serde_json::from_value(serde_json::json!({
        "providerId": "openai",
        "modelId": "audio-model",
        "messages": [{"role": "user", "content": [{"type": "text", "text": prompt}]}]
    }))
    .unwrap();
    request.messages[0].content.push(ContentBlock::Audio {
        media_type: AudioMediaType::Wav,
        bytes: b"RIFFnative-wav".to_vec(),
    });
    request
}

#[test]
fn decodes_explicit_audio_capability_usage_and_reasoning() {
    let models = decode_model_list(
        br#"{"data":[{"id":"gpt-example","capabilities":["vision","audio","tools"]}]}"#,
    )
    .unwrap();
    assert_eq!(models[0].provider_id, "openai");
    assert!(models[0].capabilities.vision);
    assert!(models[0].capabilities.audio);
    assert!(models[0].capabilities.tools);
    let usage = decode_stream_payload(
        r#"{"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":7,"cost":0.004}}"#,
    )
    .unwrap();
    assert_eq!(usage.usage.and_then(|usage| usage.cost_usd), Some(0.004));
    let reasoning =
        decode_stream_payload(r#"{"choices":[{"delta":{"reasoning_content":"checking"}}]}"#)
            .unwrap();
    assert_eq!(reasoning.reasoning_delta, "checking");
}

#[test]
fn request_serializes_native_wav_as_an_openai_audio_part() {
    let body = serde_json::to_value(OpenAiChatRequest::from(audio_request(
        "answer this recording",
    )))
    .unwrap();

    assert_eq!(body["messages"][0]["content"][1]["type"], "input_audio");
    assert_eq!(
        body["messages"][0]["content"][1]["input_audio"]["format"],
        "wav"
    );
    assert_eq!(
        body["messages"][0]["content"][1]["input_audio"]["data"],
        "UklGRm5hdGl2ZS13YXY="
    );
}

#[test]
fn tool_follow_up_keeps_text_and_image_but_removes_one_shot_audio() {
    let mut request = audio_request("inspect this");
    request.memory_enabled = true;
    request.messages[0].content.insert(
        1,
        ContentBlock::Image {
            media_type: ImageMediaType::Png,
            bytes: b"normalized-png".to_vec(),
        },
    );
    let mut session = OpenAiToolSession::new(request).unwrap();
    let first = serde_json::to_value(&session.request).unwrap();
    assert_eq!(first["messages"][0]["content"][2]["type"], "input_audio");

    session
        .append_results(
            OpenAiToolRound {
                reasoning: String::new(),
                content: String::new(),
                tool_calls: vec![OpenAiToolCall::fixture(
                    "call_1",
                    "search_memory",
                    serde_json::json!({"query": "audio"}),
                )],
                usage: None,
            },
            vec![OpenAiToolResult {
                tool_call_id: "call_1".into(),
                content: r#"{"ok":true,"result":{}}"#.into(),
            }],
        )
        .unwrap();
    let follow_up = serde_json::to_value(&session.request).unwrap();
    let content = follow_up["messages"][0]["content"].as_array().unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[1]["type"], "image_url");
}
