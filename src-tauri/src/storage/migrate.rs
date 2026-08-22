//! Transactional orchestration for ordered conversation-store migrations.

use rusqlite::{Connection, params};

use super::{
    CURRENT_SCHEMA_VERSION, ConversationStore, DEFAULT_PROFILE_ID, DEFAULT_PROFILE_NAME,
    StorageError,
    memory_chunks::backfill_memory_chunks,
    memory_chunks_migration::MIGRATION_17,
    memory_exclusion_migration::MIGRATION_19,
    memory_lexical_migration::MIGRATION_16,
    memory_semantic_migration::MIGRATION_18,
    migrations::{
        MIGRATION_1, MIGRATION_2, MIGRATION_3, MIGRATION_4, MIGRATION_5, MIGRATION_6, MIGRATION_7,
        MIGRATION_8, MIGRATION_9, MIGRATION_10, MIGRATION_11, MIGRATION_12, MIGRATION_13,
        MIGRATION_14, MIGRATION_15,
    },
    now_ms,
    retention_migration::MIGRATION_20,
    tool_audit_migration::MIGRATION_21,
};

impl ConversationStore {
    /// Applies each pending migration exactly once and ensures the built-in profile exists.
    pub(super) fn migrate(&self, connection: &mut Connection) -> Result<(), StorageError> {
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > CURRENT_SCHEMA_VERSION {
            return Err(StorageError::internal());
        }
        if version < 1 {
            let transaction =
                connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            transaction.execute_batch(MIGRATION_1)?;
            let now = now_ms()?;
            transaction.execute(
                "INSERT INTO schema_migrations (version, name, applied_at_ms) VALUES (1, 'storage foundation', ?1)",
                [now],
            )?;
            transaction.execute(
                "INSERT INTO profiles (id, name, created_at_ms) VALUES (?1, ?2, ?3)",
                params![DEFAULT_PROFILE_ID, DEFAULT_PROFILE_NAME, now],
            )?;
            transaction.pragma_update(None, "user_version", 1)?;
            transaction.commit()?;
        }
        if version < 2 {
            apply_migration(connection, MIGRATION_2, 2, "branch-local message order")?;
        }
        if version < 3 {
            apply_migration(connection, MIGRATION_3, 3, "provider runs and usage")?;
        }
        if version < 4 {
            apply_migration(connection, MIGRATION_4, 4, "last open conversation")?;
        }
        if version < 5 {
            apply_migration(connection, MIGRATION_5, 5, "selected conversation branch")?;
        }
        if version < 6 {
            apply_migration(connection, MIGRATION_6, 6, "assistant response ratings")?;
        }
        if version < 7 {
            apply_migration(connection, MIGRATION_7, 7, "tool invocations and results")?;
        }
        if version < 8 {
            apply_migration(connection, MIGRATION_8, 8, "content-addressed attachments")?;
        }
        if version < 9 {
            apply_migration(connection, MIGRATION_9, 9, "durable message attachments")?;
        }
        if version < 10 {
            apply_migration(connection, MIGRATION_10, 10, "attachment text extraction")?;
        }
        if version < 11 {
            apply_migration(connection, MIGRATION_11, 11, "bounded PDF text extraction")?;
        }
        if version < 12 {
            apply_migration(connection, MIGRATION_12, 12, "bounded DOCX text extraction")?;
        }
        if version < 13 {
            apply_migration(connection, MIGRATION_13, 13, "bounded image normalization")?;
        }
        if version < 14 {
            apply_migration(
                connection,
                MIGRATION_14,
                14,
                "attachment text indexing readiness",
            )?;
        }
        if version < 15 {
            apply_migration(
                connection,
                MIGRATION_15,
                15,
                "conversation attachment scope",
            )?;
        }
        if version < 16 {
            apply_migration(
                connection,
                MIGRATION_16,
                16,
                "FTS5 lexical memory foundation",
            )?;
        }
        if version < 17 {
            apply_memory_chunk_migration(connection)?;
        }
        if version < 18 {
            apply_migration(
                connection,
                MIGRATION_18,
                18,
                "resumable sqlite-vec semantic index",
            )?;
        }
        if version < 19 {
            apply_migration(
                connection,
                MIGRATION_19,
                19,
                "per-conversation memory exclusion",
            )?;
        }
        if version < 20 {
            apply_migration(connection, MIGRATION_20, 20, "time-based Trash retention")?;
        }
        if version < 21 {
            apply_migration(
                connection,
                MIGRATION_21,
                21,
                "structured tool execution audit",
            )?;
        }
        Ok(())
    }
}

/// Creates and backfills the Rust-derived chunk catalog in one immediate transaction.
fn apply_memory_chunk_migration(connection: &mut Connection) -> Result<(), StorageError> {
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    transaction.execute_batch(MIGRATION_17)?;
    backfill_memory_chunks(&transaction)?;
    transaction.execute(
        "INSERT INTO schema_migrations (version, name, applied_at_ms) VALUES (17, ?1, ?2)",
        params!["versioned deterministic memory chunks", now_ms()?],
    )?;
    transaction.pragma_update(None, "user_version", 17)?;
    transaction.commit()?;
    Ok(())
}

/// Applies and records one migration inside its own immediate transaction.
fn apply_migration(
    connection: &mut Connection,
    sql: &str,
    version: i64,
    name: &str,
) -> Result<(), StorageError> {
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    transaction.execute_batch(sql)?;
    transaction.execute(
        "INSERT INTO schema_migrations (version, name, applied_at_ms) VALUES (?1, ?2, ?3)",
        params![version, name, now_ms()?],
    )?;
    transaction.pragma_update(None, "user_version", version)?;
    transaction.commit()?;
    Ok(())
}
