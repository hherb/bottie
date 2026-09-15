//! Exact durable cloud and local image retry routing without backend fallback.

use tauri::{State, ipc::Channel};

use super::{
    HostedImageRequest, ImageGenerationEvent, ImageGenerationRun, RetryImageGenerationRequest,
    spawn_image_run, spawn_local_image_run, storage_error,
};
use crate::{
    AppState,
    image_generation::{
        DASHSCOPE_QWEN_IMAGE_MODEL_ID, DashScopeQwenImageProvider, GeneratedImageDownloader,
        ImageEditingRequest, ImageGenerationRequest, QWEN_IMAGE_PROVIDER_ID,
    },
    inference::ProviderError,
    storage::{
        GeneratedAssetExecution, GeneratedAssetStatus, GeneratedImageRetry, StartedGeneratedImage,
    },
};

#[tauri::command]
/// Retries one selected terminal image response through its exact durable execution backend.
pub(crate) async fn retry_image_generation(
    state: State<'_, AppState>,
    request: RetryImageGenerationRequest,
    on_event: Channel<ImageGenerationEvent>,
) -> Result<ImageGenerationRun, ProviderError> {
    if state.microphone.is_capturing() {
        return Err(ProviderError::invalid_request(
            "Stop or discard local voice capture before generating an image.",
        ));
    }
    let retry = state
        .conversations
        .inspect_generated_image_retry(&request.message_id)
        .map_err(storage_error)?;
    match retry.provenance.execution {
        GeneratedAssetExecution::Cloud => {
            retry_cloud(&state, &request.message_id, retry, on_event).await
        }
        GeneratedAssetExecution::Local => {
            retry_local(&state, &request.message_id, retry, on_event).await
        }
    }
}

async fn retry_cloud(
    state: &AppState,
    message_id: &str,
    retry: GeneratedImageRetry,
    on_event: Channel<ImageGenerationEvent>,
) -> Result<ImageGenerationRun, ProviderError> {
    let generation_request = validate_cloud_retry(&retry)?;
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
    let run_id = uuid::Uuid::new_v4().to_string();
    let Some(abort_registration) = state.image_runs.reserve_abortable(run_id.clone()).await else {
        return Err(active_run_error());
    };
    let started = start_retry(state, message_id, &retry, &run_id).await?;
    Ok(spawn_image_run(
        state.image_runs.clone(),
        state.conversations.clone(),
        state.diagnostics.clone(),
        run_id,
        abort_registration,
        started.message,
        retry.output_count,
        generation_request,
        provider,
        downloader,
        on_event,
    ))
}

async fn retry_local(
    state: &AppState,
    message_id: &str,
    retry: GeneratedImageRetry,
    on_event: Channel<ImageGenerationEvent>,
) -> Result<ImageGenerationRun, ProviderError> {
    let generation_request = validate_local_retry(&retry)?;
    let seed = retry
        .provenance
        .seed
        .and_then(|seed| u64::try_from(seed).ok())
        .ok_or_else(invalid_retry_error)?;
    let installation = state
        .local_image_availability
        .inspect_for_execution()
        .await
        .map_err(|_| {
            ProviderError::unavailable("The selected local image runtime is not ready.", None)
        })?;
    if installation.model_id() != retry.provenance.model_id {
        return Err(invalid_retry_error());
    }
    let run_id = uuid::Uuid::new_v4().to_string();
    let Some(cancellation) = state.image_runs.reserve_cooperative(run_id.clone()).await else {
        return Err(active_run_error());
    };
    let started = start_retry(state, message_id, &retry, &run_id).await?;
    Ok(spawn_local_image_run(
        state.image_runs.clone(),
        state.local_image_generation.clone(),
        state.conversations.clone(),
        state.diagnostics.clone(),
        run_id,
        cancellation,
        started.message,
        retry.output_count,
        generation_request,
        installation,
        seed,
        on_event,
    ))
}

async fn start_retry(
    state: &AppState,
    message_id: &str,
    inspected: &GeneratedImageRetry,
    run_id: &str,
) -> Result<StartedGeneratedImage, ProviderError> {
    let started = match state
        .conversations
        .retry_generated_image_message(message_id)
    {
        Ok(started) => started,
        Err(error) => {
            state.image_runs.finish(run_id).await;
            return Err(storage_error(error));
        }
    };
    if retry_matches(inspected, &started) {
        return Ok(started);
    }
    let _ = state.conversations.fail_generated_image_message(
        &started.message.id,
        GeneratedAssetStatus::Failed,
        Some("invalid_retry_request"),
    );
    state.image_runs.finish(run_id).await;
    Err(invalid_retry_error())
}

