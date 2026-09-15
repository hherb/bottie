//! Explicit app-owned orchestration for the selected local image-model acquisition.

use std::sync::{Arc, Mutex, MutexGuard};

use tauri::{AppHandle, Emitter, Runtime};

use super::{
    availability::LocalImageAvailability,
    availability_service::{LocalImageAvailabilityService, VerifiedLocalImageAcquisition},
    model_cache::{CacheResumeProgress, inspect_bound_resume},
    model_download::{ModelDownloadCancellation, ModelDownloader},
};

#[path = "acquisition_coordinator/session.rs"]
mod session;

use session::command_error;
pub(crate) use session::{
    LocalImageAcquisitionApproval, LocalImageAcquisitionError, LocalImageAcquisitionErrorCode,
    LocalImageAcquisitionSession, LocalImageAcquisitionStatus,
};
#[cfg(test)]
pub(crate) use session::{LocalImageAcquisitionFailure, LocalImageAcquisitionPhase};

/// Fixed application event carrying only path-free acquisition lifecycle state.
pub(crate) const LOCAL_IMAGE_ACQUISITION_EVENT: &str = "local-image-acquisition-changed";

/// Event sink kept abstract so state-machine tests never require an application WebView.
pub(crate) trait LocalImageAcquisitionPublisher: Send + Sync {
    /// Publishes one complete path-free status snapshot.
    fn publish(&self, status: LocalImageAcquisitionStatus);
}

impl<R: Runtime> LocalImageAcquisitionPublisher for AppHandle<R> {
    fn publish(&self, status: LocalImageAcquisitionStatus) {
        let _ = self.emit(LOCAL_IMAGE_ACQUISITION_EVENT, status);
    }
}

/// Rust-owned coordinator for one explicit, cancellable, resumable selected-package download.
pub(crate) struct LocalImageAcquisitionCoordinator {
    availability: Arc<LocalImageAvailabilityService>,
    downloader: ModelDownloader,
    session: Arc<Mutex<LocalImageAcquisitionSession>>,
    publisher: Arc<dyn LocalImageAcquisitionPublisher>,
}

impl LocalImageAcquisitionCoordinator {
    /// Creates the production coordinator without touching the cache or network.
    pub(crate) fn new(
        availability: Arc<LocalImageAvailabilityService>,
        publisher: impl LocalImageAcquisitionPublisher + 'static,
    ) -> Result<Self, LocalImageAcquisitionError> {
        let selected = super::model_package::selected_qwen_image_2512_q4_package()
            .map_err(|_| command_error(LocalImageAcquisitionErrorCode::InvalidState))?;
        let downloader = ModelDownloader::new()
            .map_err(|_| command_error(LocalImageAcquisitionErrorCode::InvalidState))?;
        Ok(Self {
            availability,
            downloader,
            session: Arc::new(Mutex::new(LocalImageAcquisitionSession::new(selected))),
            publisher: Arc::new(publisher),
        })
    }

    /// Returns fresh path-free readiness and exact resumable progress without mutating cache state.
    pub(crate) async fn status(
        &self,
    ) -> Result<LocalImageAcquisitionStatus, LocalImageAcquisitionError> {
        if lock(&self.session).is_active() {
            return Ok(lock(&self.session).status());
        }
        let metadata = self
            .availability
            .inspect()
            .await
            .map_err(|_| command_error(LocalImageAcquisitionErrorCode::Unavailable))?;
        let resume = if matches!(
            metadata.availability,
            LocalImageAvailability::ModelMissing | LocalImageAvailability::ModelMismatch
        ) {
            self.inspect_resume().await?
        } else {
            None
        };
        Ok(lock(&self.session).refresh_idle(metadata, resume))
    }

