//! Read-only schema, ledger, semantic-contract, and source-identity migration validation.

use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use super::{DEFAULT_PROFILE_ID, StorageError, memory_semantic, migrate::migration_name};

const REQUIRED_TABLES: &[&str] = &[
    "schema_migrations",
    "profiles",
    "conversations",
    "branches",
    "messages",
    "message_blocks",
];
const SOURCE_IDENTITIES: &[(&str, &str)] = &[
    ("profiles", "hex(CAST(id AS BLOB))"),
    ("conversations", "hex(CAST(id AS BLOB))"),
    ("branches", "hex(CAST(id AS BLOB))"),
    ("messages", "hex(CAST(id AS BLOB))"),
    ("message_blocks", "hex(CAST(id AS BLOB))"),
    ("provider_runs", "hex(CAST(id AS BLOB))"),
    ("usage_records", "hex(CAST(id AS BLOB))"),
    ("response_ratings", "hex(CAST(message_id AS BLOB))"),
    ("tool_invocations", "hex(CAST(id AS BLOB))"),
    ("tool_results", "hex(CAST(id AS BLOB))"),
    ("attachments", "hex(CAST(id AS BLOB))"),
    (
        "message_attachments",
        "hex(CAST(message_id AS BLOB)) || ':' || hex(CAST(attachment_id AS BLOB))",
    ),
    ("attachment_extractions", "hex(CAST(attachment_id AS BLOB))"),
    (
        "attachment_image_normalizations",
        "hex(CAST(attachment_id AS BLOB))",
    ),
    (
        "attachment_text_indexing",
        "hex(CAST(attachment_id AS BLOB))",
    ),
    (
        "conversation_attachments",
        "hex(CAST(conversation_id AS BLOB)) || ':' || hex(CAST(attachment_id AS BLOB))",
    ),
    (
        "conversation_memory_preferences",
        "hex(CAST(conversation_id AS BLOB))",
    ),
    (
        "conversation_retention_policies",
        "hex(CAST(profile_id AS BLOB))",
    ),
];

/// Stable identities from every user-owned source table present before migration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SourceSnapshot(Vec<(String, Vec<String>)>);

/// Validates integrity, foreign keys, exact ledger, profile, and current semantic metadata.
pub(super) fn validate_database(
    path: &Path,
    version: i64,
    semantic: bool,
) -> Result<(), StorageError> {
    let connection = read_only_connection(path)?;
    validate_connection(&connection, version, semantic)
}

/// Applies the exact database validation contract to one already-open connection.
pub(super) fn validate_connection(
    connection: &Connection,
    expected_version: i64,
    semantic: bool,
) -> Result<(), StorageError> {
    let quick_check: String = connection
        .pragma_query_value(None, "quick_check", |row| row.get(0))
        .map_err(|_| StorageError::migration())?;
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| StorageError::migration())?;
    let foreign_key_failures: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .map_err(|_| StorageError::migration())?;
    if quick_check != "ok"
        || version != expected_version
        || foreign_key_failures != 0
        || version < 1
    {
        return Err(StorageError::migration());
    }
    for table in REQUIRED_TABLES {
        if !table_exists(connection, table)? {
            return Err(StorageError::migration());
        }
    }
    let profile_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM profiles WHERE id = ?1",
            [DEFAULT_PROFILE_ID],
            |row| row.get(0),
        )
        .map_err(|_| StorageError::migration())?;
    if profile_count != 1 || !ledger_is_exact(connection, version)? {
        return Err(StorageError::migration());
    }
    if semantic {
        memory_semantic::validate_semantic_contract(connection)
            .map_err(|_| StorageError::migration())?;
    }
    Ok(())
}

/// Requires one exact migration name for every contiguous version through the expected version.
fn ledger_is_exact(connection: &Connection, expected_version: i64) -> Result<bool, StorageError> {
    let mut statement = connection
        .prepare("SELECT version, name FROM schema_migrations ORDER BY version")
        .map_err(|_| StorageError::migration())?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| StorageError::migration())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| StorageError::migration())?;
    if rows.len() != expected_version as usize {
        return Ok(false);
    }
    for (index, (version, name)) in rows.iter().enumerate() {
        let expected = index as i64 + 1;
        if *version != expected || name != migration_name(expected)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Captures stable identities for every source table present before migration.
pub(super) fn source_snapshot(path: &Path) -> Result<SourceSnapshot, StorageError> {
    let connection = read_only_connection(path)?;
    let mut tables = Vec::new();
    for (table, identity) in SOURCE_IDENTITIES {
        if !table_exists(&connection, table)? {
            continue;
        }
        let sql = format!("SELECT {identity} FROM {table} ORDER BY 1");
        let mut statement = connection
            .prepare(&sql)
            .map_err(|_| StorageError::migration())?;
        let identities = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|_| StorageError::migration())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| StorageError::migration())?;
        tables.push(((*table).to_owned(), identities));
    }
    Ok(SourceSnapshot(tables))
}

/// Rejects a candidate or restored source that lost or invented source identities.
pub(super) fn validate_source_snapshot(
    path: &Path,
    expected: &SourceSnapshot,
) -> Result<(), StorageError> {
    let actual = source_snapshot(path)?;
    let retained = SourceSnapshot(
        actual
            .0
            .into_iter()
            .filter(|(table, _)| {
                expected
                    .0
                    .iter()
                    .any(|(expected_table, _)| expected_table == table)
            })
            .collect(),
    );
    if &retained == expected {
        Ok(())
    } else {
        Err(StorageError::migration())
    }
}

/// Opens a database without writable flags so preflight cannot create SQLite sidecars.
pub(super) fn read_only_connection(path: &Path) -> Result<Connection, StorageError> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| StorageError::migration())
}

/// Returns whether a version-zero SQLite file contains no schema objects.
pub(super) fn schema_is_empty(connection: &Connection) -> Result<bool, StorageError> {
    connection
        .query_row(
            "SELECT NOT EXISTS (SELECT 1 FROM sqlite_schema)",
            [],
            |row| row.get(0),
        )
        .map_err(|_| StorageError::migration())
}

/// Returns whether one required source table exists.
fn table_exists(connection: &Connection, table: &str) -> Result<bool, StorageError> {
    connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )
        .map_err(|_| StorageError::migration())
}
