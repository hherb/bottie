//! Exact path-free IPC outcomes for native backup and restore workflows.

use std::path::Path;

use serde::Serialize;

/// Result of one native backup Save-dialog interaction without exposing a filesystem path.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupOutcome {
    /// Whether a complete verified snapshot was written or the user cancelled the dialog.
    status: BackupStatus,
    /// Saved leaf filename, absent when the dialog was cancelled.
    file_name: Option<String>,
}

/// Stable native backup outcomes returned to the presentation layer.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum BackupStatus {
    /// The SQLite snapshot was written and verified successfully.
    Saved,
    /// The user closed the native dialog without selecting a destination.
    Cancelled,
}

/// Result of one native restore interaction without exposing any filesystem path.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreOutcome {
    /// Whether a validated backup was restored or the user cancelled the interaction.
    status: RestoreStatus,
    /// Selected backup's leaf filename, absent when the interaction was cancelled.
    file_name: Option<String>,
    /// Application-private safety file or directory name, absent when cancelled.
    preserved_copy_name: Option<String>,
}

/// Stable native restore outcomes returned to the presentation layer.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum RestoreStatus {
    /// The selected backup replaced the live store after validation and safety copy.
    Restored,
    /// The user closed either native dialog without completing a restore.
    Cancelled,
}

/// Builds the neutral outcome for a cancelled native backup picker.
pub(super) fn cancelled_backup() -> BackupOutcome {
    BackupOutcome {
        status: BackupStatus::Cancelled,
        file_name: None,
    }
}

/// Builds a saved-backup outcome from only the selected leaf filename.
pub(super) fn saved_backup(path: &Path, fallback: &str) -> BackupOutcome {
    BackupOutcome {
        status: BackupStatus::Saved,
        file_name: Some(leaf_name(path, fallback)),
    }
}

/// Builds the neutral outcome shared by restore picker and confirmation cancellation.
pub(super) fn cancelled_restore() -> RestoreOutcome {
    RestoreOutcome {
        status: RestoreStatus::Cancelled,
        file_name: None,
        preserved_copy_name: None,
    }
}

/// Builds a restored outcome from path-free selected and preserved leaf labels.
pub(super) fn restored(
    selected_name: String,
    preserved_path: &Path,
    preserved_fallback: &str,
) -> RestoreOutcome {
    RestoreOutcome {
        status: RestoreStatus::Restored,
        file_name: Some(selected_name),
        preserved_copy_name: Some(leaf_name(preserved_path, preserved_fallback)),
    }
}

/// Returns one Unicode leaf filename or a stable path-free fallback.
pub(super) fn leaf_name(path: &Path, fallback: &str) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(fallback)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use std::{io, path::Path};

    use serde_json::json;

    use super::*;
    use crate::storage::StorageError;

    #[test]
    fn backup_and_restore_outcomes_serialize_only_leaf_metadata() {
        let backup = saved_backup(
            Path::new("/Users/alice/Documents/private/bottie backup.sqlite3"),
            "bottie-backup.sqlite3",
        );
        let restore = restored(
            "selected backup.sqlite3".into(),
            Path::new("/Users/alice/Library/Application Support/Bottie/pre-restore.sqlite3"),
            "Bottie preserved local data",
        );

        assert_eq!(
            serde_json::to_value(backup).expect("backup outcome should serialize"),
            json!({"status": "saved", "fileName": "bottie backup.sqlite3"})
        );
        assert_eq!(
            serde_json::to_value(restore).expect("restore outcome should serialize"),
            json!({
                "status": "restored",
                "fileName": "selected backup.sqlite3",
                "preservedCopyName": "pre-restore.sqlite3"
            })
        );
    }

    #[test]
    fn cancelled_file_workflows_return_no_filename_or_path() {
        assert_eq!(
            serde_json::to_value(cancelled_backup()).expect("backup cancellation should serialize"),
            json!({"status": "cancelled", "fileName": null})
        );
        assert_eq!(
            serde_json::to_value(cancelled_restore())
                .expect("restore cancellation should serialize"),
            json!({
                "status": "cancelled",
                "fileName": null,
                "preservedCopyName": null
            })
        );
    }

    #[test]
    fn filesystem_failures_serialize_without_native_error_detail() {
        let errors = [
            StorageError::from(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "permission denied: /Users/alice/private/bottie.sqlite3",
            )),
            StorageError::attachment_read(),
            StorageError::export(),
            StorageError::backup(),
            StorageError::invalid_backup(),
            StorageError::restore(),
            StorageError::restore_safety_copy(),
        ];
        let serialized = serde_json::to_string(&errors).expect("storage failures should serialize");

        assert!(!serialized.contains("/Users/alice"));
        assert!(!serialized.contains("permission denied"));
        assert!(!serialized.contains("sqlite3"));
        assert!(serialized.contains("invalid_request"));
        assert!(serialized.contains("internal"));
    }
}
