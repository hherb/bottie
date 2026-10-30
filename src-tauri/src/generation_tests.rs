//! Native provider request reconstruction tests.

use super::*;
use crate::inference::{AudioMediaType, ContentBlock, ImageMediaType};
use crate::microphone::{CapturedAudio, CapturedAudioFormat};
use crate::storage::{
    ProviderAttachmentContext, ProviderContextImage, ProviderContextMessage, ProviderImageFormat,
    StoredRole,
};

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
        request_with_attachment_context(text_request("Describe this"), image_context(), true, None)
            .expect("vision request should prepare");

    assert!(matches!(
        &request.messages[0].content[1],
        ContentBlock::Image { media_type: ImageMediaType::Png, bytes } if bytes == b"normalized"
    ));
}

#[test]
fn adds_one_native_audio_block_only_after_audio_capability_confirmation() {
    let audio = CapturedAudio {
        format: CapturedAudioFormat::Wav,
        bytes: b"RIFFnative-wav".to_vec(),
        duration_ms: 750,
        sample_rate_hz: 16_000,
    };
    let request = request_with_attachment_context(
        text_request("Answer this recording"),
        ProviderAttachmentContext {
            messages: vec![ProviderContextMessage {
                role: StoredRole::User,
                text: "Answer this recording".into(),
                images: Vec::new(),
            }],
            current_request_has_image: false,
        },
        true,
        Some(audio),
    )
    .expect("audio-capable request should prepare");

    assert!(matches!(
        &request.messages[0].content[1],
        ContentBlock::Audio { media_type: AudioMediaType::Wav, bytes } if bytes == b"RIFFnative-wav"
    ));
}

#[test]
fn rejects_current_images_for_text_only_models() {
    let error = request_with_attachment_context(
        text_request("Describe this"),
        image_context(),
        false,
        None,
    )
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

    let request =
        request_with_attachment_context(text_request("Describe this"), context, false, None)
            .expect("historical images should not block a text-only request");

    assert_eq!(request.messages[0].content.len(), 1);
    assert!(matches!(
        request.messages[0].content[0],
        ContentBlock::Text { .. }
    ));
}

#[test]
fn rejects_webview_text_that_does_not_match_durable_context() {
    let error = request_with_attachment_context(
        text_request("Different text"),
        image_context(),
        true,
        None,
    )
    .expect_err("stale WebView context must be rejected");

    assert_eq!(error.code.as_str(), "invalid_request");
}

#[test]
fn removes_anthropic_sampling_before_provider_run_provenance() {
    let mut request = text_request("Hi");
    request.provider_id = "anthropic".into();
    request.model_id = "claude-sonnet-5".into();

    let normalized = normalize_provider_request(request);

    assert_eq!(normalized.settings.temperature, None);
}

#[test]
fn applies_saved_generation_limits_before_provider_run_provenance() {
    let mut request = text_request("Hi");
    request.settings.max_output_tokens = Some(1);
    let limits = crate::inference::GenerationLimits {
        max_tool_rounds: 16,
        max_tool_calls: 40,
        max_output_tokens: 12_288,
    };
    let request = request_with_generation_limits(request, limits);
    assert_eq!(request.settings.max_output_tokens, Some(12_288));
    assert_eq!(request.settings.generation_limits, limits);
}

#[test]
fn start_chat_writes_its_run_row_only_after_restore_lets_the_run_register() {
    use crate::image_generation::ImageGenerationRuns;
    use crate::run_cancellation::{ActiveRuns, lock_idle_generation};
    use crate::storage::{MessageState, NewStoredMessage};
    use futures_util::FutureExt;

    let path = std::env::temp_dir()
        .join("bottie-generation-tests")
        .join(format!("{}.sqlite3", uuid::Uuid::new_v4()));
    let store = ConversationStore::initialize(path).expect("fixture store should initialize");
    let conversation = store
        .create_conversation("Run order")
        .expect("conversation should create");
    let message = store
        .append_message_with_attachments(
            NewStoredMessage {
                conversation_id: conversation.id.clone(),
                role: StoredRole::User,
                text: "Hello".into(),
                reasoning: None,
                state: MessageState::Final,
                provider_id: None,
                model_id: None,
            },
            &[],
        )
        .expect("request should append");
    let context = ProviderRunContext {
        conversation_id: conversation.id,
        request_message_id: message.id,
    };
    let request = text_request("Hello");

    tauri::async_runtime::block_on(async {
        let runs = ActiveRuns::default();
        let image_runs = ImageGenerationRuns::default();
        // A restore past its last check holds the run registry until its store swap is done.
        let restore = lock_idle_generation(&runs, &image_runs)
            .await
            .expect("nothing is running");
        let mut start = Box::pin(reserve_chat_run(
            &runs,
            &store,
            "run-one".into(),
            &context,
            &request,
        ));
        assert!(
            (&mut start).now_or_never().is_none(),
            "the run must wait for restore"
        );
        // `recover_interrupted_runs` counts runs left in the running state: none may exist yet.
        assert_eq!(
            store.recover_interrupted_runs().unwrap(),
            0,
            "the row was written before registration"
        );

        drop(restore);
        let (_registration, _abort, _tools, _context) = start.await.expect("run should start");
        assert!(
            lock_idle_generation(&runs, &image_runs).await.is_none(),
            "the run is registered"
        );
        assert_eq!(
            store.recover_interrupted_runs().unwrap(),
            1,
            "the run row is written"
        );
    });
}
