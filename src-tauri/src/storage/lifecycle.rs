//! Recoverable conversation lifecycle operations.

use rusqlite::{Connection, OptionalExtension, params};

use super::{
    ConversationLifecycle, ConversationStore, ConversationSummary, DEFAULT_PROFILE_ID,
    StorageError, normalized_title, now_ms,
};

impl ConversationStore {
    /// Returns active, archived, and deleted conversations for lifecycle navigation.
    pub(crate) fn list_conversations(&self) -> Result<Vec<ConversationSummary>, StorageError> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT id, title, updated_at_ms,
                    CASE
                        WHEN deleted_at_ms IS NOT NULL THEN 'deleted'
                        WHEN archived_at_ms IS NOT NULL THEN 'archived'
                        ELSE 'active'
                    END AS lifecycle,
                    EXISTS (
                        SELECT 1 FROM conversation_memory_preferences
                        WHERE conversation_id = conversations.id AND excluded = 1
                    ) AS memory_excluded
             FROM conversations WHERE profile_id = ?1
             ORDER BY CASE lifecycle WHEN 'active' THEN 0 WHEN 'archived' THEN 1 ELSE 2 END,
                      COALESCE(deleted_at_ms, archived_at_ms, updated_at_ms) DESC, id DESC",
        )?;
        let rows = statement.query_map([DEFAULT_PROFILE_ID], summary_from_row)?;
        rows.collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(summary_from_database)
            .collect()
    }

    /// Renames one active or archived conversation after normalizing its title.
    pub(crate) fn rename_conversation(
        &self,
        conversation_id: &str,
        title: &str,
    ) -> Result<ConversationSummary, StorageError> {
        let title = normalized_title(title)?;
        let connection = self.open()?;
        let changed = connection.execute(
            "UPDATE conversations SET title = ?1
             WHERE id = ?2 AND profile_id = ?3 AND deleted_at_ms IS NULL",
            params![title, conversation_id, DEFAULT_PROFILE_ID],
        )?;
        require_change(changed)?;
        load_summary(&connection, conversation_id)
    }

    /// Moves one non-deleted conversation into or out of the archive.
    pub(crate) fn set_conversation_archived(
        &self,
        conversation_id: &str,
        archived: bool,
    ) -> Result<ConversationSummary, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let archived_at_ms = archived.then(now_ms).transpose()?;
        let changed = transaction.execute(
            "UPDATE conversations SET archived_at_ms = ?1
             WHERE id = ?2 AND profile_id = ?3 AND deleted_at_ms IS NULL",
            params![archived_at_ms, conversation_id, DEFAULT_PROFILE_ID],
        )?;
        require_change(changed)?;
        if archived {
            clear_selected_conversation(&transaction, conversation_id)?;
        }
        let summary = load_summary(&transaction, conversation_id)?;
        transaction.commit()?;
        Ok(summary)
    }

    /// Excludes or restores one active or archived conversation in every native memory path.
    pub(crate) fn set_conversation_memory_excluded(
        &self,
        conversation_id: &str,
        excluded: bool,
    ) -> Result<ConversationSummary, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let available = transaction.query_row(
            "SELECT EXISTS (
                 SELECT 1 FROM conversations
                 WHERE id = ?1 AND profile_id = ?2 AND deleted_at_ms IS NULL
             )",
            params![conversation_id, DEFAULT_PROFILE_ID],
            |row| row.get::<_, bool>(0),
        )?;
        if !available {
            return Err(missing_conversation());
        }
        let has_active_run = transaction.query_row(
            "SELECT EXISTS (
                 SELECT 1 FROM provider_runs WHERE conversation_id = ?1 AND state = 'running'
             )",
            [conversation_id],
            |row| row.get::<_, bool>(0),
        )?;
        if has_active_run {
            return Err(StorageError::invalid(
                "Wait for the active response to finish before changing memory access.",
            ));
        }
        if excluded {
            transaction.execute(
                "INSERT INTO conversation_memory_preferences (conversation_id, excluded, updated_at_ms)
                 VALUES (?1, 1, ?2)
                 ON CONFLICT(conversation_id) DO UPDATE
                 SET excluded = 1, updated_at_ms = ?2",
                params![conversation_id, now_ms()?],
            )?;
        } else {
            transaction.execute(
                "DELETE FROM conversation_memory_preferences WHERE conversation_id = ?1",
                [conversation_id],
            )?;
        }
        let message_ids = source_message_ids(&transaction, conversation_id)?;
        for message_id in message_ids {
            super::memory_chunks::refresh_message_chunks(&transaction, &message_id)?;
        }
        let summary = load_summary(&transaction, conversation_id)?;
        transaction.commit()?;
        Ok(summary)
    }

    /// Soft-deletes one conversation while preserving all of its durable content.
    pub(crate) fn delete_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<ConversationSummary, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE conversations SET deleted_at_ms = ?1
             WHERE id = ?2 AND profile_id = ?3 AND deleted_at_ms IS NULL",
            params![now_ms()?, conversation_id, DEFAULT_PROFILE_ID],
        )?;
        require_change(changed)?;
        clear_selected_conversation(&transaction, conversation_id)?;
        let summary = load_summary(&transaction, conversation_id)?;
        transaction.commit()?;
        Ok(summary)
    }

    /// Restores one soft-deleted conversation to the active recent list.
    pub(crate) fn restore_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<ConversationSummary, StorageError> {
        let connection = self.open()?;
        let changed = connection.execute(
            "UPDATE conversations SET deleted_at_ms = NULL, archived_at_ms = NULL
             WHERE id = ?1 AND profile_id = ?2 AND deleted_at_ms IS NOT NULL",
            params![conversation_id, DEFAULT_PROFILE_ID],
        )?;
        require_change(changed)?;
        load_summary(&connection, conversation_id)
    }

    /// Permanently deletes one trashed conversation and all conversation-owned source records.
    pub(crate) fn forget_conversation(&self, conversation_id: &str) -> Result<(), StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let deleted = transaction
            .query_row(
                "SELECT deleted_at_ms IS NOT NULL FROM conversations
                 WHERE id = ?1 AND profile_id = ?2",
                params![conversation_id, DEFAULT_PROFILE_ID],
                |row| row.get::<_, bool>(0),
            )
            .optional()?
            .ok_or_else(missing_conversation)?;
        if !deleted {
            return Err(StorageError::invalid(
                "Move the conversation to Trash before forgetting it permanently.",
            ));
        }
        let has_active_run = transaction.query_row(
            "SELECT EXISTS (
                 SELECT 1 FROM provider_runs WHERE conversation_id = ?1 AND state = 'running'
             )",
            [conversation_id],
            |row| row.get::<_, bool>(0),
        )?;
        if has_active_run {
            return Err(StorageError::invalid(
                "Wait for the active response to finish before forgetting this conversation.",
            ));
        }
        let changed = transaction.execute(
            "DELETE FROM conversations
             WHERE id = ?1 AND profile_id = ?2 AND deleted_at_ms IS NOT NULL",
            params![conversation_id, DEFAULT_PROFILE_ID],
        )?;
        require_change(changed)?;
        transaction.commit()?;
        Ok(())
    }
}

