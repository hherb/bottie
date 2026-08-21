//! Portable attachment payloads embedded only inside Bottie SQLite backup files.

use std::{fs, path::Path};

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use sha2::{Digest, Sha256};

use super::StorageError;

const PORTABLE_BACKUP_FORMAT: &str = "bottie-portable-backup";
const PORTABLE_BACKUP_VERSION: i64 = 1;
const MANIFEST_TABLE: &str = "bottie_portable_manifest";
const ATTACHMENT_TABLE: &str = "bottie_portable_attachment_blobs";
const DERIVATIVE_TABLE: &str = "bottie_portable_image_derivatives";
const BLOB_DIRECTORY: &str = "blobs";
const NORMALIZED_DIRECTORY: &str = "normalized-images";

/// Adds verified source and derivative bytes to an independently copied SQLite snapshot.
pub(super) fn embed_portable_payload(
    database: &Path,
    attachment_root: &Path,
) -> Result<(), StorageError> {
    let mut connection = Connection::open(database).map_err(|_| StorageError::backup())?;
    connection
        .pragma_update(None, "journal_mode", "DELETE")
        .map_err(|_| StorageError::backup())?;
    let attachments = attachment_rows(&connection).map_err(|_| StorageError::backup())?;
    let derivatives = derivative_rows(&connection).map_err(|_| StorageError::backup())?;
    let transaction = connection
        .transaction()
        .map_err(|_| StorageError::backup())?;
    transaction
        .execute_batch(&format!(
            "DROP TABLE IF EXISTS {MANIFEST_TABLE};
             DROP TABLE IF EXISTS {ATTACHMENT_TABLE};
             DROP TABLE IF EXISTS {DERIVATIVE_TABLE};
             CREATE TABLE {MANIFEST_TABLE} (
                 format TEXT PRIMARY KEY NOT NULL,
                 version INTEGER NOT NULL
             ) STRICT;
             CREATE TABLE {ATTACHMENT_TABLE} (
                 sha256 TEXT PRIMARY KEY NOT NULL,
                 byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
                 content BLOB NOT NULL
             ) STRICT;
             CREATE TABLE {DERIVATIVE_TABLE} (
                 sha256 TEXT PRIMARY KEY NOT NULL,
                 format TEXT NOT NULL CHECK (format IN ('jpeg', 'png')),
                 byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
                 content BLOB NOT NULL
             ) STRICT;"
        ))
        .map_err(|_| StorageError::backup())?;
    transaction
        .execute(
            &format!("INSERT INTO {MANIFEST_TABLE} (format, version) VALUES (?1, ?2)"),
            params![PORTABLE_BACKUP_FORMAT, PORTABLE_BACKUP_VERSION],
        )
        .map_err(|_| StorageError::backup())?;
    for (sha256, byte_size) in attachments {
        let path = content_path(attachment_root, BLOB_DIRECTORY, &sha256, None)?;
        let bytes = verified_file(&path, &sha256, byte_size).map_err(|_| StorageError::backup())?;
        transaction
            .execute(
                &format!(
                    "INSERT INTO {ATTACHMENT_TABLE} (sha256, byte_size, content) VALUES (?1, ?2, ?3)"
                ),
                params![sha256, byte_size, bytes],
            )
            .map_err(|_| StorageError::backup())?;
    }
    for (sha256, format, byte_size) in derivatives {
        let path = content_path(
            attachment_root,
            NORMALIZED_DIRECTORY,
            &sha256,
            Some(&format),
        )?;
        let bytes = verified_file(&path, &sha256, byte_size).map_err(|_| StorageError::backup())?;
        transaction
            .execute(
                &format!(
                    "INSERT INTO {DERIVATIVE_TABLE} (sha256, format, byte_size, content)
                     VALUES (?1, ?2, ?3, ?4)"
                ),
                params![sha256, format, byte_size, bytes],
            )
            .map_err(|_| StorageError::backup())?;
    }
    transaction.commit().map_err(|_| StorageError::backup())?;
    validate_portable_payload(&connection)
        .map(|_| ())
        .map_err(|_| StorageError::backup())
}

