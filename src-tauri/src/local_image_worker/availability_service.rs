//! App-owned path resolution and serialized native inspection for local-image readiness.

use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use serde::Serialize;

use super::{
    availability::{
        LocalImageAvailability, ModelCacheReadiness, WorkerInstallationReadiness,
        evaluate_local_image_availability, inspect_model_cache, inspect_worker_installation,
    },
    execution::VerifiedLocalImageInstallation,
    hardware::probe_local_image_hardware,
    model_package::{SelectedModelPackage, selected_qwen_image_2512_q4_package},
};

/// Fixed resource directory containing the complete selected local-image worker bundle.
pub(crate) const LOCAL_IMAGE_WORKER_DIRECTORY: &str = "local-image-worker";
/// Exact executable name inside the selected local-image worker bundle.
pub(crate) const LOCAL_IMAGE_WORKER_EXECUTABLE: &str = "bottie-local-image-mlx-worker";
/// Fixed application-data directory owning transactional local-image model caches.
pub(crate) const LOCAL_IMAGE_MODEL_CACHE_DIRECTORY: &str = "local-image-models";

/// Stable path-free failures returned by service construction or inspection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LocalImageServiceError {
    /// An application-owned base path was relative or contained lexical traversal.
    UnsafeLayout,
    /// The compiled selected-package contract was unexpectedly invalid.
    InvalidPackage,
    /// Native hardware facts or a blocking inspection task could not be completed.
    InspectionUnavailable,
    /// The exact hardware, worker, and model package did not all pass fresh verification.
    NotReady,
}

/// Closed path-free metadata returned by the read-only local-image availability command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalImageAvailabilityMetadata {
    /// Exact open-weight model identity, kept distinct from hosted Qwen Image 2.0.
    pub(crate) model_id: String,
    /// Exact reviewed model-package identity.
    pub(crate) package_id: String,
    /// Exact pinned worker runtime identity.
    pub(crate) runtime_id: String,
    /// SPDX license identifier for the selected model package.
    pub(crate) license: String,
    /// Immutable selected package revision.
    pub(crate) source_revision: String,
    /// Exact package bytes required in the app-owned model cache.
    pub(crate) expected_disk_bytes: u64,
    /// Measured whole-process peak bytes required by the accepted hardware proof.
    pub(crate) required_memory_bytes: u64,
    /// Coarse fail-closed native readiness state.
    pub(crate) availability: LocalImageAvailability,
}

/// Immutable app-owned locations for native-only local-image inspection.
#[derive(Clone, Debug)]
struct LocalImageInstallationLayout {
    worker_bundle_root: PathBuf,
    worker_executable: PathBuf,
    model_cache_root: PathBuf,
}

/// Serializes expensive read-only readiness checks and moves them off the WebView task.
#[derive(Debug)]
pub(crate) struct LocalImageAvailabilityService {
    layout: LocalImageInstallationLayout,
    selected: Arc<SelectedModelPackage>,
    inspection: tauri::async_runtime::Mutex<()>,
}

impl LocalImageAvailabilityService {
    /// Resolves the one fixed worker and model-cache layout without touching the filesystem.
    pub(crate) fn new(
        resource_directory: impl AsRef<Path>,
        app_data_directory: impl AsRef<Path>,
    ) -> Result<Self, LocalImageServiceError> {
        let resources = safe_absolute_root(resource_directory.as_ref())?;
        let app_data = safe_absolute_root(app_data_directory.as_ref())?;
        let worker_bundle_root = resources.join(LOCAL_IMAGE_WORKER_DIRECTORY);
        let layout = LocalImageInstallationLayout {
            worker_executable: worker_bundle_root.join(LOCAL_IMAGE_WORKER_EXECUTABLE),
            worker_bundle_root,
            model_cache_root: app_data.join(LOCAL_IMAGE_MODEL_CACHE_DIRECTORY),
        };
        let selected = selected_qwen_image_2512_q4_package()
            .map_err(|_| LocalImageServiceError::InvalidPackage)?;
        Ok(Self {
            layout,
            selected: Arc::new(selected),
            inspection: tauri::async_runtime::Mutex::new(()),
        })
    }

