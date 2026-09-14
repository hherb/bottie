//! Strict source planning and resumable HTTP download policy for local image-model packages.

use std::{path::Path, time::Duration};

use futures_util::StreamExt;
use reqwest::{
    Client, RequestBuilder,
    header::{IF_RANGE, RANGE},
};
use tokio::time::Instant;

use super::{
    model_acquisition::{AcquisitionPhase, ModelAcquisition},
    model_cache::{CacheError, CacheFileWriter, CacheWriteStatus, ModelCacheTransaction},
    protocol::ModelLocation,
};

#[path = "model_download/limits.rs"]
mod limits;
#[path = "model_download/response.rs"]
mod response;
#[path = "model_download/source.rs"]
mod source;

use response::{
    validate_direct_response, validate_hugging_face_resolution, validate_resolved_response,
};
use source::SourceDelivery;

pub(crate) use limits::{ModelDownloadCancellation, ModelDownloadLimits, ModelDownloadProgress};
pub(crate) use source::{ModelFileSource, ModelSourcePlan};
/// Stable download failures without source URLs, cache paths, hashes, or response bodies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DownloadError {
    /// The repository, revision, file mapping, or entity validator was not an exact approved plan.
    InvalidPlan,
    /// The exact acquisition has not passed its explicit approval phase.
    ApprovalRequired,
    /// Declared file or package bytes exceed the downloader's configured ceilings.
    LimitExceeded,
    /// HTTP status or range, validator, offset, or length metadata did not match the plan.
    InvalidResponse,
    /// The request could not be completed before receiving a valid response.
    Transport,
    /// A valid response stopped before every declared byte arrived.
    Interrupted,
    /// A configured per-file or package deadline elapsed.
    Timeout,
    /// The caller requested cancellation.
    Cancelled,
    /// App-owned cache validation or durable storage failed.
    Cache(CacheError),
}

/// Native downloader with no automatic redirects that feeds only the transactional app-owned cache.
#[derive(Clone)]
pub(crate) struct ModelDownloader {
    client: Client,
    limits: ModelDownloadLimits,
}

impl ModelDownloader {
    /// Creates the production downloader with fixed redirects, connection, time, and byte policy.
    pub(crate) fn new() -> Result<Self, DownloadError> {
        Self::build(ModelDownloadLimits::default())
    }

    /// Creates a downloader with small deterministic fixture limits.
    #[cfg(test)]
    pub(crate) fn for_fixture(limits: ModelDownloadLimits) -> Result<Self, DownloadError> {
        Self::build(limits)
    }

    fn build(limits: ModelDownloadLimits) -> Result<Self, DownloadError> {
        if !limits.valid() {
            return Err(DownloadError::InvalidPlan);
        }
        let client = Client::builder()
            .connect_timeout(limits.connect_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| DownloadError::Transport)?;
        Ok(Self { client, limits })
    }

    /// Downloads every planned file, reporting only synced progress, then verifies and promotes it.
    pub(crate) async fn download_to_cache(
        &self,
        plan: &ModelSourcePlan,
        acquisition: &mut ModelAcquisition,
        cache_root: &Path,
        cancellation: &ModelDownloadCancellation,
        mut progress: impl FnMut(ModelDownloadProgress),
    ) -> Result<ModelLocation, DownloadError> {
        self.validate_limits(plan)?;
        if acquisition.manifest() != plan.manifest()
            || acquisition.status().phase != AcquisitionPhase::Downloading
        {
            return Err(DownloadError::ApprovalRequired);
        }
        if cancellation.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }
        let transaction = ModelCacheTransaction::open_bound(
            cache_root,
            plan.manifest.clone(),
            &plan.resume_binding(),
        )
        .map_err(DownloadError::Cache)?;
        let package_deadline = Instant::now() + self.limits.package_timeout;
        let mut offsets = Vec::with_capacity(plan.files.len());
        let mut completed_files = 0_u32;
        let mut downloaded_bytes = 0_u64;
        for source in &plan.files {
            let offset = transaction
                .resume_offset(&source.relative_path)
                .map_err(DownloadError::Cache)?;
            if offset == contract(plan, source)?.byte_size {
                completed_files += 1;
            }
            downloaded_bytes = downloaded_bytes
                .checked_add(offset)
                .ok_or(DownloadError::LimitExceeded)?;
            offsets.push(offset);
        }
        if downloaded_bytes > 0 {
            report_progress(
                acquisition,
                &mut progress,
                download_progress(plan, completed_files, downloaded_bytes),
            )?;
        }