    /// Starts only after exact explicit approval and fresh native worker verification.
    pub(crate) async fn start(
        self: &Arc<Self>,
        approval: LocalImageAcquisitionApproval,
    ) -> Result<LocalImageAcquisitionStatus, LocalImageAcquisitionError> {
        let prerequisites = self
            .availability
            .inspect_for_acquisition()
            .await
            .map_err(|_| command_error(LocalImageAcquisitionErrorCode::Unavailable))?;
        let resume = inspect_resume(&prerequisites).await?;
        let cancellation = lock(&self.session)
            .begin(&approval, prerequisites.metadata(), resume)
            .map_err(command_error)?;
        let status = lock(&self.session).status();
        self.publisher.publish(status.clone());
        let coordinator = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            coordinator.run(prerequisites, cancellation).await;
        });
        Ok(status)
    }

    /// Requests cancellation of the one active acquisition while retaining synced partials.
    pub(crate) fn cancel(&self) -> Result<LocalImageAcquisitionStatus, LocalImageAcquisitionError> {
        let status = lock(&self.session)
            .request_cancel()
            .map_err(command_error)?;
        self.publisher.publish(status.clone());
        Ok(status)
    }

    /// Returns whether model acquisition currently owns its native mutation slot.
    pub(crate) fn is_active(&self) -> bool {
        lock(&self.session).is_active()
    }

    async fn inspect_resume(
        &self,
    ) -> Result<Option<CacheResumeProgress>, LocalImageAcquisitionError> {
        let selected = self.availability.selected_package();
        inspect_resume_parts(
            self.availability.acquisition_cache_root(),
            selected.manifest().clone(),
            selected.source_plan().resume_binding(),
        )
        .await
    }

    async fn run(
        self: Arc<Self>,
        prerequisites: VerifiedLocalImageAcquisition,
        cancellation: ModelDownloadCancellation,
    ) {
        let mut acquisition =
            match super::model_acquisition::ModelAcquisition::new(prerequisites.manifest().clone())
            {
                Ok(acquisition) => acquisition,
                Err(_) => {
                    self.finish_internal_failure();
                    return;
                }
            };
        if acquisition.begin_download().is_err() {
            self.finish_internal_failure();
            return;
        }
        let session = Arc::clone(&self.session);
        let publisher = Arc::clone(&self.publisher);
        let result = self
            .downloader
            .download_to_cache(
                prerequisites.source_plan(),
                &mut acquisition,
                prerequisites.cache_root(),
                &cancellation,
                move |progress| {
                    if let Ok(status) = lock(&session).record_progress(progress) {
                        publisher.publish(status);
                    }
                },
            )
            .await;
        let status = match result {
            Ok(_) => match self.availability.inspect().await {
                Ok(metadata) => lock(&self.session).finish_success(&metadata),
                Err(_) => {
                    self.finish_internal_failure();
                    return;
                }
            },
            Err(error) => lock(&self.session).finish_failure(error),
        };
        if let Ok(status) = status {
            self.publisher.publish(status);
        } else {
            self.finish_internal_failure();
        }
    }

    fn finish_internal_failure(&self) {
        let status = lock(&self.session).finish_internal_failure();
        self.publisher.publish(status);
    }
}

async fn inspect_resume(
    prerequisites: &VerifiedLocalImageAcquisition,
) -> Result<Option<CacheResumeProgress>, LocalImageAcquisitionError> {
    inspect_resume_parts(
        prerequisites.cache_root().to_path_buf(),
        prerequisites.manifest().clone(),
        prerequisites.source_plan().resume_binding(),
    )
    .await
}

async fn inspect_resume_parts(
    cache_root: std::path::PathBuf,
    manifest: super::model_acquisition::ModelPackageManifest,
    binding: String,
) -> Result<Option<CacheResumeProgress>, LocalImageAcquisitionError> {
    tauri::async_runtime::spawn_blocking(move || {
        inspect_bound_resume(&cache_root, &manifest, &binding)
    })
    .await
    .map_err(|_| command_error(LocalImageAcquisitionErrorCode::InvalidState))?
    .map_err(|_| command_error(LocalImageAcquisitionErrorCode::InvalidState))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
