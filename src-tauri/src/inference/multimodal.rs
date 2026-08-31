//! Shared OpenAI-shaped multimodal request serialization.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;

use super::types::ContentBlock;

/// String content for text-only turns or ordered parts for multimodal turns.
#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum OpenAiContent {
    /// Compact legacy representation retained for text-only requests.
    Text(String),
    /// Ordered content parts used when a turn contains native media.
    Parts(Vec<OpenAiContentPart>),
}

impl OpenAiContent {
    /// Removes one-shot audio before a provider tool follow-up while retaining text and images.
    pub(super) fn remove_audio(&mut self) {
        if let Self::Parts(parts) = self {
            parts.retain(|part| !matches!(part, OpenAiContentPart::InputAudio { .. }));
        }
    }
}

/// One OpenAI Chat Completions content part.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum OpenAiContentPart {
    /// Plain text content.
    Text { text: String },
    /// Inline data URL accepted by OpenAI-shaped vision endpoints.
    ImageUrl { image_url: OpenAiImageUrl },
    /// Inline base64 WAV accepted by audio-capable Chat Completions models.
    InputAudio { input_audio: OpenAiInputAudio },
}

/// String content for Anthropic text-only turns or ordered multimodal blocks.
#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum AnthropicContent {
    /// Compact text-only representation.
    Text(String),
    /// Ordered content blocks used when a turn contains an image.
    Blocks(Vec<AnthropicContentBlock>),
}

/// One Anthropic Messages content block.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum AnthropicContentBlock {
    /// Plain text content.
    Text { text: String },
    /// Inline normalized image source.
    Image { source: AnthropicImageSource },
}

/// Base64 source object required by Anthropic Messages.
#[derive(Serialize)]
pub(super) struct AnthropicImageSource {
    #[serde(rename = "type")]
    source_type: &'static str,
    media_type: &'static str,
    data: String,
}

/// Nested image URL object required by Chat Completions.
#[derive(Serialize)]
pub(super) struct OpenAiImageUrl {
    url: String,
}

/// Nested audio object required by OpenAI-shaped Chat Completions.
#[derive(Serialize)]
pub(super) struct OpenAiInputAudio {
    data: String,
    format: &'static str,
}

/// Converts native blocks while retaining the compact string shape for text-only turns.
pub(super) fn openai_content(blocks: Vec<ContentBlock>) -> OpenAiContent {
    if blocks
        .iter()
        .all(|block| matches!(block, ContentBlock::Text { .. }))
    {
        return OpenAiContent::Text(text_content(blocks));
    }
    OpenAiContent::Parts(
        blocks
            .into_iter()
            .map(|block| match block {
                ContentBlock::Text { text } => OpenAiContentPart::Text { text },
                ContentBlock::Image { media_type, bytes } => OpenAiContentPart::ImageUrl {
                    image_url: OpenAiImageUrl {
                        url: format!(
                            "data:{};base64,{}",
                            media_type.as_mime_type(),
                            STANDARD.encode(bytes)
                        ),
                    },
                },
                ContentBlock::Audio { bytes, .. } => OpenAiContentPart::InputAudio {
                    input_audio: OpenAiInputAudio {
                        data: STANDARD.encode(bytes),
                        format: "wav",
                    },
                },
            })
            .collect(),
    )
}

/// Converts native blocks while retaining string content for text-only Anthropic turns.
pub(super) fn anthropic_content(blocks: Vec<ContentBlock>) -> AnthropicContent {
    if blocks
        .iter()
        .all(|block| matches!(block, ContentBlock::Text { .. }))
    {
        return AnthropicContent::Text(text_content(blocks));
    }
    AnthropicContent::Blocks(
        blocks
            .into_iter()
            .map(|block| match block {
                ContentBlock::Text { text } => AnthropicContentBlock::Text { text },
                ContentBlock::Image { media_type, bytes } => AnthropicContentBlock::Image {
                    source: AnthropicImageSource {
                        source_type: "base64",
                        media_type: media_type.as_mime_type(),
                        data: base64_image(&bytes),
                    },
                },
                ContentBlock::Audio { .. } => AnthropicContentBlock::Text {
                    text: "[Audio input is unavailable on this provider.]".into(),
                },
            })
            .collect(),
    )
}

/// Joins every text block and deliberately ignores native image bytes.
pub(super) fn text_content(blocks: Vec<ContentBlock>) -> String {
    blocks
        .into_iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text),
            ContentBlock::Image { .. } => None,
            ContentBlock::Audio { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Base64-encodes normalized bytes for provider-native image fields.
pub(super) fn base64_image(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}