        for (source, offset) in plan.files.iter().zip(offsets) {
            let file_size = contract(plan, source)?.byte_size;
            if offset == file_size {
                continue;
            }
            let other_durable_bytes = downloaded_bytes - offset;
            let result = self
                .download_file(
                    plan,
                    source,
                    &transaction,
                    offset,
                    other_durable_bytes,
                    completed_files,
                    package_deadline,
                    cancellation,
                    acquisition,
                    &mut progress,
                )
                .await;
            if let Err(error) = result {
                if discards_transaction(error) {
                    transaction.discard().map_err(DownloadError::Cache)?;
                }
                return Err(error);
            }
            completed_files += 1;
            downloaded_bytes = other_durable_bytes + file_size;
            report_progress(
                acquisition,
                &mut progress,
                download_progress(plan, completed_files, downloaded_bytes),
            )?;
        }
        let promoted = transaction.promote().map_err(DownloadError::Cache)?;
        acquisition
            .begin_verification()
            .map_err(|_| DownloadError::Cache(CacheError::InvalidState))?;
        acquisition
            .activate(Path::new(&promoted.model_directory))
            .map_err(|_| DownloadError::Cache(CacheError::Integrity))
    }

    #[allow(clippy::too_many_arguments)]
    async fn download_file(
        &self,
        plan: &ModelSourcePlan,
        source: &ModelFileSource,
        transaction: &ModelCacheTransaction,
        offset: u64,
        other_durable_bytes: u64,
        completed_files: u32,
        package_deadline: Instant,
        cancellation: &ModelDownloadCancellation,
        acquisition: &mut ModelAcquisition,
        progress: &mut impl FnMut(ModelDownloadProgress),
    ) -> Result<(), DownloadError> {
        let contract = contract(plan, source)?;
        let deadline = package_deadline.min(Instant::now() + self.limits.file_timeout);
        let mut request = self.client.get(plan.source_url(source)?);
        if offset > 0 {
            request = request
                .header(RANGE, format!("bytes={offset}-"))
                .header(IF_RANGE, &source.strong_etag);
        }
        let mut response = self.send_request(request, deadline, cancellation).await?;
        match plan.delivery() {
            SourceDelivery::Direct => {
                validate_direct_response(
                    &response,
                    offset,
                    contract.byte_size,
                    &source.strong_etag,
                )?;
            }
            SourceDelivery::HuggingFace => {
                let resolved_url =
                    validate_hugging_face_resolution(&response, plan, source, contract.byte_size)?;
                let mut resolved_request = self.client.get(resolved_url);
                if offset > 0 {
                    resolved_request = resolved_request.header(RANGE, format!("bytes={offset}-"));
                }
                response = self
                    .send_request(resolved_request, deadline, cancellation)
                    .await?;
                validate_resolved_response(&response, offset, contract.byte_size)?;
            }
        };
        let mut writer = transaction
            .begin_file_write(&source.relative_path, offset)
            .map_err(DownloadError::Cache)?;
        let mut stream = response.bytes_stream();
        let mut last_published = offset;
        loop {
            let remaining = match remaining(deadline) {
                Ok(value) => value,
                Err(error) => {
                    return retain_writer(
                        writer,
                        error,
                        plan,
                        other_durable_bytes,
                        completed_files,
                        acquisition,
                        progress,
                    );
                }
            };
            let item = tokio::select! {
                biased;
                _ = cancellation.cancelled() => {
                    return retain_writer(
                        writer,
                        DownloadError::Cancelled,
                        plan,
                        other_durable_bytes,
                        completed_files,
                        acquisition,
                        progress,
                    );
                }
                result = tokio::time::timeout(remaining, stream.next()) => {
                    match result {
                        Err(_) => {
                            return retain_writer(
                                writer,
                                DownloadError::Timeout,
                                plan,
                                other_durable_bytes,
                                completed_files,
                                acquisition,
                                progress,
                            );
                        }
                        Ok(item) => item,
                    }
                }
            };
            let Some(item) = item else { break };
            let chunk = match item {
                Ok(chunk) => chunk,
                Err(error) => {
                    let failure = if error.is_timeout() {
                        DownloadError::Timeout
                    } else {
                        DownloadError::Interrupted
                    };
                    return retain_writer(
                        writer,
                        failure,
                        plan,
                        other_durable_bytes,
                        completed_files,
                        acquisition,
                        progress,
                    );
                }
            };
            writer.append(&chunk).map_err(DownloadError::Cache)?;
            if writer.appended_bytes().saturating_sub(last_published)
                >= self.limits.progress_sync_bytes
            {
                let synced = writer.sync_progress().map_err(DownloadError::Cache)?;
                last_published = synced;
                report_progress(
                    acquisition,
                    progress,
                    download_progress(plan, completed_files, other_durable_bytes + synced),
                )?;
            }
        }
        let final_bytes = writer.appended_bytes();
        match writer.finish().map_err(DownloadError::Cache)? {
            CacheWriteStatus::Complete => Ok(()),
            CacheWriteStatus::Incomplete => {
                report_progress(
                    acquisition,
                    progress,
                    download_progress(plan, completed_files, other_durable_bytes + final_bytes),
                )?;
                Err(DownloadError::Interrupted)
            }
        }
    }

    async fn send_request(
        &self,
        request: RequestBuilder,
        deadline: Instant,
        cancellation: &ModelDownloadCancellation,
    ) -> Result<reqwest::Response, DownloadError> {
        let request_remaining = remaining(deadline)?;
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(DownloadError::Cancelled),
            result = tokio::time::timeout(
                request_remaining,
                request.timeout(request_remaining).send(),
            ) => {
                match result {
                    Err(_) => Err(DownloadError::Timeout),
                    Ok(Err(error)) if error.is_timeout() => Err(DownloadError::Timeout),
                    Ok(Err(_)) => Err(DownloadError::Transport),
                    Ok(Ok(response)) => Ok(response),
                }
            }
        }
    }

    fn validate_limits(&self, plan: &ModelSourcePlan) -> Result<(), DownloadError> {
        if plan.manifest.expected_disk_bytes > self.limits.max_package_bytes
            || plan
                .manifest
                .files
                .iter()
                .any(|file| file.byte_size > self.limits.max_file_bytes)
        {
            Err(DownloadError::LimitExceeded)
        } else {
            Ok(())
        }
    }
}

