//! Consistent manual snapshots of Bottie's live SQLite conversation store.

use std::path::Path;

use rusqlite::{Connection, MAIN_DB};

use super::{ConversationStore, StorageError};

impl ConversationStore {
    /// Copies every committed page to an independently readable database through SQLite's online backup API.
    pub(crate) fn backup_to(&self, destination: &Path) -> Result<(), StorageError> {
        if paths_refer_to_same_file(&self.path, destination) {
            return Err(StorageError::invalid(
                "Choose a different location for the Bottie backup.",
            ));
        }
        let source = self.open()?;
        source
            .backup(MAIN_DB, destination, None)
            .map_err(|_| StorageError::backup())?;
        verify_backup(destination)
    }
}

/// Returns whether two existing paths resolve to the same filesystem entry.
fn paths_refer_to_same_file(source: &Path, destination: &Path) -> bool {
    if source == destination {
        return true;
    }
    match (
        std::fs::canonicalize(source),
        std::fs::canonicalize(destination),
    ) {
        (Ok(source), Ok(destination)) => source == destination,
        _ => false,
    }
}

/// Reopens the completed snapshot and rejects output that fails SQLite's quick integrity check.
fn verify_backup(path: &Path) -> Result<(), StorageError> {
    let connection = Connection::open(path).map_err(|_| StorageError::backup())?;
    let integrity: String = connection
        .pragma_query_value(None, "quick_check", |row| row.get(0))
        .map_err(|_| StorageError::backup())?;
    if integrity == "ok" {
        Ok(())
    } else {
        Err(StorageError::backup())
    }
}
