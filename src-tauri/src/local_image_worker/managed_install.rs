//! Native model-file acquisition for the optional Qwen 2.1 installer; never runtime readiness.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    model_acquisition::ModelAcquisition,
    model_cache::{
        CacheError, CacheResumeProgress, inspect_bound_resume, inspect_cached_model_location,
    },
    model_download::{
        DownloadError, ModelDownloadCancellation, ModelDownloadProgress, ModelDownloader,
        ModelSourcePlan,
    },
};

#[path = "managed_install/package.rs"]
mod package;

#[cfg(test)]
use package::RUNTIME_SOURCE_REVISION;

/// Exact path-free facts for the optional model-file download, without runtime-readiness claims.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManagedModelInstallDisclosure {
    /// Exact quantized model repository identity.
    pub(crate) package_id: String,
    /// Immutable model repository commit.
    pub(crate) source_revision: String,
    /// License governing the model, taken from its included terms rather than model-card tags.
    pub(crate) license: String,
    /// Immutable HTTPS document the user must review before agreeing.
    pub(crate) license_url: String,
    /// SHA-256 of the exact terms, binding consent to their contents.
    pub(crate) license_sha256: String,
    /// Exact sum of the installed model files; runtime download is separate.
    pub(crate) expected_disk_bytes: u64,
    /// Number of files that must pass complete verification.
    pub(crate) total_files: u32,
}

/// Explicit acknowledgement carried by a deliberate optional-install action.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ManagedModelInstallApproval {
    /// Exact repository shown in the disclosure.
    pub(crate) package_id: String,
    /// Exact revision shown in the disclosure.
    pub(crate) source_revision: String,
    /// Exact license identifier shown in the disclosure.
    pub(crate) license: String,
    /// Exact terms digest shown in the disclosure.
    pub(crate) license_sha256: String,
    /// Exact model-file byte count shown in the disclosure.
    pub(crate) expected_disk_bytes: u64,
    /// Affirmative license consent; absent and false never permit acquisition.
    #[serde(default)]
    pub(crate) license_accepted: bool,
}

/// Stable native acquisition failures; no source locations or response contents are returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ManagedInstallError {
    /// Embedded reviewed contracts failed validation.
    InvalidPackage,
    /// The user did not explicitly accept every exact disclosed fact.
    ApprovalRequired,
    /// Source transfer or transactional verification failed.
    Download(DownloadError),
    /// Read-only resumable-cache inspection failed.
    Cache(CacheError),
}

/// Verified model files only; this type provides no generation or server-start capability.
pub(crate) struct InstalledModelFiles {
    directory: PathBuf,
}

impl InstalledModelFiles {
    /// Returns the native-only model root for future separately qualified runtime installation.
    pub(crate) fn directory(&self) -> &Path {
        &self.directory
    }
}

/// Reuses Bottie's transactional downloader with revision- and terms-bound explicit consent.
/// The future app adapter must serialize mutation access with other image operations.
pub(crate) struct ManagedModelInstaller {
    plan: ModelSourcePlan,
    downloader: ModelDownloader,
    license_sha256: String,
}

impl ManagedModelInstaller {
    /// Validates embedded source facts without network access, model loading, or cache mutation.
    pub(crate) fn new() -> Result<Self, ManagedInstallError> {
        Self::from_plan(package::source_plan()?)
    }

    /// Builds the consent boundary from native source contracts, never WebView-supplied URLs.
    fn from_plan(plan: ModelSourcePlan) -> Result<Self, ManagedInstallError> {
        let license_sha256 = plan
            .manifest()
            .files
            .iter()
            .find(|file| file.relative_path == "LICENSE")
            .ok_or(ManagedInstallError::InvalidPackage)?
            .sha256
            .clone();
        Ok(Self {
            plan,
            downloader: ModelDownloader::new().map_err(ManagedInstallError::Download)?,
            license_sha256,
        })
    }

    /// Returns only the facts necessary to make the explicit optional download reviewable.
    pub(crate) fn disclosure(&self) -> ManagedModelInstallDisclosure {
        let manifest = self.plan.manifest();
        ManagedModelInstallDisclosure {
            package_id: manifest.package_id.clone(),
            source_revision: manifest.source_revision.clone(),
            license: manifest.license.clone(),
            license_url: format!(
                "https://huggingface.co/{}/blob/{}/LICENSE",
                manifest.package_id, manifest.source_revision
            ),
            license_sha256: self.license_sha256.clone(),
            expected_disk_bytes: manifest.expected_disk_bytes,
            total_files: manifest.files.len() as u32,
        }
    }

    /// Inspects only this exact revision's retained progress without creating or repairing files.
    pub(crate) fn resume_progress(
        &self,
        root: &Path,
    ) -> Result<Option<CacheResumeProgress>, ManagedInstallError> {
        inspect_bound_resume(root, self.plan.manifest(), &self.plan.resume_binding())
            .map_err(ManagedInstallError::Cache)
    }

    /// Requires consent before any filesystem or network work, then verifies and promotes all files.
    /// Cancellation retains only synced, source-bound partials for a later explicitly approved retry.
    pub(crate) async fn download(
        &self,
        root: &Path,
        approval: &ManagedModelInstallApproval,
        cancellation: &ModelDownloadCancellation,
        progress: impl FnMut(ModelDownloadProgress),
    ) -> Result<InstalledModelFiles, ManagedInstallError> {
        self.validate_approval(approval)?;
        if cancellation.is_cancelled() {
            return Err(ManagedInstallError::Download(DownloadError::Cancelled));
        }
        if let Some(files) = inspect_cached_model_location(root, self.plan.manifest())
            .map_err(ManagedInstallError::Cache)?
        {
            return Ok(InstalledModelFiles {
                directory: PathBuf::from(files.model_directory),
            });
        }
        let mut acquisition = ModelAcquisition::new(self.plan.manifest().clone())
            .map_err(|_| ManagedInstallError::InvalidPackage)?;
        acquisition
            .begin_download()
            .map_err(|_| ManagedInstallError::InvalidPackage)?;
        let files = self
            .downloader
            .download_to_cache(&self.plan, &mut acquisition, root, cancellation, progress)
            .await
            .map_err(ManagedInstallError::Download)?;
        Ok(InstalledModelFiles {
            directory: PathBuf::from(files.model_directory),
        })
    }

    /// Binds the affirmative action to the immutable package bytes and the exact license text.
    fn validate_approval(
        &self,
        approval: &ManagedModelInstallApproval,
    ) -> Result<(), ManagedInstallError> {
        let manifest = self.plan.manifest();
        if !approval.license_accepted
            || approval.package_id != manifest.package_id
            || approval.source_revision != manifest.source_revision
            || approval.license != manifest.license
            || approval.license_sha256 != self.license_sha256
            || approval.expected_disk_bytes != manifest.expected_disk_bytes
        {
            return Err(ManagedInstallError::ApprovalRequired);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "managed_install/tests.rs"]
mod tests;