fn contract<'a>(
    plan: &'a ModelSourcePlan,
    source: &ModelFileSource,
) -> Result<&'a super::model_acquisition::ModelFileContract, DownloadError> {
    plan.manifest
        .files
        .iter()
        .find(|file| file.relative_path == source.relative_path)
        .ok_or(DownloadError::InvalidPlan)
}

fn remaining(deadline: Instant) -> Result<Duration, DownloadError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or(DownloadError::Timeout)
}

fn retain_writer(
    mut writer: CacheFileWriter,
    error: DownloadError,
    plan: &ModelSourcePlan,
    other_durable_bytes: u64,
    completed_files: u32,
    acquisition: &mut ModelAcquisition,
    progress: &mut impl FnMut(ModelDownloadProgress),
) -> Result<(), DownloadError> {
    let synced = writer.sync_progress().map_err(DownloadError::Cache)?;
    report_progress(
        acquisition,
        progress,
        download_progress(plan, completed_files, other_durable_bytes + synced),
    )?;
    drop(writer);
    Err(error)
}

fn report_progress(
    acquisition: &mut ModelAcquisition,
    progress: &mut impl FnMut(ModelDownloadProgress),
    status: ModelDownloadProgress,
) -> Result<(), DownloadError> {
    acquisition
        .record_download_progress(status.completed_files, status.downloaded_bytes)
        .map_err(|_| DownloadError::Cache(CacheError::InvalidState))?;
    progress(status);
    Ok(())
}

fn download_progress(
    plan: &ModelSourcePlan,
    completed_files: u32,
    downloaded_bytes: u64,
) -> ModelDownloadProgress {
    ModelDownloadProgress {
        completed_files,
        total_files: plan.manifest.files.len() as u32,
        downloaded_bytes,
        total_bytes: plan.manifest.expected_disk_bytes,
    }
}

fn discards_transaction(error: DownloadError) -> bool {
    matches!(
        error,
        DownloadError::InvalidPlan
            | DownloadError::InvalidResponse
            | DownloadError::LimitExceeded
            | DownloadError::Cache(CacheError::Integrity | CacheError::InvalidPath)
    )
}
