//! Transactional orchestration for ordered conversation-store migrations.

use rusqlite::{Connection, params};

use super::{
    CURRENT_SCHEMA_VERSION, ConversationStore, DEFAULT_PROFILE_ID, DEFAULT_PROFILE_NAME,
    StorageError,
    migrations::{
        MIGRATION_1, MIGRATION_2, MIGRATION_3, MIGRATION_4, MIGRATION_5, MIGRATION_6, MIGRATION_7,
        MIGRATION_8, MIGRATION_9, MIGRATION_10, MIGRATION_11,
    },
    now_ms,
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
        Ok(())
    }
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