    /// Returns exact path-free metadata after one serialized blocking native inspection.
    pub(crate) async fn inspect(
        &self,
    ) -> Result<LocalImageAvailabilityMetadata, LocalImageServiceError> {
        let _inspection = self.inspection.lock().await;
        let layout = self.layout.clone();
        let selected = self.selected.clone();
        tauri::async_runtime::spawn_blocking(move || inspect_installation(&layout, &selected))
            .await
            .map_err(|_| LocalImageServiceError::InspectionUnavailable)?
            .map(|inspection| inspection.metadata)
    }

    /// Re-verifies every readiness gate and returns native locations only to Rust execution.
    pub(crate) async fn inspect_for_execution(
        &self,
    ) -> Result<VerifiedLocalImageInstallation, LocalImageServiceError> {
        let _inspection = self.inspection.lock().await;
        let layout = self.layout.clone();
        let selected = self.selected.clone();
        let inspection =
            tauri::async_runtime::spawn_blocking(move || inspect_installation(&layout, &selected))
                .await
                .map_err(|_| LocalImageServiceError::InspectionUnavailable)??;
        inspection
            .installation
            .ok_or(LocalImageServiceError::NotReady)
    }

    #[cfg(test)]
    /// Returns the fixed worker bundle root for native layout tests only.
    pub(crate) fn worker_bundle_root_for_test(&self) -> &Path {
        &self.layout.worker_bundle_root
    }

    #[cfg(test)]
    /// Returns the fixed worker executable for native layout tests only.
    pub(crate) fn worker_executable_for_test(&self) -> &Path {
        &self.layout.worker_executable
    }

    #[cfg(test)]
    /// Returns the fixed model cache root for native layout tests only.
    pub(crate) fn model_cache_root_for_test(&self) -> &Path {
        &self.layout.model_cache_root
    }
}

struct InspectedInstallation {
    metadata: LocalImageAvailabilityMetadata,
    installation: Option<VerifiedLocalImageInstallation>,
}

fn inspect_installation(
    layout: &LocalImageInstallationLayout,
    selected: &SelectedModelPackage,
) -> Result<InspectedInstallation, LocalImageServiceError> {
    let hardware =
        probe_local_image_hardware().map_err(|_| LocalImageServiceError::InspectionUnavailable)?;
    let mut availability = evaluate_local_image_availability(
        selected,
        hardware,
        WorkerInstallationReadiness::Missing,
        ModelCacheReadiness::Missing,
    );
    if matches!(availability, LocalImageAvailability::WorkerMissing) {
        let worker = inspect_worker_installation(
            &layout.worker_bundle_root,
            &layout.worker_executable,
            selected.evidence(),
        );
        availability = evaluate_local_image_availability(
            selected,
            hardware,
            worker,
            ModelCacheReadiness::Missing,
        );
        if matches!(availability, LocalImageAvailability::ModelMissing) {
            availability = evaluate_local_image_availability(
                selected,
                hardware,
                worker,
                inspect_model_cache(&layout.model_cache_root, selected.manifest()),
            );
        }
    }
    let installation = matches!(availability, LocalImageAvailability::Ready).then(|| {
        VerifiedLocalImageInstallation::verified(
            layout.worker_executable.clone(),
            layout.model_cache_root.clone(),
            selected.manifest().clone(),
        )
    });
    Ok(InspectedInstallation {
        metadata: metadata_for_availability(selected, availability),
        installation,
    })
}

/// Builds the closed path-free response from trusted selected-package data and one coarse state.
pub(crate) fn metadata_for_availability(
    selected: &SelectedModelPackage,
    availability: LocalImageAvailability,
) -> LocalImageAvailabilityMetadata {
    let manifest = selected.manifest();
    LocalImageAvailabilityMetadata {
        model_id: manifest.model_id.clone(),
        package_id: manifest.package_id.clone(),
        runtime_id: manifest.runtime_id.clone(),
        license: manifest.license.clone(),
        source_revision: manifest.source_revision.clone(),
        expected_disk_bytes: manifest.expected_disk_bytes,
        required_memory_bytes: selected.evidence().peak_memory_bytes,
        availability,
    }
}

fn safe_absolute_root(path: &Path) -> Result<PathBuf, LocalImageServiceError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(LocalImageServiceError::UnsafeLayout);
    }
    Ok(path.to_path_buf())
}