fn validate_cloud_retry(retry: &GeneratedImageRetry) -> Result<HostedImageRequest, ProviderError> {
    if retry.provenance.provider_id != QWEN_IMAGE_PROVIDER_ID
        || retry.provenance.model_id != DASHSCOPE_QWEN_IMAGE_MODEL_ID
        || retry.provenance.seed.is_some()
        || !retry.options.prompt_extend
    {
        return Err(invalid_retry_error());
    }
    if retry.sources.is_empty() {
        return ImageGenerationRequest::new(
            retry.prompt.clone(),
            retry.options.width,
            retry.options.height,
            retry.output_count,
        )
        .map(HostedImageRequest::Generation);
    }
    ImageEditingRequest::new(
        retry.prompt.clone(),
        retry.options.width,
        retry.options.height,
        retry.output_count,
        retry.sources.clone(),
    )
    .map(HostedImageRequest::Editing)
}

fn validate_local_retry(
    retry: &GeneratedImageRetry,
) -> Result<ImageGenerationRequest, ProviderError> {
    if retry.provenance.provider_id != QWEN_IMAGE_PROVIDER_ID
        || retry.options.prompt_extend
        || !retry.sources.is_empty()
    {
        return Err(invalid_retry_error());
    }
    ImageGenerationRequest::new_local(
        retry.prompt.clone(),
        retry.options.width,
        retry.options.height,
        retry.output_count,
    )
    .map_err(|_| invalid_retry_error())
}

fn retry_matches(inspected: &GeneratedImageRetry, started: &StartedGeneratedImage) -> bool {
    inspected.prompt == started.prompt
        && inspected.output_count == started.output_count
        && inspected.provenance == started.provenance
        && inspected.options == started.options
}

fn active_run_error() -> ProviderError {
    ProviderError::invalid_request("Wait for the active image generation to finish.")
}

fn invalid_retry_error() -> ProviderError {
    ProviderError::invalid_request("That image response cannot be retried.")
}

#[cfg(test)]
mod tests {
    use super::{HostedImageRequest, validate_cloud_retry, validate_local_retry};
    use crate::storage::{
        GeneratedAssetExecution, GeneratedImageProvenance, GeneratedImageRequestOptions,
        GeneratedImageRetry, GeneratedImageSourceFormat, GeneratedImageSourceReference,
        ValidatedGeneratedImageSource,
    };

    /// Creates one exact retry contract with optional native edit bytes.
    fn retry(sources: Vec<ValidatedGeneratedImageSource>) -> GeneratedImageRetry {
        GeneratedImageRetry::for_test(
            "Edit this",
            GeneratedImageProvenance::new(
                "qwen-image",
                "qwen-image-2.0-2026-03-03",
                GeneratedAssetExecution::Cloud,
                None,
            )
            .unwrap(),
            GeneratedImageRequestOptions::new(1_024, 1_024, true).unwrap(),
            sources,
        )
    }

    /// Creates one bounded native edit source without a filesystem path.
    fn source() -> ValidatedGeneratedImageSource {
        ValidatedGeneratedImageSource::for_test(
            GeneratedImageSourceReference::Attachment(uuid::Uuid::new_v4().to_string()),
            GeneratedImageSourceFormat::Png,
            8,
            8,
            vec![1, 2, 3],
        )
        .unwrap()
    }

    #[test]
    fn cloud_retry_routes_retained_sources_back_through_editing() {
        assert!(matches!(
            validate_cloud_retry(&retry(vec![source()])),
            Ok(HostedImageRequest::Editing(_))
        ));
        assert!(matches!(
            validate_cloud_retry(&retry(Vec::new())),
            Ok(HostedImageRequest::Generation(_))
        ));
    }

    #[test]
    fn local_retry_rejects_edit_sources_without_fallback() {
        let mut retry = retry(vec![source()]);
        retry.provenance.execution = GeneratedAssetExecution::Local;
        retry.provenance.model_id = "Qwen/Qwen-Image-2512".into();
        retry.provenance.seed = Some(42);
        retry.options = GeneratedImageRequestOptions::new(512, 512, false).unwrap();

        assert!(validate_local_retry(&retry).is_err());
    }
}