/// Raw summary tuple decoded before mapping the lifecycle string.
type RawSummary = (String, String, i64, String, bool);

/// Decodes a summary row without leaking application errors through rusqlite callbacks.
fn summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawSummary> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
    ))
}

/// Maps one trusted database tuple into the serialized navigation contract.
fn summary_from_database(raw: RawSummary) -> Result<ConversationSummary, StorageError> {
    Ok(ConversationSummary {
        id: raw.0,
        title: raw.1,
        updated_at_ms: raw.2,
        lifecycle: ConversationLifecycle::from_database(&raw.3)?,
        memory_excluded: raw.4,
    })
}

/// Loads one lifecycle summary belonging to the built-in local profile.
fn load_summary(
    connection: &Connection,
    conversation_id: &str,
) -> Result<ConversationSummary, StorageError> {
    let raw = connection
        .query_row(
            "SELECT id, title, updated_at_ms,
                    CASE
                        WHEN deleted_at_ms IS NOT NULL THEN 'deleted'
                        WHEN archived_at_ms IS NOT NULL THEN 'archived'
                        ELSE 'active'
                    END,
                    EXISTS (
                        SELECT 1 FROM conversation_memory_preferences
                        WHERE conversation_id = conversations.id AND excluded = 1
                    )
             FROM conversations WHERE id = ?1 AND profile_id = ?2",
            params![conversation_id, DEFAULT_PROFILE_ID],
            summary_from_row,
        )
        .optional()?
        .ok_or_else(missing_conversation)?;
    summary_from_database(raw)
}

/// Loads stable source identities before exclusion refresh mutates the chunk catalog.
fn source_message_ids(
    connection: &Connection,
    conversation_id: &str,
) -> Result<Vec<String>, StorageError> {
    let mut statement = connection
        .prepare("SELECT id FROM messages WHERE conversation_id = ?1 ORDER BY sequence, id")?;
    statement
        .query_map([conversation_id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

/// Converts a guarded update with no matching row into a stable missing-record error.
fn require_change(changed: usize) -> Result<(), StorageError> {
    if changed == 0 {
        return Err(missing_conversation());
    }
    Ok(())
}

/// Creates the shared lifecycle missing-record error.
fn missing_conversation() -> StorageError {
    StorageError::not_found("That conversation is unavailable for this action.")
}

/// Clears the profile selection only when it points at the lifecycle target.
fn clear_selected_conversation(
    connection: &Connection,
    conversation_id: &str,
) -> Result<(), StorageError> {
    connection.execute(
        "UPDATE profiles SET last_open_conversation_id = NULL
         WHERE id = ?1 AND last_open_conversation_id = ?2",
        params![DEFAULT_PROFILE_ID, conversation_id],
    )?;
    Ok(())
}
