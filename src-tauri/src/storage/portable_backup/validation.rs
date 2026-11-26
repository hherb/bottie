//! Fault type and validation for a snapshot's portable payload, shared by backup, rotation and restore.

use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use sha2::{Digest, Sha256};

use super::{
    ATTACHMENT_TABLE, DERIVATIVE_TABLE, GENERATED_ASSET_TABLE, LEGACY_PORTABLE_BACKUP_VERSION,
    MANIFEST_TABLE, PORTABLE_BACKUP_FORMAT, PORTABLE_BACKUP_VERSION, StorageError, attachment_rows,
    derivative_rows, generated_asset_rows, table_exists,
};

/// Why reading or checking a snapshot's portable payload failed. Restore turns every fault into
/// "Choose a valid Bottie backup."; backup and rotation keep SQLite's code, so a busy or failing disk
/// reads differently from bad content (#263, #264).
#[derive(Debug, PartialEq)]
pub(in crate::storage) enum PayloadFault {
    /// SQLite could not run a read (busy, I/O, corrupt, not a database, ...), or returned a value of the
    /// wrong type, which `cause` reports as "an unexpected database error".
    Sqlite(rusqlite::Error),
    /// The data was read but breaks the payload contract: wrong hash, size, manifest or extra rows.
    Invalid,
    /// The file lacks Bottie's core tables (empty, or another app's database), so restore rejects it.
    NotBottie,
}

impl From<rusqlite::Error> for PayloadFault {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<PayloadFault> for StorageError {
    fn from(_: PayloadFault) -> Self {
        StorageError::invalid_backup()
    }
}

impl PayloadFault {
    /// Path-free cause: SQLite's code and fixed text, never its free-text message, which can name a file.
    pub(in crate::storage) fn cause(&self) -> String {
        match self {
            Self::Sqlite(error) => error.sqlite_error().map_or_else(
                || "an unexpected database error".to_owned(),
                ToString::to_string,
            ),
            Self::Invalid => "the embedded files failed validation".to_owned(),
            Self::NotBottie => "not a Bottie database".to_owned(),
        }
    }

    /// The failure as a backup error that names its cause.
    pub(in crate::storage) fn backup_error(&self) -> StorageError {
        StorageError::backup_because(self.cause())
    }

    /// Whether the file is not a Bottie database, or SQLite reports it corrupt or not a database, so
    /// no later check can pass and restore would reject it too. Every other fault is treated as one that may pass next time.
    pub(in crate::storage) fn is_permanent(&self) -> bool {
        matches!(self, Self::NotBottie)
            || matches!(self, Self::Sqlite(error) if matches!(
                error.sqlite_error_code(),
                Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase)
            ))
    }
}

/// Validates a complete embedded payload and reports whether the legacy backup has one.
pub(in crate::storage) fn validate_portable_payload(
    connection: &Connection,
) -> Result<bool, PayloadFault> {
    let present = [
        MANIFEST_TABLE,
        ATTACHMENT_TABLE,
        DERIVATIVE_TABLE,
        GENERATED_ASSET_TABLE,
    ]
    .into_iter()
    .map(|table| table_exists(connection, table))
    .collect::<Result<Vec<_>, _>>()?;
    if present.iter().all(|value| !value) {
        return Ok(false);
    }
    if !present[..3].iter().all(|value| *value) {
        return Err(PayloadFault::Invalid);
    }
    let manifest: Option<(String, i64)> = connection
        .query_row(
            &format!("SELECT format, version FROM {MANIFEST_TABLE}"),
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let manifest_count: i64 = connection.query_row(
        &format!("SELECT COUNT(*) FROM {MANIFEST_TABLE}"),
        [],
        |row| row.get(0),
    )?;
    let version = manifest
        .as_ref()
        .filter(|(format, _)| format == PORTABLE_BACKUP_FORMAT)
        .map(|(_, version)| *version);
    if manifest_count != 1
        || (version != Some(LEGACY_PORTABLE_BACKUP_VERSION)
            && version != Some(PORTABLE_BACKUP_VERSION))
        || (version == Some(LEGACY_PORTABLE_BACKUP_VERSION) && present[3])
        || (version == Some(PORTABLE_BACKUP_VERSION) && !present[3])
    {
        return Err(PayloadFault::Invalid);
    }
    validate_embedded_rows(connection, ATTACHMENT_TABLE, &attachment_rows(connection)?)?;
    validate_embedded_derivatives(connection, &derivative_rows(connection)?)?;
    if present[3] {
        validate_embedded_rows(
            connection,
            GENERATED_ASSET_TABLE,
            &generated_asset_rows(connection)?,
        )?;
    }
    Ok(true)
}

/// Validates embedded bytes against exact catalog rows and rejects extra payload content.
fn validate_embedded_rows(
    connection: &Connection,
    table: &str,
    expected: &[(String, i64)],
) -> Result<(), PayloadFault> {
    let count: i64 = connection.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })?;
    let mut found = 0;
    for (sha256, byte_size) in expected {
        let bytes: Option<Vec<u8>> = connection
            .query_row(
                &format!("SELECT content FROM {table} WHERE sha256 = ?1 AND byte_size = ?2"),
                params![sha256, byte_size],
                |row| row.get(0),
            )
            .optional()?;
        // Absent rows are files that were missing at backup time; present rows must verify.
        if let Some(bytes) = bytes {
            verify_bytes(&bytes, sha256, *byte_size)?;
            found += 1;
        }
    }
    // Every embedded row must match a catalog row; extra payload content is rejected.
    if count != found {
        return Err(PayloadFault::Invalid);
    }
    Ok(())
}

/// Validates ready derivative format as well as its exact bytes, size, and identity.
fn validate_embedded_derivatives(
    connection: &Connection,
    expected: &[(String, String, i64)],
) -> Result<(), PayloadFault> {
    let count: i64 = connection.query_row(
        &format!("SELECT COUNT(*) FROM {DERIVATIVE_TABLE}"),
        [],
        |row| row.get(0),
    )?;
    let mut found = 0;
    for (sha256, format, byte_size) in expected {
        let bytes: Option<Vec<u8>> = connection
            .query_row(
                &format!(
                    "SELECT content FROM {DERIVATIVE_TABLE}
                     WHERE sha256 = ?1 AND format = ?2 AND byte_size = ?3"
                ),
                params![sha256, format, byte_size],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(bytes) = bytes {
            verify_bytes(&bytes, sha256, *byte_size)?;
            found += 1;
        }
    }
    if count != found {
        return Err(PayloadFault::Invalid);
    }
    Ok(())
}

/// Verifies exact byte length and lowercase SHA-256 content identity.
pub(super) fn verify_bytes(bytes: &[u8], sha256: &str, byte_size: i64) -> Result<(), PayloadFault> {
    if byte_size < 0
        || bytes.len() as i64 != byte_size
        || format!("{:x}", Sha256::digest(bytes)) != sha256
    {
        return Err(PayloadFault::Invalid);
    }
    Ok(())
}
