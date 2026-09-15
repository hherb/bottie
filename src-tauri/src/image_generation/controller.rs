//! One-active-run image generation, cancellation, progress, and durable terminal orchestration.

use futures_util::future::Abortable;
use serde::{Deserialize, Serialize};
use tauri::{State, ipc::Channel};

#[path = "controller/editing.rs"]
pub(super) mod editing;
#[path = "controller/local_run.rs"]
mod local_run;
#[path = "controller/retry.rs"]
mod retry;

pub(crate) use editing::start_image_editing;
use local_run::spawn_local_image_run;
pub(crate) use retry::retry_image_generation;

use crate::{
    AppState,
    diagnostics::{record_diagnostic, sanitized},
    inference::ProviderError,
    storage::{
        GeneratedAssetExecution, GeneratedAssetStatus, GeneratedImageProvenance,
        GeneratedImageRequestOptions, StartedGeneratedImage, StorageError, StoredMessage,
    },
};

use super::{
    DASHSCOPE_QWEN_IMAGE_MODEL_ID, DashScopeQwenImageProvider, GeneratedImageDownloader,
    ImageEditingProvider, ImageEditingRequest, ImageGenerationProvider, ImageGenerationRequest,
    ImageGenerationRuns, QWEN_IMAGE_PROVIDER_ID,
};

/// Exact hosted operation retained behind one shared durable run lifecycle.
pub(super) enum HostedImageRequest {
    /// A text-only image generation request.
    Generation(ImageGenerationRequest),
    /// An edit over ordered validated native source bytes.
    Editing(ImageEditingRequest),
}

impl HostedImageRequest {
    /// Executes the exact selected hosted operation without fallback.
    async fn execute(
        self,
        provider: &DashScopeQwenImageProvider,
    ) -> Result<Vec<super::GeneratedImageReference>, ProviderError> {
        match self {
            Self::Generation(request) => provider.generate(request).await,
            Self::Editing(request) => provider.edit(request).await,
        }
    }
}

/// Explicit path-free image generation request accepted from the composer.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StartImageGenerationRequest {
    /// Existing durable conversation that owns the assistant image message.
    pub(crate) conversation_id: String,
    /// Exact durable user message that owns the prompt sent to the provider.
    pub(crate) request_message_id: String,
    /// User-authored image description kept behind the native provider boundary.
    pub(crate) prompt: String,
    /// Explicit requested output width.
    pub(crate) width: u32,
    /// Explicit requested output height.
    pub(crate) height: u32,
    /// Exact requested output count.
    pub(crate) count: u8,
    /// Explicit execution backend selected before any provider or worker operation.
    pub(crate) execution: GeneratedAssetExecution,
}

/// Opaque terminal image response selected for exact native retry.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RetryImageGenerationRequest {
    /// Failed or cancelled assistant image message on the selected branch.
    pub(crate) message_id: String,
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
    /// Waiting for the selected Cloud or local model to produce output.
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

#[tauri::command]
/// Starts one explicit cloud or freshly verified local image generation.
pub(crate) async fn start_image_generation(
    state: State<'_, AppState>,
    request: StartImageGenerationRequest,
    on_event: Channel<ImageGenerationEvent>,
) -> Result<ImageGenerationRun, ProviderError> {
    if state.microphone.is_capturing() {
        return Err(ProviderError::invalid_request(
            "Stop or discard local voice capture before generating an image.",
        ));
    }
    match request.execution {
        GeneratedAssetExecution::Cloud => {
            start_cloud_image_generation(&state, request, on_event).await
        }
        GeneratedAssetExecution::Local => {
            start_local_image_generation(&state, request, on_event).await
        }
    }
}

