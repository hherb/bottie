//! Hosted image-editing command and durable run acceptance.

use serde::Deserialize;
use tauri::{State, ipc::Channel};

use super::{
    HostedImageRequest, ImageGenerationEvent, ImageGenerationRun, spawn_image_run, storage_error,
};
use crate::{
    AppState,
    image_generation::{
        DASHSCOPE_QWEN_IMAGE_MODEL_ID, DashScopeQwenImageProvider, GeneratedImageDownloader,
        ImageEditingRequest, ImageGenerationRequest, QWEN_IMAGE_PROVIDER_ID,
    },
    inference::ProviderError,
    storage::{
        GeneratedAssetExecution, GeneratedAssetStatus, GeneratedImageProvenance,
        GeneratedImageRequestOptions, GeneratedImageSourceReference, GeneratedImageSourceType,
        validate_generated_image_source_references,
    },
};

/// Explicit path-free hosted image-editing request accepted from a future editing UI.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StartImageEditingRequest {
    /// Existing durable conversation that owns the assistant image message.
    conversation_id: String,
    /// Exact current durable user message containing the editing instruction.
    request_message_id: String,
    /// User-authored edit instruction rechecked against durable storage.
    prompt: String,
    /// Explicit requested output width.
    width: u32,
    /// Explicit requested output height.
    height: u32,
    /// Exact requested output count.
    count: u8,
    /// One to three ordered opaque sources resolved and validated only in Rust.
    sources: Vec<ImageEditingSourceRequest>,
}

/// One path-free source identity selected for hosted editing.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImageEditingSourceRequest {
    /// Stable source category, never a filesystem path.
    source_type: GeneratedImageSourceType,
    /// Opaque attachment or generated-asset identity.
    source_id: String,
}

impl StartImageEditingRequest {
    /// Converts ordered path-free input into native source references without resolving bytes.
    pub(crate) fn source_references(&self) -> Vec<GeneratedImageSourceReference> {
        self.sources
            .iter()
            .map(|source| match source.source_type {
                GeneratedImageSourceType::Attachment => {
                    GeneratedImageSourceReference::Attachment(source.source_id.clone())
                }
                GeneratedImageSourceType::GeneratedAsset => {
                    GeneratedImageSourceReference::GeneratedAsset(source.source_id.clone())
                }
            })
            .collect()
    }
}

#[tauri::command]
/// Starts one explicit hosted edit after native selected-lineage and byte revalidation.
pub(crate) async fn start_image_editing(
    state: State<'_, AppState>,
    request: StartImageEditingRequest,
    on_event: Channel<ImageGenerationEvent>,
) -> Result<ImageGenerationRun, ProviderError> {
    if state.microphone.is_capturing() {
        return Err(ProviderError::invalid_request(
            "Stop or discard local voice capture before editing an image.",
        ));
    }
    let source_references = request.source_references();
    validate_generated_image_source_references(&source_references).map_err(storage_error)?;
    let generation =
        ImageGenerationRequest::new(request.prompt, request.width, request.height, request.count)?;
    let provenance = GeneratedImageProvenance::new(
        QWEN_IMAGE_PROVIDER_ID,
        DASHSCOPE_QWEN_IMAGE_MODEL_ID,
        GeneratedAssetExecution::Cloud,
        None,
    )
    .map_err(storage_error)?;
    let options = GeneratedImageRequestOptions::new(
        request.width,
        request.height,
        generation.prompt_extend(),
    )
    .map_err(storage_error)?;
    let settings = state.providers.read().await.settings();
    let api_key = state
        .credentials
        .get(QWEN_IMAGE_PROVIDER_ID)?
        .ok_or_else(|| {
            ProviderError::invalid_request(
                "Save a Model Studio API key before editing a cloud image.",
            )
        })?;
    let provider = DashScopeQwenImageProvider::new(&settings.qwen_image_base_url, api_key)?;
    let downloader = GeneratedImageDownloader::new()?;
    let run_id = uuid::Uuid::new_v4().to_string();
    let Some(abort_registration) = state.image_runs.reserve_abortable(run_id.clone()).await else {
        return Err(ProviderError::invalid_request(
            "Wait for the active image generation to finish.",
        ));
    };
    let started = match state.conversations.start_generated_image_edit_message(
        &request.conversation_id,
        &request.request_message_id,
        generation.prompt(),
        request.count,
        &provenance,
        &options,
        &source_references,
    ) {
        Ok(started) => started,
        Err(error) => {
            state.image_runs.finish(&run_id).await;
            return Err(storage_error(error));
        }
    };
    let pending_message = started.message;
    let editing = match ImageEditingRequest::new(
        started.prompt,
        started.options.width,
        started.options.height,
        started.output_count,
        started.sources,
    ) {
        Ok(editing) => editing,
        Err(error) => {
            let _ = state.conversations.fail_generated_image_message(
                &pending_message.id,
                GeneratedAssetStatus::Failed,
                Some("invalid_edit_request"),
            );
            state.image_runs.finish(&run_id).await;
            return Err(error);
        }
    };
    Ok(spawn_image_run(
        state.image_runs.clone(),
        state.conversations.clone(),
        state.diagnostics.clone(),
        run_id,
        abort_registration,
        pending_message,
        request.count,
        HostedImageRequest::Editing(editing),
        provider,
        downloader,
        on_event,
    ))
}
