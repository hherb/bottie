//! Local worker mapping into shared generated-PNG normalization and cancellation contracts.

use std::{fs, path::PathBuf, sync::Arc};

use tokio::sync::watch;

use crate::{
    inference::ProviderError,
    local_image_worker::execution::{
        LocalGeneratedOutput, LocalGenerationRequest, LocalImageExecutionError,
        LocalImageWorkerRuntime, VerifiedLocalImageInstallation,
    },
    storage::{ConversationStore, PreparedGeneratedImage, normalize_generated_png},
};

use super::ImageGenerationRequest;

/// Cancellation-aware local generation failure kept separate from ordinary provider errors.
pub(crate) enum LocalImageGenerationError {
    /// The private worker cooperatively cancelled or was forcibly reaped inside its grace.
    Cancelled,
    /// A path-free validation, runtime, or storage failure ended the request.
    Failed(ProviderError),
}

/// Cloneable native adapter around one process-wide warm worker runtime.
#[derive(Clone)]
pub(crate) struct LocalImageGenerator {
    runtime: Arc<LocalImageWorkerRuntime>,
}

impl LocalImageGenerator {
    /// Creates a lazy worker adapter beneath the app-private generated-image temporary root.
    pub(crate) fn new(output_parent: PathBuf) -> Result<Self, ProviderError> {
        let runtime = LocalImageWorkerRuntime::new(output_parent).map_err(map_execution_error)?;
        Ok(Self {
            runtime: Arc::new(runtime),
        })
    }

    /// Runs one freshly verified request and normalizes all worker PNGs before returning.
    pub(crate) async fn generate(
        &self,
        installation: VerifiedLocalImageInstallation,
        request: ImageGenerationRequest,
        seed: u64,
        store: ConversationStore,
        mut cancellation: watch::Receiver<bool>,
        on_progress: impl FnMut() + Send,
    ) -> Result<Vec<PreparedGeneratedImage>, LocalImageGenerationError> {
        let (width, height) = request.dimensions();
        let local_request =
            LocalGenerationRequest::new(request.prompt(), width, height, request.count(), seed)
                .map_err(|error| LocalImageGenerationError::Failed(map_execution_error(error)))?;
        let outputs = self
            .runtime
            .generate(
                installation,
                local_request,
                cancellation.clone(),
                on_progress,
            )
            .await
            .map_err(|error| match error {
                LocalImageExecutionError::Cancelled => LocalImageGenerationError::Cancelled,
                other => LocalImageGenerationError::Failed(map_execution_error(other)),
            })?;
        if *cancellation.borrow_and_update() {
            return Err(LocalImageGenerationError::Cancelled);
        }
        let prepared =
            tauri::async_runtime::spawn_blocking(move || normalize_local_outputs(outputs, &store))
                .await
                .map_err(|_| {
                    LocalImageGenerationError::Failed(ProviderError::internal(
                        "Bottie could not safely validate the local image.",
                        None,
                    ))
                })?
                .map_err(|error| {
                    LocalImageGenerationError::Failed(ProviderError::malformed(error.message, None))
                })?;
        if *cancellation.borrow_and_update() {
            return Err(LocalImageGenerationError::Cancelled);
        }
        Ok(prepared)
    }
}

/// Normalizes worker-owned source PNGs through the same bounded codec used by cloud results.
pub(super) fn normalize_local_outputs(
    outputs: Vec<LocalGeneratedOutput>,
    store: &ConversationStore,
) -> Result<Vec<PreparedGeneratedImage>, crate::storage::StorageError> {
    let temporary_directory = store.generated_asset_temporary_directory();
    fs::create_dir_all(&temporary_directory)
        .map_err(|_| crate::storage::StorageError::generated_image())?;
    outputs
        .into_iter()
        .map(|output| {
            let destination =
                temporary_directory.join(format!("{}.local.png.part", uuid::Uuid::new_v4()));
            normalize_generated_png(output.path(), destination, output.dimensions())
        })
        .collect()
}

fn map_execution_error(error: LocalImageExecutionError) -> ProviderError {
    match error {
        LocalImageExecutionError::InvalidOutput => ProviderError::malformed(
            "The local image worker returned an invalid PNG image.",
            None,
        ),
        LocalImageExecutionError::GenerationFailed => ProviderError::unavailable(
            "The local image worker could not generate this image.",
            None,
        ),
        LocalImageExecutionError::Cancelled => {
            ProviderError::unavailable("The local image generation was cancelled.", None)
        }
        _ => ProviderError::unavailable("The verified local image runtime is unavailable.", None),
    }
}