async fn start_cloud_image_generation(
    state: &AppState,
    request: StartImageGenerationRequest,
    on_event: Channel<ImageGenerationEvent>,
) -> Result<ImageGenerationRun, ProviderError> {
    let generation_request =
        ImageGenerationRequest::new(request.prompt, request.width, request.height, request.count)?;
    let options = GeneratedImageRequestOptions::new(
        request.width,
        request.height,
        generation_request.prompt_extend(),
    )
    .map_err(storage_error)?;
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
    let Some(abort_registration) = state.image_runs.reserve_abortable(run_id.clone()).await else {
        return Err(ProviderError::invalid_request(
            "Wait for the active image generation to finish.",
        ));
    };
    let started: StartedGeneratedImage = match state.conversations.start_generated_image_message(
        &request.conversation_id,
        &request.request_message_id,
        generation_request.prompt(),
        request.count,
        &provenance,
        &options,
    ) {
        Ok(started) => started,
        Err(error) => {
            state.image_runs.finish(&run_id).await;
            return Err(storage_error(error));
        }
    };
    Ok(spawn_image_run(
        state.image_runs.clone(),
        state.conversations.clone(),
        state.diagnostics.clone(),
        run_id,
        abort_registration,
        started.message,
        request.count,
        HostedImageRequest::Generation(generation_request),
        provider,
        downloader,
        on_event,
    ))
}

async fn start_local_image_generation(
    state: &AppState,
    request: StartImageGenerationRequest,
    on_event: Channel<ImageGenerationEvent>,
) -> Result<ImageGenerationRun, ProviderError> {
    let generation_request = ImageGenerationRequest::new_local(
        request.prompt,
        request.width,
        request.height,
        request.count,
    )?;
    let installation = state
        .local_image_availability
        .inspect_for_execution()
        .await
        .map_err(|_| {
            ProviderError::unavailable("The selected local image runtime is not ready.", None)
        })?;
    let seed = local_generation_seed();
    let provenance = GeneratedImageProvenance::new(
        QWEN_IMAGE_PROVIDER_ID,
        installation.model_id(),
        GeneratedAssetExecution::Local,
        Some(seed),
    )
    .map_err(storage_error)?;
    let options = GeneratedImageRequestOptions::new(
        request.width,
        request.height,
        generation_request.prompt_extend(),
    )
    .map_err(storage_error)?;
    let run_id = uuid::Uuid::new_v4().to_string();
    let Some(cancellation) = state.image_runs.reserve_cooperative(run_id.clone()).await else {
        return Err(ProviderError::invalid_request(
            "Wait for the active image generation to finish.",
        ));
    };
    let started = match state.conversations.start_generated_image_message(
        &request.conversation_id,
        &request.request_message_id,
        generation_request.prompt(),
        request.count,
        &provenance,
        &options,
    ) {
        Ok(started) => started,
        Err(error) => {
            state.image_runs.finish(&run_id).await;
            return Err(storage_error(error));
        }
    };
    Ok(spawn_local_image_run(
        state.image_runs.clone(),
        state.local_image_generation.clone(),
        state.conversations.clone(),
        state.diagnostics.clone(),
        run_id,
        cancellation,
        started.message,
        request.count,
        generation_request,
        installation,
        u64::try_from(seed).expect("local seed is non-negative"),
        on_event,
    ))
}

fn local_generation_seed() -> i64 {
    let bytes: [u8; 8] = uuid::Uuid::new_v4().as_bytes()[..8]
        .try_into()
        .expect("UUID prefix has eight bytes");
    i64::from_be_bytes(bytes) & i64::MAX
}

/// Spawns one already-reserved provider run and returns its path-free accepted state.
#[allow(clippy::too_many_arguments)]
fn spawn_image_run(
    runs: ImageGenerationRuns,
    conversations: crate::storage::ConversationStore,
    diagnostics: crate::diagnostics::Diagnostics,
    run_id: String,
    abort_registration: futures_util::future::AbortRegistration,
    pending_message: StoredMessage,
    output_count: u8,
    hosted_request: HostedImageRequest,
    provider: DashScopeQwenImageProvider,
    downloader: GeneratedImageDownloader,
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
        let _ = on_event.send(ImageGenerationEvent::Progress {
            run_id: task_run_id.clone(),
            stage: ImageGenerationStage::Generating,
            total: output_count,
        });
        let generation = async {
            let references = hosted_request.execute(&provider).await?;
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
    accepted
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
