//! Native provider request reconstruction tests.

use super::*;
use crate::storage::{ProviderContextImage, ProviderContextMessage};

/// Builds the WebView text request matched by each durable context fixture.
fn text_request(text: &str) -> ChatRequest {
    serde_json::from_value(serde_json::json!({
        "providerId": "ollama",
        "modelId": "vision-model",
        "messages": [{"role": "user", "content": [{"type": "text", "text": text}]}]
    }))
    .expect("text request should deserialize")
}

/// Builds one current user turn with normalized native-only bytes.
fn image_context() -> ProviderAttachmentContext {
    ProviderAttachmentContext {
        messages: vec![ProviderContextMessage {
            role: StoredRole::User,
            text: "Describe this".into(),
            images: vec![ProviderContextImage {
                format: ProviderImageFormat::Png,
                sha256: "normalized".repeat(4),
                byte_size: 10,
                bytes: Some(b"normalized".to_vec()),
            }],
        }],
        current_request_has_image: true,
    }
}

#[test]
fn adds_native_images_only_after_vision_capability_confirmation() {
    let request =
        request_with_attachment_context(text_request("Describe this"), image_context(), true)
            .expect("vision request should prepare");

    assert!(matches!(
        &request.messages[0].content[1],
        ContentBlock::Image { media_type: ImageMediaType::Png, bytes } if bytes == b"normalized"
    ));
}

#[test]
fn rejects_current_images_for_text_only_models() {
    let error =
        request_with_attachment_context(text_request("Describe this"), image_context(), false)
            .expect_err("text-only request must be rejected");

    assert_eq!(error.code.as_str(), "invalid_request");
    assert_eq!(
        error.message,
        "The selected model is text-only. Choose a vision model or remove the image."
    );
}

#[test]
fn omits_unloaded_historical_images_for_text_only_models() {
    let mut context = image_context();
    context.current_request_has_image = false;
    context.messages[0].images[0].bytes = None;

    let request = request_with_attachment_context(text_request("Describe this"), context, false)
        .expect("historical images should not block a text-only request");

    assert_eq!(request.messages[0].content.len(), 1);
    assert!(matches!(
        request.messages[0].content[0],
        ContentBlock::Text { .. }
    ));
}

#[test]
fn rejects_webview_text_that_does_not_match_durable_context() {
    let error =
        request_with_attachment_context(text_request("Different text"), image_context(), true)
            .expect_err("stale WebView context must be rejected");

    assert_eq!(error.code.as_str(), "invalid_request");
}