/// Validates a complete embedded payload and reports whether the legacy backup has one.
pub(super) fn validate_portable_payload(connection: &Connection) -> Result<bool, StorageError> {
    let present = [MANIFEST_TABLE, ATTACHMENT_TABLE, DERIVATIVE_TABLE]
        .into_iter()
        .map(|table| table_exists(connection, table))
        .collect::<Result<Vec<_>, _>>()?;
    if present.iter().all(|value| !value) {
        return Ok(false);
    }
    if !present.iter().all(|value| *value) {
        return Err(StorageError::invalid_backup());
    }
    let manifest: Option<(String, i64)> = connection
        .query_row(
            &format!("SELECT format, version FROM {MANIFEST_TABLE}"),
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| StorageError::invalid_backup())?;
    let manifest_count: i64 = connection
        .query_row(
            &format!("SELECT COUNT(*) FROM {MANIFEST_TABLE}"),
            [],
            |row| row.get(0),
        )
        .map_err(|_| StorageError::invalid_backup())?;
    if manifest_count != 1
        || manifest != Some((PORTABLE_BACKUP_FORMAT.into(), PORTABLE_BACKUP_VERSION))
    {
        return Err(StorageError::invalid_backup());
    }
    validate_embedded_rows(connection, ATTACHMENT_TABLE, &attachment_rows(connection)?)?;
    validate_embedded_derivatives(connection, &derivative_rows(connection)?)?;
    Ok(true)
}

/// Rehydrates a verified portable payload into a new application-private attachment root.
pub(super) fn extract_portable_payload(
    database: &Path,
    destination: &Path,
) -> Result<bool, StorageError> {
    let connection = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| StorageError::invalid_backup())?;
    if !validate_portable_payload(&connection)? {
        return Ok(false);
    }
    fs::create_dir(destination).map_err(|_| StorageError::restore())?;
    let result = (|| {
        extract_table(
            &connection,
            ATTACHMENT_TABLE,
            destination,
            BLOB_DIRECTORY,
            false,
        )?;
        extract_table(
            &connection,
            DERIVATIVE_TABLE,
            destination,
            NORMALIZED_DIRECTORY,
            true,
        )?;
        Ok(true)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    result
}

/// Removes backup-only payload tables before a staged database becomes the live store.
pub(super) fn strip_portable_payload(database: &Path) -> Result<(), StorageError> {
    let connection = Connection::open(database).map_err(|_| StorageError::restore())?;
    strip_portable_payload_from_connection(&connection)?;
    Ok(())
}

/// Removes backup-only tables through an already open live restore connection.
pub(super) fn strip_portable_payload_from_connection(
    connection: &Connection,
) -> Result<(), StorageError> {
    connection
        .execute_batch(&format!(
            "DROP TABLE IF EXISTS {MANIFEST_TABLE};
             DROP TABLE IF EXISTS {ATTACHMENT_TABLE};
             DROP TABLE IF EXISTS {DERIVATIVE_TABLE};"
        ))
        .map_err(|_| StorageError::restore())?;
    Ok(())
}

/// Loads every retained source identity and exact byte size from the copied schema.
fn attachment_rows(connection: &Connection) -> Result<Vec<(String, i64)>, StorageError> {
    let mut statement = connection
        .prepare("SELECT sha256, byte_size FROM attachments ORDER BY sha256")
        .map_err(|_| StorageError::invalid_backup())?;
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|_| StorageError::invalid_backup())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| StorageError::invalid_backup())
}

/// Loads every ready normalized derivative identity, format, and byte size.
fn derivative_rows(connection: &Connection) -> Result<Vec<(String, String, i64)>, StorageError> {
    let mut statement = connection
        .prepare(
            "SELECT DISTINCT normalized_sha256, format, byte_size
             FROM attachment_image_normalizations
             WHERE state = 'ready'
             ORDER BY normalized_sha256",
        )
        .map_err(|_| StorageError::invalid_backup())?;
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(|_| StorageError::invalid_backup())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| StorageError::invalid_backup())
}

/// Validates embedded bytes against exact catalog rows and rejects extra payload content.
fn validate_embedded_rows(
    connection: &Connection,
    table: &str,
    expected: &[(String, i64)],
) -> Result<(), StorageError> {
    let count: i64 = connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .map_err(|_| StorageError::invalid_backup())?;
    if count != expected.len() as i64 {
        return Err(StorageError::invalid_backup());
    }
    for (sha256, byte_size) in expected {
        let bytes: Vec<u8> = connection
            .query_row(
                &format!("SELECT content FROM {table} WHERE sha256 = ?1 AND byte_size = ?2"),
                params![sha256, byte_size],
                |row| row.get(0),
            )
            .map_err(|_| StorageError::invalid_backup())?;
        verify_bytes(&bytes, sha256, *byte_size)?;
    }
    Ok(())
}

