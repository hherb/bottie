//! One-active-run image generation, cancellation, progress, and durable terminal orchestration.

use std::sync::Arc;

use futures_util::future::{AbortHandle, Abortable};
use serde::{Deserialize, Serialize};
use tauri::{State, ipc::Channel};

use crate::{
    AppState,
    diagnostics::{record_diagnostic, sanitized},
    inference::ProviderError,
    storage::{
        GeneratedAssetExecution, GeneratedAssetStatus, GeneratedImageProvenance, StorageError,
        StoredMessage,
    },
};

use super::{
    DASHSCOPE_QWEN_IMAGE_MODEL_ID, DashScopeQwenImageProvider, GeneratedImageDownloader,
    ImageGenerationProvider, ImageGenerationRequest, QWEN_IMAGE_PROVIDER_ID,
};

/// Process-wide one-active-image-run registry.
#[derive(Clone, Default)]
pub(crate) struct ImageGenerationRuns {
    active: Arc<tauri::async_runtime::Mutex<Option<ActiveImageRun>>>,
}

/// Cancellation handle for the one accepted image generation.
struct ActiveImageRun {
    run_id: String,
    abort_handle: AbortHandle,
}

/// Explicit path-free image generation request accepted from the composer.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StartImageGenerationRequest {
    /// Existing durable conversation that owns the assistant image message.
    pub(crate) conversation_id: String,
    /// User-authored image description kept behind the native provider boundary.
    pub(crate) prompt: String,
    /// Explicit requested output width.
    pub(crate) width: u32,
    /// Explicit requested output height.
    pub(crate) height: u32,
    /// Exact requested output count.
    pub(crate) count: u8,
    /// Explicit execution backend; this slice accepts cloud only.
    pub(crate) execution: GeneratedAssetExecution,
}

/// Opaque accepted run identity and already-durable pending assistant message.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageGenerationRun {
    /// Bottie-owned cancellation identity with no provider correlation value.
    pub(crate) run_id: String,
    /// Pending path-free assistant message inserted before provider I/O begins.
    pub(crate) message: StoredMessage,
}

/// Bounded progress stages emitted without URLs, paths, credentials, or provider identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ImageGenerationStage {
    /// Waiting for the hosted model to return temporary image references.
    Generating,
    /// Downloading and validating native-only temporary results.
    Downloading,
}

/// Path-free events delivered through one typed Tauri IPC channel per image generation.
#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(crate) enum ImageGenerationEvent {
    /// Native storage accepted the request before provider work began.
    Started {
        /// Bottie-owned cancellation identity.
        run_id: String,
        /// Durable pending assistant message.
        message: StoredMessage,
    },
    /// The run entered one bounded stage without exposing provider progress payloads.
    Progress {
        /// Bottie-owned cancellation identity.
        run_id: String,
        /// Current native orchestration stage.
        stage: ImageGenerationStage,
        /// Exact number of outputs requested for this run.
        total: u8,
    },
    /// Validated bytes and exact provenance reached durable storage.
    Completed {
        /// Bottie-owned cancellation identity.
        run_id: String,
        /// Final durable assistant image message.
        message: StoredMessage,
    },
    /// Cancellation reached durable storage and all temporary bytes were discarded.
    Cancelled {
        /// Bottie-owned cancellation identity.
        run_id: String,
        /// Final cancelled assistant image message.
        message: StoredMessage,
    },
    /// A redacted provider, validation, or storage failure reached durable storage.
    Failed {
        /// Bottie-owned cancellation identity.
        run_id: String,
        /// Final failed assistant image message when storage cleanup succeeded.
        message: Option<StoredMessage>,
        /// Stable user-readable failure without provider response content.
        error: ProviderError,
    },
}

impl ImageGenerationRuns {
    /// Reserves the one image-generation slot for an opaque Bottie run identity.
    async fn reserve(&self, run_id: String, abort_handle: AbortHandle) -> bool {
        let mut active = self.active.lock().await;
        if active.is_some() {
            return false;
        }
        *active = Some(ActiveImageRun {
            run_id,
            abort_handle,
        });
        true
    }

