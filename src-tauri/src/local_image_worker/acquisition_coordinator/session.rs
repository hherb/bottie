//! Pure single-slot state machine for local image-model acquisition.

use serde::{Deserialize, Serialize};

use super::super::{
    availability::LocalImageAvailability,
    availability_service::LocalImageAvailabilityMetadata,
    model_cache::CacheResumeProgress,
    model_download::{DownloadError, ModelDownloadCancellation, ModelDownloadProgress},
    model_package::SelectedModelPackage,
};

/// Exact disclosure echoed by an explicit user action before native mutation is permitted.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct LocalImageAcquisitionApproval {
    /// Exact open-weight model identity presented to the user.
    pub(crate) model_id: String,
    /// Exact reviewed package identity presented to the user.
    pub(crate) package_id: String,
    /// Exact pinned runtime identity presented to the user.
    pub(crate) runtime_id: String,
    /// SPDX license identifier presented to the user.
    pub(crate) license: String,
    /// Immutable package revision presented to the user.
    pub(crate) source_revision: String,
    /// Exact package bytes presented to the user.
    pub(crate) expected_disk_bytes: u64,
    /// Exact measured peak-memory requirement presented to the user.
    pub(crate) required_memory_bytes: u64,
    /// Affirmative acknowledgement set only by the explicit install or resume action.
    pub(crate) approved: bool,
}

/// Closed path-free lifecycle presented for the single selected package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LocalImageAcquisitionPhase {
    /// Exact hardware or worker prerequisites are not currently satisfied.
    Unavailable,
    /// The complete disclosure awaits explicit user approval.
    AwaitingApproval,
    /// Exact source bytes are being written into resumable app-owned staging.
    Downloading,
    /// Cancellation was requested and the active network/body wait is unwinding.
    Cancelling,
    /// Durable partial bytes may be resumed only through another explicit action.
    Paused,
    /// All files arrived and exact hashes plus atomic promotion are in progress.
    Verifying,
    /// Exact promoted bytes passed a fresh native readiness inspection.
    Ready,
    /// The operation failed closed and requires another explicit action to retry.
    Failed,
}

/// Stable path-free reason retained after an interrupted or failed acquisition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LocalImageAcquisitionFailure {
    /// The user cancelled after already-synced progress was retained.
    Cancelled,
    /// A valid response ended before every declared byte arrived.
    Interrupted,
    /// A fixed request or package deadline elapsed.
    Timeout,
    /// A request could not reach or complete against the approved source.
    Transport,
    /// Source response metadata no longer matched the exact approved plan.
    SourceMismatch,
    /// Native cache state or exact file integrity failed closed.
    Integrity,
    /// A native-only orchestration invariant failed.
    Internal,
}

/// Complete path-free acquisition status returned by commands and native events.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalImageAcquisitionStatus {
    /// Exact open-weight model identity.
    pub(crate) model_id: String,
    /// Exact reviewed package identity.
    pub(crate) package_id: String,
    /// Exact pinned worker runtime identity.
    pub(crate) runtime_id: String,
    /// SPDX license identifier.
    pub(crate) license: String,
    /// Immutable package revision.
    pub(crate) source_revision: String,
    /// Exact package bytes expected after installation.
    pub(crate) expected_disk_bytes: u64,
    /// Measured whole-process peak-memory requirement.
    pub(crate) required_memory_bytes: u64,
    /// Coarse native readiness captured by the same serialized inspection.
    pub(crate) availability: LocalImageAvailability,
    /// Current closed acquisition lifecycle.
    pub(crate) phase: LocalImageAcquisitionPhase,
    /// Stable terminal or resumable failure, when present.
    pub(crate) failure: Option<LocalImageAcquisitionFailure>,
    /// Exact files already durably downloaded.
    pub(crate) downloaded_files: u32,
    /// Exact file count in the selected manifest.
    pub(crate) total_files: u32,
    /// Exact bytes already durably downloaded.
    pub(crate) downloaded_bytes: u64,
    /// Files that passed exact verification after promotion.
    pub(crate) verified_files: u32,
}

