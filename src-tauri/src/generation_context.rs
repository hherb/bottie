//! Native reconstruction of provider context from durable text, images, and one explicit capture.

use crate::{
    inference::{
        AudioMediaType, ChatRequest, ChatRole, ChatTurn, ContentBlock, ImageMediaType,
        ProviderError,
    },
    microphone::{CapturedAudio, CapturedAudioError, CapturedAudioFormat},
    storage::{ProviderAttachmentContext, ProviderImageFormat, StoredRole},
};

/// Removes provider-neutral sampling defaults that Anthropic may reject before provenance is recorded.
pub(crate) fn normalize_provider_request(mut request: ChatRequest) -> ChatRequest {
    if request.provider_id == "anthropic" {
        request.settings.temperature = None;
    }
    request
}

/// Reconciles WebView text with durable selected-lineage context and native-only media.
pub(crate) fn request_with_attachment_context(
    mut request: ChatRequest,
    context: ProviderAttachmentContext,
    supports_vision: bool,
    captured_audio: Option<CapturedAudio>,
) -> Result<ChatRequest, ProviderError> {
    let provided_request = request
        .messages
        .iter()
        .rev()
        .find(|turn| turn.role == ChatRole::User)
        .map(text_for_turn);
    let durable_request = context
        .messages
        .iter()
        .rev()
        .find(|message| message.role == StoredRole::User)
        .map(|message| message.text.as_str());
    if provided_request.as_deref() != durable_request {
        return Err(ProviderError::invalid_request(
            "The provider request no longer matches the selected conversation branch.",
        ));
    }
    if context.current_request_has_image && !supports_vision {
        return Err(ProviderError::invalid_request(
            "The selected model is text-only. Choose a vision model or remove the image.",
        ));
    }
    let last_user_index = context
        .messages
        .iter()
        .rposition(|message| message.role == StoredRole::User);
    request.messages = context
        .messages
        .into_iter()
        .enumerate()
        .map(|(index, message)| -> Result<ChatTurn, ProviderError> {
            let mut content = vec![ContentBlock::Text { text: message.text }];
            if supports_vision {
                for image in message.images {
                    content.push(ContentBlock::Image {
                        media_type: match image.format {
                            ProviderImageFormat::Jpeg => ImageMediaType::Jpeg,
                            ProviderImageFormat::Png => ImageMediaType::Png,
                        },
                        bytes: image.bytes.ok_or_else(|| {
                            ProviderError::internal(
                                "A normalized image was unavailable for provider delivery.",
                                None,
                            )
                        })?,
                    });
                }
            }
            if Some(index) == last_user_index
                && let Some(audio) = captured_audio.as_ref()
            {
                content.push(ContentBlock::Audio {
                    media_type: match audio.format {
                        CapturedAudioFormat::Wav => AudioMediaType::Wav,
                    },
                    bytes: audio.bytes.clone(),
                });
            }
            Ok(ChatTurn {
                role: chat_role(message.role),
                content,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(request)
}

/// Maps capture preparation failures into one fixed provider-facing request error.
pub(crate) fn captured_audio_error(_error: CapturedAudioError) -> ProviderError {
    ProviderError::invalid_request(
        "The stopped recording is unavailable. Record again before sending audio.",
    )
}

/// Joins WebView-supplied text while native image and audio variants remain impossible to deserialize.
fn text_for_turn(turn: &ChatTurn) -> String {
    turn.content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::Image { .. } | ContentBlock::Audio { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Maps the durable two-role schema into provider-neutral chat roles.
fn chat_role(role: StoredRole) -> ChatRole {
    match role {
        StoredRole::User => ChatRole::User,
        StoredRole::Assistant => ChatRole::Assistant,
    }
}
