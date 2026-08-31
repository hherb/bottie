//! oMLX one-shot native-audio tool-follow-up tests.

use super::*;
use crate::inference::types::{
    AudioMediaType, ChatRole, ChatSettings, ChatTurn, ContentBlock, ImageMediaType, ReasoningEffort,
};

fn audio_request() -> ChatRequest {
    ChatRequest {
        provider_id: PROVIDER_ID.into(),
        model_id: "audio-model".into(),
        messages: vec![ChatTurn {
            role: ChatRole::User,
            content: vec![
                ContentBlock::Text {
                    text: "inspect this".into(),
                },
                ContentBlock::Image {
                    media_type: ImageMediaType::Png,
                    bytes: b"normalized-png".to_vec(),
                },
                ContentBlock::Audio {
                    media_type: AudioMediaType::Wav,
                    bytes: b"RIFFnative-wav".to_vec(),
                },
            ],
        }],
        memory_enabled: true,
        web_enabled: false,
        email_enabled: false,
        audio_enabled: true,
        retain_audio: false,
        settings: ChatSettings {
            temperature: Some(0.0),
            max_output_tokens: Some(80),
            reasoning_effort: ReasoningEffort::Off,
        },
    }
}

#[test]
fn tool_follow_up_keeps_text_and_image_but_removes_one_shot_audio() {
    let mut session = OmlxToolSession::new(audio_request()).unwrap();
    let first = serde_json::to_value(&session.request).unwrap();
    assert_eq!(first["messages"][0]["content"][2]["type"], "input_audio");

    session
        .append_results(
            OmlxToolRound {
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