/// Stable command rejection category without native paths, URLs, hashes, or response data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LocalImageAcquisitionErrorCode {
    /// Exact hardware and worker prerequisites are not ready.
    Unavailable,
    /// The explicit acknowledgement did not exactly match the selected disclosure.
    ApprovalMismatch,
    /// One acquisition already owns the single native slot.
    AlreadyActive,
    /// No acquisition is active to cancel.
    NotActive,
    /// Native state failed closed.
    InvalidState,
}

/// Fixed path-free command error for local-image acquisition actions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalImageAcquisitionError {
    /// Stable machine-readable category.
    pub(crate) code: LocalImageAcquisitionErrorCode,
    /// Fixed user-safe explanation.
    pub(crate) message: &'static str,
}

/// Testable single-slot lifecycle that contains no filesystem or network operations.
pub(crate) struct LocalImageAcquisitionSession {
    selected: SelectedModelPackage,
    status: LocalImageAcquisitionStatus,
    cancellation: Option<ModelDownloadCancellation>,
}

impl LocalImageAcquisitionSession {
    /// Creates an idle session from the exact package selected by reviewed native evidence.
    pub(crate) fn new(selected: SelectedModelPackage) -> Self {
        let status = status_for(
            &selected,
            LocalImageAvailability::UnsupportedPlatform,
            LocalImageAcquisitionPhase::Unavailable,
            None,
            None,
        );
        Self {
            selected,
            status,
            cancellation: None,
        }
    }

    /// Returns whether a download or cancellation unwind currently owns the slot.
    pub(crate) fn is_active(&self) -> bool {
        self.cancellation.is_some()
    }

    /// Returns the current immutable path-free snapshot.
    pub(crate) fn status(&self) -> LocalImageAcquisitionStatus {
        self.status.clone()
    }

    /// Reconciles an idle session with fresh readiness and read-only durable staging progress.
    pub(crate) fn refresh_idle(
        &mut self,
        metadata: LocalImageAvailabilityMetadata,
        resume: Option<CacheResumeProgress>,
    ) -> LocalImageAcquisitionStatus {
        if self.is_active() {
            return self.status();
        }
        if !metadata_matches_selected(&self.selected, &metadata) {
            self.status = status_for(
                &self.selected,
                metadata.availability,
                LocalImageAcquisitionPhase::Unavailable,
                None,
                Some(LocalImageAcquisitionFailure::Internal),
            );
            return self.status();
        }
        let phase = match metadata.availability {
            LocalImageAvailability::Ready => LocalImageAcquisitionPhase::Ready,
            LocalImageAvailability::ModelMissing | LocalImageAvailability::ModelMismatch
                if resume.is_some_and(|progress| progress.downloaded_bytes > 0) =>
            {
                LocalImageAcquisitionPhase::Paused
            }
            LocalImageAvailability::ModelMissing | LocalImageAvailability::ModelMismatch => {
                LocalImageAcquisitionPhase::AwaitingApproval
            }
            _ => LocalImageAcquisitionPhase::Unavailable,
        };
        self.status = status_for(&self.selected, metadata.availability, phase, resume, None);
        self.status()
    }

    /// Accepts only an exact affirmative disclosure after fresh hardware and worker verification.
    pub(crate) fn begin(
        &mut self,
        approval: &LocalImageAcquisitionApproval,
        metadata: &LocalImageAvailabilityMetadata,
        resume: Option<CacheResumeProgress>,
    ) -> Result<ModelDownloadCancellation, LocalImageAcquisitionErrorCode> {
        if self.is_active() {
            return Err(LocalImageAcquisitionErrorCode::AlreadyActive);
        }
        if !matches!(
            metadata.availability,
            LocalImageAvailability::ModelMissing | LocalImageAvailability::ModelMismatch
        ) {
            return Err(LocalImageAcquisitionErrorCode::Unavailable);
        }
        if !metadata_matches_selected(&self.selected, metadata)
            || !approval_matches(approval, metadata)
        {
            return Err(LocalImageAcquisitionErrorCode::ApprovalMismatch);
        }
        if !valid_resume(&self.selected, resume) {
            return Err(LocalImageAcquisitionErrorCode::InvalidState);
        }
        let cancellation = ModelDownloadCancellation::default();
        self.cancellation = Some(cancellation.clone());
        self.status = status_for(
            &self.selected,
            metadata.availability,
            LocalImageAcquisitionPhase::Downloading,
            resume,
            None,
        );
        Ok(cancellation)
    }

