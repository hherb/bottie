//! Stable path- and database-redacted storage errors.

use serde::Serialize;

/// Stable storage failure returned across the native command boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StorageError {
    /// Stable machine-readable failure category.
    pub(crate) code: &'static str,
    /// Human-readable failure safe to show in the interface.
    pub(crate) message: String,
}

impl StorageError {
    /// Creates an invalid-input failure.
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: "invalid_request",
            message: message.into(),
        }
    }

    /// Creates a missing-record failure.
    pub(super) fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: "not_found",
            message: message.into(),
        }
    }

    /// Creates an internal storage failure without exposing SQL or local paths.
    pub(crate) fn internal() -> Self {
        Self {
            code: "internal",
            message: "Bottie could not access its local conversation store.".into(),
        }
    }

    /// Creates a stable failure while corrupt local data is awaiting guided recovery.
    pub(super) fn recovery_required() -> Self {
        Self {
            code: "recovery_required",
            message: "Bottie paused local conversation access until its data is recovered.".into(),
        }
    }

    /// Creates a path-redacted file-export failure.
    pub(crate) fn export() -> Self {
        Self {
            code: "internal",
            message: "Bottie could not save the conversation export.".into(),
        }
    }

    /// Creates a source-path-redacted attachment read failure.
    pub(crate) fn attachment_read() -> Self {
        Self {
            code: "invalid_request",
            message: "Bottie could not read that attachment.".into(),
        }
    }

    /// Creates a path-redacted SQLite-backup failure.
    pub(crate) fn backup() -> Self {
        Self {
            code: "internal",
            message: "Bottie could not save the local data backup.".into(),
        }
    }

    /// Creates a path-redacted automatic-backup rotation failure.
    pub(crate) fn automatic_backup() -> Self {
        Self {
            code: "internal",
            message: "Bottie could not update its automatic local backups.".into(),
        }
    }

    /// Creates a validation failure for a selected database that is not a supported Bottie backup.
    pub(crate) fn invalid_backup() -> Self {
        Self {
            code: "invalid_request",
            message: "Choose a valid Bottie backup.".into(),
        }
    }

    /// Creates a restore failure when native provider work still owns the live store.
    pub(crate) fn restore_while_active() -> Self {
        Self {
            code: "invalid_request",
            message: "Wait for the active response to finish before restoring local data.".into(),
        }
    }

    /// Creates a path-redacted SQLite-restore failure.
    pub(crate) fn restore() -> Self {
        Self {
            code: "internal",
            message: "Bottie could not restore the local data backup. The pre-restore safety copy was kept.".into(),
        }
    }

    /// Creates a failure when Bottie cannot preserve live data before restore.
    pub(crate) fn restore_safety_copy() -> Self {
        Self {
            code: "internal",
            message: "Bottie could not create the pre-restore safety copy. Your current data was not changed.".into(),
        }
    }

    /// Creates a failure when damaged database files cannot be preserved before recovery.
    pub(crate) fn recovery_preservation() -> Self {
        Self {
            code: "internal",
            message: "Bottie could not preserve the damaged local data. Nothing was replaced."
                .into(),
        }
    }

    /// Creates a stable failure when guided recovery has no verified managed snapshot.
    pub(crate) fn no_automatic_backup() -> Self {
        Self {
            code: "invalid_request",
            message: "No verified automatic backup is available.".into(),
        }
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(_: rusqlite::Error) -> Self {
        Self::internal()
    }
}

impl From<std::io::Error> for StorageError {
    fn from(_: std::io::Error) -> Self {
        Self::internal()
    }
}