    /// Clears the slot only when it still belongs to the completing run.
    async fn finish(&self, run_id: &str) {
        let mut active = self.active.lock().await;
        if active.as_ref().is_some_and(|run| run.run_id == run_id) {
            *active = None;
        }
    }

    /// Cancels the exact active run without accepting provider-owned identifiers.
    async fn cancel(&self, run_id: &str) -> bool {
        let active = self.active.lock().await;
        let Some(run) = active.as_ref() else {
            return false;
        };
        if run.run_id != run_id {
            return false;
        }
        run.abort_handle.abort();
        true
    }

    /// Cancels the active image run before another mutually exclusive interaction begins.
    pub(crate) async fn cancel_active(&self) -> bool {
        let active = self.active.lock().await;
        let Some(run) = active.as_ref() else {
            return false;
        };
        run.abort_handle.abort();
        true
    }
}

#[tauri::command]
/// Starts one explicit hosted image generation from saved native configuration.
pub(crate) async fn start_image_generation(
    state: State<'_, AppState>,
    request: StartImageGenerationRequest,
    on_event: Channel<ImageGenerationEvent>,
) -> Result<ImageGenerationRun, ProviderError> {
    if request.execution != GeneratedAssetExecution::Cloud {
        return Err(ProviderError::invalid_request(
            "The selected local image-generation route is not installed.",
        ));
    }
    if state.microphone.is_capturing() {
        return Err(ProviderError::invalid_request(
            "Stop or discard local voice capture before generating an image.",
        ));
    }
    let generation_request =
        ImageGenerationRequest::new(request.prompt, request.width, request.height, request.count)?;
    let settings = state.providers.read().await.settings();
    let api_key = state
        .credentials
        .get(QWEN_IMAGE_PROVIDER_ID)?
        .ok_or_else(|| {
            ProviderError::invalid_request(
                "Save a Model Studio API key before generating a cloud image.",
            )
        })?;
    let provider = DashScopeQwenImageProvider::new(&settings.qwen_image_base_url, api_key)?;
    let downloader = GeneratedImageDownloader::new()?;
    let provenance = GeneratedImageProvenance::new(
        QWEN_IMAGE_PROVIDER_ID,
        DASHSCOPE_QWEN_IMAGE_MODEL_ID,
        GeneratedAssetExecution::Cloud,
        None,
    )
    .map_err(storage_error)?;
    let run_id = uuid::Uuid::new_v4().to_string();
    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    if !state.image_runs.reserve(run_id.clone(), abort_handle).await {
        return Err(ProviderError::invalid_request(
            "Wait for the active image generation to finish.",
        ));
    }
    let pending_message = match state.conversations.start_generated_image_message(
        &request.conversation_id,
        request.count,
        &provenance,
    ) {
        Ok(message) => message,
        Err(error) => {
            state.image_runs.finish(&run_id).await;
            return Err(storage_error(error));
        }
    };
    let accepted = ImageGenerationRun {
        run_id: run_id.clone(),
        message: pending_message.clone(),
    };
    let runs = state.image_runs.clone();
    let conversations = state.conversations.clone();
    let diagnostics = state.diagnostics.clone();
    let task_run_id = run_id.clone();
    let message_id = pending_message.id.clone();
    let output_count = request.count;
    tauri::async_runtime::spawn(async move {
        let _ = on_event.send(ImageGenerationEvent::Started {
            run_id: task_run_id.clone(),
            message: pending_message,
        });
        let _ = on_event.send(ImageGenerationEvent::Progress {
            run_id: task_run_id.clone(),
            stage: ImageGenerationStage::Generating,
            total: output_count,
        });
        let generation = async {
            let references = provider.generate(generation_request).await?;
            let _ = on_event.send(ImageGenerationEvent::Progress {
                run_id: task_run_id.clone(),
                stage: ImageGenerationStage::Downloading,
                total: output_count,
            });
            let images = downloader.download_all(&references, &conversations).await?;
            conversations
                .complete_generated_image_message(&message_id, &images)
                .map_err(storage_error)
        };
        match Abortable::new(generation, abort_registration).await {
            Ok(Ok(message)) => {
                let _ = on_event.send(ImageGenerationEvent::Completed {
                    run_id: task_run_id.clone(),
                    message,
                });
                record_diagnostic(
                    &diagnostics,
                    "info",
                    "Image generation completed",
                    Some(QWEN_IMAGE_PROVIDER_ID),
                    Some("Validated PNG bytes reached app-private storage"),
                )
                .await;
            }
            Ok(Err(error)) => {
                let error = sanitized(error);
                let message = conversations
                    .fail_generated_image_message(
                        &message_id,
                        GeneratedAssetStatus::Failed,
                        Some(error.code.as_str()),
                    )
                    .ok();
                let _ = on_event.send(ImageGenerationEvent::Failed {
                    run_id: task_run_id.clone(),
                    message,
                    error: error.clone(),
                });
                record_diagnostic(
                    &diagnostics,
                    "error",
                    "Image generation failed",
                    Some(QWEN_IMAGE_PROVIDER_ID),
                    error.diagnostic.as_deref().or(Some(&error.message)),
                )
                .await;
            }
            Err(_) => {
                match conversations.fail_generated_image_message(
                    &message_id,
                    GeneratedAssetStatus::Cancelled,
                    None,
                ) {
                    Ok(message) => {
                        let _ = on_event.send(ImageGenerationEvent::Cancelled {
                            run_id: task_run_id.clone(),
                            message,
                        });
                    }
                    Err(error) => {
                        let _ = on_event.send(ImageGenerationEvent::Failed {
                            run_id: task_run_id.clone(),
                            message: None,
                            error: storage_error(error),
                        });
                    }
                }
                record_diagnostic(
                    &diagnostics,
                    "info",
                    "Image generation cancelled",
                    Some(QWEN_IMAGE_PROVIDER_ID),
                    Some("Temporary provider references and partial bytes were discarded"),
                )
                .await;
            }
        }
        runs.finish(&task_run_id).await;
    });
    Ok(accepted)
}