    /// Applies one monotonic exact progress report from the trusted downloader.
    pub(crate) fn record_progress(
        &mut self,
        progress: ModelDownloadProgress,
    ) -> Result<LocalImageAcquisitionStatus, LocalImageAcquisitionErrorCode> {
        if !self.is_active()
            || !matches!(
                self.status.phase,
                LocalImageAcquisitionPhase::Downloading | LocalImageAcquisitionPhase::Cancelling
            )
            || progress.total_files != self.status.total_files
            || progress.total_bytes != self.status.expected_disk_bytes
            || progress.completed_files < self.status.downloaded_files
            || progress.downloaded_bytes < self.status.downloaded_bytes
            || progress.completed_files > progress.total_files
            || progress.downloaded_bytes > progress.total_bytes
        {
            return Err(LocalImageAcquisitionErrorCode::InvalidState);
        }
        self.status.downloaded_files = progress.completed_files;
        self.status.downloaded_bytes = progress.downloaded_bytes;
        if progress.completed_files == progress.total_files
            && progress.downloaded_bytes == progress.total_bytes
        {
            self.status.phase = LocalImageAcquisitionPhase::Verifying;
        }
        Ok(self.status())
    }

    /// Requests cooperative cancellation without discarding already synced exact partial bytes.
    pub(crate) fn request_cancel(
        &mut self,
    ) -> Result<LocalImageAcquisitionStatus, LocalImageAcquisitionErrorCode> {
        let cancellation = self
            .cancellation
            .as_ref()
            .ok_or(LocalImageAcquisitionErrorCode::NotActive)?;
        cancellation.cancel();
        self.status.phase = LocalImageAcquisitionPhase::Cancelling;
        Ok(self.status())
    }

    /// Completes only after atomic promotion is confirmed by a fresh full readiness inspection.
    pub(crate) fn finish_success(
        &mut self,
        metadata: &LocalImageAvailabilityMetadata,
    ) -> Result<LocalImageAcquisitionStatus, LocalImageAcquisitionErrorCode> {
        if self.cancellation.take().is_none()
            || metadata.availability != LocalImageAvailability::Ready
        {
            return Err(LocalImageAcquisitionErrorCode::InvalidState);
        }
        self.status.phase = LocalImageAcquisitionPhase::Ready;
        self.status.availability = LocalImageAvailability::Ready;
        self.status.failure = None;
        self.status.downloaded_files = self.status.total_files;
        self.status.downloaded_bytes = self.status.expected_disk_bytes;
        self.status.verified_files = self.status.total_files;
        Ok(self.status())
    }

    /// Retains resumable progress and maps native failures to a fixed path-free terminal state.
    pub(crate) fn finish_failure(
        &mut self,
        failure: DownloadError,
    ) -> Result<LocalImageAcquisitionStatus, LocalImageAcquisitionErrorCode> {
        if self.cancellation.take().is_none() {
            return Err(LocalImageAcquisitionErrorCode::InvalidState);
        }
        let (phase, failure) = public_failure(failure);
        self.status.phase = phase;
        self.status.failure = Some(failure);
        self.status.verified_files = 0;
        Ok(self.status())
    }

    /// Clears the active slot and records one fixed invariant failure.
    pub(crate) fn finish_internal_failure(&mut self) -> LocalImageAcquisitionStatus {
        self.cancellation = None;
        self.status.phase = LocalImageAcquisitionPhase::Failed;
        self.status.failure = Some(LocalImageAcquisitionFailure::Internal);
        self.status()
    }
}

fn status_for(
    selected: &SelectedModelPackage,
    availability: LocalImageAvailability,
    phase: LocalImageAcquisitionPhase,
    resume: Option<CacheResumeProgress>,
    failure: Option<LocalImageAcquisitionFailure>,
) -> LocalImageAcquisitionStatus {
    let manifest = selected.manifest();
    let resume = resume.unwrap_or(CacheResumeProgress {
        completed_files: 0,
        downloaded_bytes: 0,
    });
    let ready = phase == LocalImageAcquisitionPhase::Ready;
    LocalImageAcquisitionStatus {
        model_id: manifest.model_id.clone(),
        package_id: manifest.package_id.clone(),
        runtime_id: manifest.runtime_id.clone(),
        license: manifest.license.clone(),
        source_revision: manifest.source_revision.clone(),
        expected_disk_bytes: manifest.expected_disk_bytes,
        required_memory_bytes: selected.evidence().peak_memory_bytes,
        availability,
        phase,
        failure,
        downloaded_files: if ready {
            manifest.files.len() as u32
        } else {
            resume.completed_files
        },
        total_files: manifest.files.len() as u32,
        downloaded_bytes: if ready {
            manifest.expected_disk_bytes
        } else {
            resume.downloaded_bytes
        },
        verified_files: if ready {
            manifest.files.len() as u32
        } else {
            0
        },
    }
}