/// Validates ready derivative format as well as its exact bytes, size, and identity.
fn validate_embedded_derivatives(
    connection: &Connection,
    expected: &[(String, String, i64)],
) -> Result<(), StorageError> {
    let count: i64 = connection
        .query_row(
            &format!("SELECT COUNT(*) FROM {DERIVATIVE_TABLE}"),
            [],
            |row| row.get(0),
        )
        .map_err(|_| StorageError::invalid_backup())?;
    if count != expected.len() as i64 {
        return Err(StorageError::invalid_backup());
    }
    for (sha256, format, byte_size) in expected {
        let bytes: Vec<u8> = connection
            .query_row(
                &format!(
                    "SELECT content FROM {DERIVATIVE_TABLE}
                     WHERE sha256 = ?1 AND format = ?2 AND byte_size = ?3"
                ),
                params![sha256, format, byte_size],
                |row| row.get(0),
            )
            .map_err(|_| StorageError::invalid_backup())?;
        verify_bytes(&bytes, sha256, *byte_size)?;
    }
    Ok(())
}

/// Extracts one trusted portable table to deterministic hash-sharded native paths.
fn extract_table(
    connection: &Connection,
    table: &str,
    root: &Path,
    directory: &str,
    has_format: bool,
) -> Result<(), StorageError> {
    let query = if has_format {
        format!("SELECT sha256, format, byte_size, content FROM {table} ORDER BY sha256")
    } else {
        format!("SELECT sha256, NULL, byte_size, content FROM {table} ORDER BY sha256")
    };
    let mut statement = connection
        .prepare(&query)
        .map_err(|_| StorageError::invalid_backup())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Vec<u8>>(3)?,
            ))
        })
        .map_err(|_| StorageError::invalid_backup())?;
    for row in rows {
        let (sha256, format, byte_size, bytes) = row.map_err(|_| StorageError::invalid_backup())?;
        verify_bytes(&bytes, &sha256, byte_size)?;
        let path = content_path(root, directory, &sha256, format.as_deref())?;
        let parent = path.parent().ok_or_else(StorageError::restore)?;
        fs::create_dir_all(parent).map_err(|_| StorageError::restore())?;
        fs::write(path, bytes).map_err(|_| StorageError::restore())?;
    }
    Ok(())
}

/// Reads and verifies one bounded file before embedding it into a backup.
fn verified_file(path: &Path, sha256: &str, byte_size: i64) -> Result<Vec<u8>, StorageError> {
    let bytes = fs::read(path).map_err(|_| StorageError::backup())?;
    verify_bytes(&bytes, sha256, byte_size).map_err(|_| StorageError::backup())?;
    Ok(bytes)
}

/// Verifies exact byte length and lowercase SHA-256 content identity.
fn verify_bytes(bytes: &[u8], sha256: &str, byte_size: i64) -> Result<(), StorageError> {
    if byte_size < 0
        || bytes.len() as i64 != byte_size
        || format!("{:x}", Sha256::digest(bytes)) != sha256
    {
        return Err(StorageError::invalid_backup());
    }
    Ok(())
}

/// Builds one validated hash-sharded source or normalized derivative path.
fn content_path(
    root: &Path,
    directory: &str,
    sha256: &str,
    format: Option<&str>,
) -> Result<std::path::PathBuf, StorageError> {
    if sha256.len() != 64
        || !sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        || format.is_some_and(|value| !matches!(value, "jpeg" | "png"))
    {
        return Err(StorageError::invalid_backup());
    }
    let file_name = format.map_or_else(|| sha256.to_owned(), |value| format!("{sha256}.{value}"));
    Ok(root.join(directory).join(&sha256[..2]).join(file_name))
}

/// Checks one exact SQLite table name without accepting views or similarly named objects.
fn table_exists(connection: &Connection, table: &str) -> Result<bool, StorageError> {
    connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )
        .map_err(|_| StorageError::invalid_backup())
}
