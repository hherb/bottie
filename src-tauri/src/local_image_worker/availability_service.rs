//! App-owned path resolution and serialized native inspection for local-image readiness.

use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use serde::Serialize;

use super::{
    availability::{
        LocalImageAvailability, ModelCacheReadiness, WorkerInstallationReadiness,
        evaluate_local_image_availability, inspect_model_cache,
    },
    execution::VerifiedLocalImageInstallation,
    hardware::probe_local_image_hardware,
    model_acquisition::ModelPackageManifest,
    model_download::ModelSourcePlan,
    model_package::{SelectedModelPackage, selected_qwen_image_2512_q4_package},
    worker_cache::{
        WorkerCacheError, WorkerImportApproval, import_worker_bundle, resolve_promoted_worker,
    },
};

/// Exact native-only package and location returned only after hardware and worker verification.
#[derive(Clone, Debug)]
pub(crate) struct VerifiedLocalImageAcquisition {
    cache_root: PathBuf,
    selected: Arc<SelectedModelPackage>,
    metadata: LocalImageAvailabilityMetadata,
}

impl VerifiedLocalImageAcquisition {
    /// Returns the fixed app-owned cache root without crossing IPC.
    pub(crate) fn cache_root(&self) -> &Path {
        &self.cache_root
    }

    /// Returns the exact selected manifest retained only by native orchestration.
    pub(crate) fn manifest(&self) -> &ModelPackageManifest {
        self.selected.manifest()
    }

    /// Returns the exact approved source plan retained only by native orchestration.
    pub(crate) fn source_plan(&self) -> &ModelSourcePlan {
        self.selected.source_plan()
    }

    /// Returns the path-free metadata captured by the same fresh prerequisite inspection.
    pub(crate) fn metadata(&self) -> &LocalImageAvailabilityMetadata {
        &self.metadata
    }
}

/// Fixed application-data directory owning promoted private worker bundles.
pub(crate) const LOCAL_IMAGE_WORKER_CACHE_DIRECTORY: &str = "local-image-workers";
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
    /// Exact accepted worker-bundle bytes required in the app-owned worker cache.
    pub(crate) worker_expected_disk_bytes: u64,
    /// Measured whole-process peak bytes required by the accepted hardware proof.
    pub(crate) required_memory_bytes: u64,
    /// Coarse fail-closed native readiness state.
    pub(crate) availability: LocalImageAvailability,
}

/// Immutable app-owned locations for native-only local-image inspection.
#[derive(Clone, Debug)]
struct LocalImageInstallationLayout {
    worker_cache_root: PathBuf,
    model_cache_root: PathBuf,
    worker_executable_name: &'static str,
}

/// Serializes expensive read-only readiness checks and moves them off the WebView task.
#[derive(Debug)]
pub(crate) struct LocalImageAvailabilityService {
    layout: LocalImageInstallationLayout,
    selected: Arc<SelectedModelPackage>,
    inspection: tauri::async_runtime::Mutex<()>,
}

