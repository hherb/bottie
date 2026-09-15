//! Durable terminal orchestration for one freshly verified local worker run.

use tauri::ipc::Channel;

use super::{ImageGenerationEvent, ImageGenerationRun, ImageGenerationStage, storage_error};
use crate::{
    diagnostics::{Diagnostics, record_diagnostic, sanitized},
    image_generation::{
        ImageGenerationRequest, ImageGenerationRuns, LocalImageGenerator, QWEN_IMAGE_PROVIDER_ID,
        local::LocalImageGenerationError,
    },
    inference::ProviderError,
    local_image_worker::execution::VerifiedLocalImageInstallation,
    storage::{ConversationStore, GeneratedAssetStatus, StoredMessage},
};

/// Spawns one freshly verified private-worker run and returns its path-free accepted state.
#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_local_image_run(
    runs: ImageGenerationRuns,
    generator: LocalImageGenerator,
    conversations: ConversationStore,
    diagnostics: Diagnostics,
    run_id: String,
    cancellation: tokio::sync::watch::Receiver<bool>,
    pending_message: StoredMessage,
    output_count: u8,
    generation_request: ImageGenerationRequest,
    installation: VerifiedLocalImageInstallation,
    seed: u64,
    on_event: Channel<ImageGenerationEvent>,
) -> ImageGenerationRun {
    let accepted = ImageGenerationRun {
        run_id: run_id.clone(),
        message: pending_message.clone(),
    };
    let task_run_id = run_id.clone();
    let message_id = pending_message.id.clone();
    tauri::async_runtime::spawn(async move {
        let _ = on_event.send(ImageGenerationEvent::Started {
            run_id: task_run_id.clone(),
            message: pending_message,
        });
        send_progress(&on_event, &task_run_id, output_count);
        let progress_run_id = task_run_id.clone();
        let progress_events = on_event.clone();
        let result = generator
            .generate(
                installation,
                generation_request,
                seed,
                conversations.clone(),
                cancellation,
                move || send_progress(&progress_events, &progress_run_id, output_count),
            )
            .await;
        match result {
            Ok(images) => {
                match conversations.complete_generated_image_message(&message_id, &images) {
                    Ok(message) => {
                        let _ = on_event.send(ImageGenerationEvent::Completed {
                            run_id: task_run_id.clone(),
                            message,
                        });
                        record_diagnostic(
                            &diagnostics,
                            "info",
                            "Local image generation completed",
                            Some(QWEN_IMAGE_PROVIDER_ID),
                            Some("Verified local PNG bytes reached app-private storage"),
                        )
                        .await;
                    }
                    Err(error) => {
                        finish_failure(
                            &conversations,
                            &diagnostics,
                            &on_event,
                            &task_run_id,
                            &message_id,
                            storage_error(error),
                        )
                        .await;
                    }
                }
            }
            Err(LocalImageGenerationError::Cancelled) => {
                finish_cancellation(
                    &conversations,
                    &diagnostics,
                    &on_event,
                    &task_run_id,
                    &message_id,
                )
                .await;
            }
            Err(LocalImageGenerationError::Failed(error)) => {
                finish_failure(
                    &conversations,
                    &diagnostics,
                    &on_event,
                    &task_run_id,
                    &message_id,
                    error,
                )
                .await;
            }
        }
        runs.finish(&task_run_id).await;
    });
    accepted
}

fn send_progress(on_event: &Channel<ImageGenerationEvent>, run_id: &str, output_count: u8) {
    let _ = on_event.send(ImageGenerationEvent::Progress {
        run_id: run_id.to_owned(),
        stage: ImageGenerationStage::Generating,
        total: output_count,
    });
}

async fn finish_cancellation(
    conversations: &ConversationStore,
    diagnostics: &Diagnostics,
    on_event: &Channel<ImageGenerationEvent>,
    run_id: &str,
    message_id: &str,
) {
    match conversations.fail_generated_image_message(
        message_id,
        GeneratedAssetStatus::Cancelled,
        None,
    ) {
        Ok(message) => {
            let _ = on_event.send(ImageGenerationEvent::Cancelled {
                run_id: run_id.to_owned(),
                message,
            });
        }
        Err(error) => {
            let _ = on_event.send(ImageGenerationEvent::Failed {
                run_id: run_id.to_owned(),
                message: None,
                error: storage_error(error),
            });
        }
    }
    record_diagnostic(
        diagnostics,
        "info",
        "Local image generation cancelled",
        Some(QWEN_IMAGE_PROVIDER_ID),
        Some("Private worker outputs and uncommitted normalized bytes were discarded"),
    )
    .await;
}

async fn finish_failure(
    conversations: &ConversationStore,
    diagnostics: &Diagnostics,
    on_event: &Channel<ImageGenerationEvent>,
    run_id: &str,
    message_id: &str,
    error: ProviderError,
) {
    let error = sanitized(error);
    let message = conversations
        .fail_generated_image_message(
            message_id,
            GeneratedAssetStatus::Failed,
            Some(error.code.as_str()),
        )
        .ok();
    let _ = on_event.send(ImageGenerationEvent::Failed {
        run_id: run_id.to_owned(),
        message,
        error: error.clone(),
    });
    record_diagnostic(
        diagnostics,
        "error",
        "Local image generation failed",
        Some(QWEN_IMAGE_PROVIDER_ID),
        error.diagnostic.as_deref().or(Some(&error.message)),
    )
    .await;
}
