//! Local server and worker mapping into shared PNG normalization and cancellation contracts.

use std::{fs, path::PathBuf, sync::Arc};

use tokio::sync::watch;

use crate::{
    inference::ProviderError,
    local_image_worker::execution::{
        LocalGeneratedOutput, LocalGenerationRequest, LocalImageExecutionError,
        LocalImageWorkerRuntime,
    },
    local_image_worker::{backend::VerifiedLocalImageBackend, server::LocalImageServerError},
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
        installation: VerifiedLocalImageBackend,
        request: ImageGenerationRequest,
        seed: u64,
        store: ConversationStore,
        mut cancellation: watch::Receiver<bool>,
        on_progress: impl FnMut() + Send,
    ) -> Result<Vec<PreparedGeneratedImage>, LocalImageGenerationError> {
        let installation = match installation {
            VerifiedLocalImageBackend::Server(server) => {
                let dimensions = request.dimensions();
                let png = server
                    .generate(
                        request.prompt(),
                        dimensions,
                        seed,
                        cancellation.clone(),
                        on_progress,
                    )
                    .await
                    .map_err(map_server_error)?;
                let prepared = tauri::async_runtime::spawn_blocking(move || {
                    normalize_server_output(png, &store, dimensions)
                })
                .await
                .map_err(|_| map_server_error(LocalImageServerError::Unavailable))?
                .map_err(|_| map_server_error(LocalImageServerError::InvalidResponse))?;
                if *cancellation.borrow_and_update() {
                    return Err(LocalImageGenerationError::Cancelled);
                }
                return Ok(vec![prepared]);
            }
            VerifiedLocalImageBackend::Moss(moss) => {
                let dimensions = request.dimensions();
                let png=moss.generate(request.prompt(),dimensions,seed,cancellation.clone(),on_progress).await
                    .map_err(|error|match error {
                        crate::local_image_worker::moss::MossError::Cancelled => LocalImageGenerationError::Cancelled,
                        _ => LocalImageGenerationError::Failed(ProviderError::unavailable(
                            "MOSS-Image could not complete this local image. Check its runtime and prompt limits.", None)),
                    })?;
                let prepared = tauri::async_runtime::spawn_blocking(move || {
                    normalize_server_output(png, &store, dimensions)
                })
                .await
                .map_err(|_| map_server_error(LocalImageServerError::Unavailable))?
                .map_err(|_| map_server_error(LocalImageServerError::InvalidResponse))?;
                if *cancellation.borrow_and_update() {
                    return Err(LocalImageGenerationError::Cancelled);
                }
                return Ok(vec![prepared]);
            }
            VerifiedLocalImageBackend::Worker(worker) => worker,
        };
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

/// Maps local transport failures without exposing response bytes or endpoint internals.
fn map_server_error(error: LocalImageServerError) -> LocalImageGenerationError {
    match error {
        LocalImageServerError::Cancelled => LocalImageGenerationError::Cancelled,
        LocalImageServerError::Unavailable => {
            LocalImageGenerationError::Failed(ProviderError::unavailable(
                "The local Qwen-Image 2.1 image server is unavailable.",
                None,
            ))
        }
        LocalImageServerError::InvalidResponse => {
            LocalImageGenerationError::Failed(ProviderError::malformed(
                "The local image server returned an invalid PNG image.",
                None,
            ))
        }
    }
}

/// Gives inline server PNGs the same metadata-free decode and storage contract as worker output.
pub(super) fn normalize_server_output(
    png: Vec<u8>,
    store: &ConversationStore,
    dimensions: (u32, u32),
) -> Result<PreparedGeneratedImage, crate::storage::StorageError> {
    use std::io::Write;
    let temporary = store.generated_asset_temporary_directory();
    fs::create_dir_all(&temporary).map_err(|_| crate::storage::StorageError::generated_image())?;
    let input = temporary.join(format!("{}.server.png.part", uuid::Uuid::new_v4()));
    let output = temporary.join(format!("{}.normalized.png.part", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&input)
            .map_err(|_| crate::storage::StorageError::generated_image())?;
        file.write_all(&png)
            .map_err(|_| crate::storage::StorageError::generated_image())?;
        drop(file);
        normalize_generated_png(&input, output, dimensions)
    })();
    let _ = fs::remove_file(input);
    result
}