impl LocalImageAvailabilityService {
    /// Resolves the fixed app-owned worker and model-cache layout without touching the filesystem.
    pub(crate) fn new(
        app_data_directory: impl AsRef<Path>,
    ) -> Result<Self, LocalImageServiceError> {
        let app_data = safe_absolute_root(app_data_directory.as_ref())?;
        let selected = selected_qwen_image_2512_q4_package()
            .map_err(|_| LocalImageServiceError::InvalidPackage)?;
        let worker_executable_name = selected
            .runtime()
            .product_executable_name()
            .ok_or(LocalImageServiceError::InvalidPackage)?;
        let layout = LocalImageInstallationLayout {
            worker_cache_root: app_data.join(LOCAL_IMAGE_WORKER_CACHE_DIRECTORY),
            model_cache_root: app_data.join(LOCAL_IMAGE_MODEL_CACHE_DIRECTORY),
            worker_executable_name,
        };
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

    /// Re-verifies exact hardware and worker gates before permitting model cache or network mutation.
    pub(crate) async fn inspect_for_acquisition(
        &self,
    ) -> Result<VerifiedLocalImageAcquisition, LocalImageServiceError> {
        let _inspection = self.inspection.lock().await;
        let layout = self.layout.clone();
        let selected = self.selected.clone();
        let inspected_selected = selected.clone();
        let inspection = tauri::async_runtime::spawn_blocking(move || {
            inspect_installation(&layout, &inspected_selected)
        })
        .await
        .map_err(|_| LocalImageServiceError::InspectionUnavailable)??;
        if !matches!(
            inspection.metadata.availability,
            LocalImageAvailability::ModelMissing | LocalImageAvailability::ModelMismatch
        ) {
            return Err(LocalImageServiceError::NotReady);
        }
        Ok(VerifiedLocalImageAcquisition {
            cache_root: self.layout.model_cache_root.clone(),
            selected,
            metadata: inspection.metadata,
        })
    }

    /// Imports one user-selected exact worker bundle after approval without exposing its location.
    pub(crate) async fn import_worker(
        &self,
        source_root: PathBuf,
        approval: WorkerImportApproval,
    ) -> Result<(), WorkerCacheError> {
        let _inspection = self.inspection.lock().await;
        let cache_root = self.layout.worker_cache_root.clone();
        let layout = self.layout.clone();
        let selected = self.selected.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let inspection = inspect_installation(&layout, &selected)
                .map_err(|_| WorkerCacheError::Unavailable)?;
            if !worker_import_is_eligible(inspection.metadata.availability) {
                return Err(WorkerCacheError::Unavailable);
            }
            import_worker_bundle(
                &cache_root,
                &source_root,
                layout.worker_executable_name,
                &selected.manifest().runtime_id,
                selected.evidence(),
                &approval,
            )
            .map(|_| ())
        })
        .await
        .map_err(|_| WorkerCacheError::Storage)?
    }

    /// Returns the fixed native cache root for read-only acquisition-progress inspection.
    pub(crate) fn acquisition_cache_root(&self) -> PathBuf {
        self.layout.model_cache_root.clone()
    }

    /// Returns the exact selected package for read-only native acquisition-progress inspection.
    pub(crate) fn selected_package(&self) -> Arc<SelectedModelPackage> {
        self.selected.clone()
    }

    #[cfg(test)]
    /// Returns the fixed app-owned worker-cache root for native layout tests only.
    pub(crate) fn worker_cache_root_for_test(&self) -> &Path {
        &self.layout.worker_cache_root
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

/// Returns whether exact worker import is the only missing native readiness prerequisite.
pub(crate) fn worker_import_is_eligible(availability: LocalImageAvailability) -> bool {
    matches!(
        availability,
        LocalImageAvailability::WorkerMissing | LocalImageAvailability::WorkerMismatch
    )
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
    let mut promoted_worker = None;
    if matches!(availability, LocalImageAvailability::WorkerMissing) {
        let resolution = resolve_promoted_worker(
            &layout.worker_cache_root,
            layout.worker_executable_name,
            &selected.manifest().runtime_id,
            selected.evidence(),
        );
        let worker = match resolution {
            Ok(Some(worker)) => {
                promoted_worker = Some(worker);
                WorkerInstallationReadiness::Verified
            }
            Ok(None) => WorkerInstallationReadiness::Missing,
            Err(_) => WorkerInstallationReadiness::Mismatch,
        };
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
    let installation = if matches!(availability, LocalImageAvailability::Ready) {
        promoted_worker.map(|worker| {
            VerifiedLocalImageInstallation::verified(
                worker.executable().to_path_buf(),
                layout.model_cache_root.clone(),
                selected.manifest().clone(),
            )
        })
    } else {
        None
    };
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
        worker_expected_disk_bytes: selected.evidence().worker_bundle_byte_size,
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