#[tauri::command]
/// Cancels the exact active image generation by its opaque Bottie run identity.
pub(crate) async fn cancel_image_generation(
    run_id: String,
    state: State<'_, AppState>,
) -> Result<bool, ProviderError> {
    Ok(state.image_runs.cancel(&run_id).await)
}

/// Maps path-free storage failures into the existing provider-error contract.
fn storage_error(error: StorageError) -> ProviderError {
    match error.code {
        "invalid_request" | "not_found" => ProviderError::invalid_request(error.message),
        _ => ProviderError::internal(error.message, None),
    }
}

#[cfg(test)]
mod tests {
    use futures_util::future::AbortHandle;

    use super::ImageGenerationRuns;

    #[test]
    fn admits_only_one_active_run_and_cancels_only_the_exact_identity() {
        tauri::async_runtime::block_on(async {
            let runs = ImageGenerationRuns::default();
            let (first, _) = AbortHandle::new_pair();
            let (second, _) = AbortHandle::new_pair();

            assert!(runs.reserve("first".into(), first).await);
            assert!(!runs.reserve("second".into(), second).await);
            assert!(!runs.cancel("wrong").await);
            assert!(runs.cancel("first").await);
            assert!(runs.cancel("first").await);
            let (blocked_after_cancel, _) = AbortHandle::new_pair();
            assert!(!runs.reserve("blocked".into(), blocked_after_cancel).await);
            runs.finish("first").await;

            let (third, _) = AbortHandle::new_pair();
            assert!(runs.reserve("third".into(), third).await);
            assert!(runs.cancel_active().await);
            assert!(runs.cancel_active().await);
            runs.finish("third").await;
            assert!(!runs.cancel_active().await);
        });
    }
}
