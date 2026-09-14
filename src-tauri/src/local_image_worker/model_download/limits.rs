//! Fixed byte, time, progress, and cancellation boundaries for model downloads.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use serde::Serialize;
use tokio::sync::Notify;

/// Production connection deadline for one repository request.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Production ceiling for one model file transfer.
const FILE_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
/// Production ceiling for one complete package transfer.
const PACKAGE_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(6 * 60 * 60);
/// Maximum file size accepted by the generic local-model download boundary.
const MAX_FILE_BYTES: u64 = 32 * 1_024 * 1_024 * 1_024;
/// Maximum package size accepted by the generic local-model download boundary.
const MAX_PACKAGE_BYTES: u64 = 64 * 1_024 * 1_024 * 1_024;
/// Durable progress cadence used to avoid syncing on every network frame.
const PROGRESS_SYNC_BYTES: u64 = 4 * 1_024 * 1_024;

/// Fixed operational ceilings for a model download session.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ModelDownloadLimits {
    pub(super) connect_timeout: Duration,
    pub(super) file_timeout: Duration,
    pub(super) package_timeout: Duration,
    pub(super) max_file_bytes: u64,
    pub(super) max_package_bytes: u64,
    pub(super) progress_sync_bytes: u64,
}

impl Default for ModelDownloadLimits {
    /// Returns production deadlines, size ceilings, and durable progress cadence.
    fn default() -> Self {
        Self {
            connect_timeout: CONNECT_TIMEOUT,
            file_timeout: FILE_DOWNLOAD_TIMEOUT,
            package_timeout: PACKAGE_DOWNLOAD_TIMEOUT,
            max_file_bytes: MAX_FILE_BYTES,
            max_package_bytes: MAX_PACKAGE_BYTES,
            progress_sync_bytes: PROGRESS_SYNC_BYTES,
        }
    }
}

impl ModelDownloadLimits {
    /// Creates small deterministic limits for loopback response tests.
    #[cfg(test)]
    pub(crate) fn for_fixture(
        connect_timeout: Duration,
        file_timeout: Duration,
        package_timeout: Duration,
        max_file_bytes: u64,
        max_package_bytes: u64,
        progress_sync_bytes: u64,
    ) -> Self {
        Self {
            connect_timeout,
            file_timeout,
            package_timeout,
            max_file_bytes,
            max_package_bytes,
            progress_sync_bytes,
        }
    }

    pub(super) fn valid(&self) -> bool {
        !self.connect_timeout.is_zero()
            && !self.file_timeout.is_zero()
            && !self.package_timeout.is_zero()
            && self.max_file_bytes > 0
            && self.max_package_bytes >= self.max_file_bytes
            && self.progress_sync_bytes > 0
    }
}

/// Cloneable permanent cancellation signal for one explicit model acquisition.
#[derive(Clone, Debug, Default)]
pub(crate) struct ModelDownloadCancellation {
    cancelled: Arc<AtomicBool>,
    notification: Arc<Notify>,
}

impl ModelDownloadCancellation {
    /// Requests cancellation and wakes the current request or body wait.
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.notification.notify_waiters();
    }

    /// Returns whether cancellation was already requested.
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(super) async fn cancelled(&self) {
        let notified = self.notification.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.is_cancelled() {
            return;
        }
        notified.await;
    }
}

/// Path-free durable transfer progress suitable for a later native command adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelDownloadProgress {
    /// Files whose exact size and digest have completed.
    pub(crate) completed_files: u32,
    /// Total files declared by the exact manifest.
    pub(crate) total_files: u32,
    /// Bytes known synced across all retained package files.
    pub(crate) downloaded_bytes: u64,
    /// Exact total bytes declared by the manifest.
    pub(crate) total_bytes: u64,
}