fn approval_matches(
    approval: &LocalImageAcquisitionApproval,
    metadata: &LocalImageAvailabilityMetadata,
) -> bool {
    approval.approved
        && approval.model_id == metadata.model_id
        && approval.package_id == metadata.package_id
        && approval.runtime_id == metadata.runtime_id
        && approval.license == metadata.license
        && approval.source_revision == metadata.source_revision
        && approval.expected_disk_bytes == metadata.expected_disk_bytes
        && approval.required_memory_bytes == metadata.required_memory_bytes
}

fn metadata_matches_selected(
    selected: &SelectedModelPackage,
    metadata: &LocalImageAvailabilityMetadata,
) -> bool {
    let manifest = selected.manifest();
    metadata.model_id == manifest.model_id
        && metadata.package_id == manifest.package_id
        && metadata.runtime_id == manifest.runtime_id
        && metadata.license == manifest.license
        && metadata.source_revision == manifest.source_revision
        && metadata.expected_disk_bytes == manifest.expected_disk_bytes
        && metadata.required_memory_bytes == selected.evidence().peak_memory_bytes
}

fn valid_resume(selected: &SelectedModelPackage, resume: Option<CacheResumeProgress>) -> bool {
    resume.is_none_or(|progress| {
        progress.completed_files <= selected.manifest().files.len() as u32
            && progress.downloaded_bytes <= selected.manifest().expected_disk_bytes
    })
}

fn public_failure(
    error: DownloadError,
) -> (LocalImageAcquisitionPhase, LocalImageAcquisitionFailure) {
    match error {
        DownloadError::Cancelled => (
            LocalImageAcquisitionPhase::Paused,
            LocalImageAcquisitionFailure::Cancelled,
        ),
        DownloadError::Interrupted => (
            LocalImageAcquisitionPhase::Paused,
            LocalImageAcquisitionFailure::Interrupted,
        ),
        DownloadError::Timeout => (
            LocalImageAcquisitionPhase::Paused,
            LocalImageAcquisitionFailure::Timeout,
        ),
        DownloadError::Transport => (
            LocalImageAcquisitionPhase::Paused,
            LocalImageAcquisitionFailure::Transport,
        ),
        DownloadError::InvalidResponse | DownloadError::InvalidPlan => (
            LocalImageAcquisitionPhase::Failed,
            LocalImageAcquisitionFailure::SourceMismatch,
        ),
        DownloadError::Cache(_) | DownloadError::LimitExceeded => (
            LocalImageAcquisitionPhase::Failed,
            LocalImageAcquisitionFailure::Integrity,
        ),
        DownloadError::ApprovalRequired => (
            LocalImageAcquisitionPhase::Failed,
            LocalImageAcquisitionFailure::Internal,
        ),
    }
}

/// Maps one stable error code to a fixed path-free command error.
pub(super) fn command_error(code: LocalImageAcquisitionErrorCode) -> LocalImageAcquisitionError {
    let message = match code {
        LocalImageAcquisitionErrorCode::Unavailable => {
            "The exact local image worker and hardware prerequisites are not ready."
        }
        LocalImageAcquisitionErrorCode::ApprovalMismatch => {
            "Review the current model disclosure before approving installation."
        }
        LocalImageAcquisitionErrorCode::AlreadyActive => {
            "A local image model acquisition is already active."
        }
        LocalImageAcquisitionErrorCode::NotActive => "No local image model acquisition is active.",
        LocalImageAcquisitionErrorCode::InvalidState => {
            "The local image model acquisition could not continue safely."
        }
    };
    LocalImageAcquisitionError { code, message }
}
