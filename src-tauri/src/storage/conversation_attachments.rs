//! Ordered attachment context shared across every branch of one conversation.

use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::{
    ConversationStore, DEFAULT_PROFILE_ID, StorageError, StoredAttachment,
    attachments::{MAX_ATTACHMENT_SELECTION_COUNT, stored_attachment},
    now_ms,
};

impl ConversationStore {
    /// Adds retained attachments to conversation scope and returns the complete ordered scope.
    pub(crate) fn add_conversation_attachments(
        &self,
        conversation_id: &str,
        attachment_ids: &[String],
    ) -> Result<Vec<StoredAttachment>, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        ensure_conversation_context_mutable(&transaction, conversation_id)?;

        let existing = conversation_attachment_ids(&transaction, conversation_id)?;
        let existing_ids = existing.iter().cloned().collect::<HashSet<_>>();
        let mut submitted_ids = HashSet::new();
        let mut additions = Vec::new();
        for attachment_id in attachment_ids {
            if !submitted_ids.insert(attachment_id.as_str()) {
                continue;
            }
            stored_attachment(&transaction, attachment_id)?.ok_or_else(|| {
                StorageError::invalid("One or more selected attachments are unavailable.")
            })?;
            if !existing_ids.contains(attachment_id) {
                additions.push(attachment_id);
            }
        }
        if existing.len() + additions.len() > MAX_ATTACHMENT_SELECTION_COUNT {
            return Err(StorageError::invalid(format!(
                "Keep at most {MAX_ATTACHMENT_SELECTION_COUNT} files in conversation context."
            )));
        }

        let mut ordinal: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(ordinal), -1) + 1 FROM conversation_attachments
             WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )?;
        let attached_at_ms = now_ms()?;
        for attachment_id in additions {
            transaction.execute(
                "INSERT INTO conversation_attachments
                 (conversation_id, attachment_id, ordinal, attached_at_ms)
                 VALUES (?1, ?2, ?3, ?4)",
                params![conversation_id, attachment_id, ordinal, attached_at_ms],
            )?;
            ordinal += 1;
        }
        transaction.execute(
            "UPDATE conversations SET updated_at_ms = ?1 WHERE id = ?2",
            params![attached_at_ms, conversation_id],
        )?;
        let attachments = load_conversation_attachments(&transaction, conversation_id)?;
        transaction.commit()?;
        Ok(attachments)
    }

    /// Removes one conversation-scoped association while retaining catalog metadata and bytes.
    pub(crate) fn remove_conversation_attachment(
        &self,
        conversation_id: &str,
        attachment_id: &str,
    ) -> Result<Vec<StoredAttachment>, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        ensure_conversation_context_mutable(&transaction, conversation_id)?;
        let removed = transaction.execute(
            "DELETE FROM conversation_attachments WHERE conversation_id = ?1 AND attachment_id = ?2",
            params![conversation_id, attachment_id],
        )?;
        if removed == 0 {
            return Err(StorageError::not_found(
                "That conversation attachment is unavailable.",
            ));
        }
        transaction.execute(
            "UPDATE conversations SET updated_at_ms = ?1 WHERE id = ?2",
            params![now_ms()?, conversation_id],
        )?;
        let attachments = load_conversation_attachments(&transaction, conversation_id)?;
        transaction.commit()?;
        Ok(attachments)
    }
}

/// Loads ordered path-free metadata shared by every branch of one conversation.
pub(super) fn load_conversation_attachments(
    connection: &Connection,
    conversation_id: &str,
) -> Result<Vec<StoredAttachment>, StorageError> {
    conversation_attachment_ids(connection, conversation_id)?
        .into_iter()
        .map(|attachment_id| {
            stored_attachment(connection, &attachment_id)?.ok_or_else(StorageError::internal)
        })
        .collect()
}

/// Loads the bounded ordered native identities for one conversation scope.
fn conversation_attachment_ids(
    connection: &Connection,
    conversation_id: &str,
) -> Result<Vec<String>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT attachment_id FROM conversation_attachments
         WHERE conversation_id = ?1 ORDER BY ordinal",
    )?;
    statement
        .query_map([conversation_id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

/// Validates local ownership and prevents shared-context mutation during provider work.
fn ensure_conversation_context_mutable(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<(), StorageError> {
    let exists = transaction
        .query_row(
            "SELECT 1 FROM conversations
             WHERE id = ?1 AND profile_id = ?2 AND deleted_at_ms IS NULL",
            params![conversation_id, DEFAULT_PROFILE_ID],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_some();
    if !exists {
        return Err(StorageError::not_found(
            "That conversation no longer exists.",
        ));
    }
    let has_active_run: bool = transaction.query_row(
        "SELECT EXISTS (
             SELECT 1 FROM provider_runs WHERE conversation_id = ?1 AND state = 'running'
         )",
        [conversation_id],
        |row| row.get(0),
    )?;
    if has_active_run {
        return Err(StorageError::invalid(
            "Wait for the active response to finish before changing conversation context.",
        ));
    }
    Ok(())
}
