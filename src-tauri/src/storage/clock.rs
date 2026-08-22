//! Shared durable-storage clock boundary.

use super::StorageError;

/// Returns the current Unix epoch timestamp in milliseconds.
pub(super) fn now_ms() -> Result<i64, StorageError> {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| StorageError::internal())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| StorageError::internal())
}
