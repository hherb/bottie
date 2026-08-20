//! Native-owned conversation branch creation and selection.

use rusqlite::{OptionalExtension, Transaction, params};

use super::{
    ConversationStore, DEFAULT_PROFILE_ID, ForkedConversation, StorageError, StoredConversation,
    load_conversation_from_connection, now_ms,
};

const BRANCH_NAME_PREFIX: &str = "Alternative";

impl ConversationStore {
    /// Forks one visible final user message, selects the new branch, and returns its request identity.
    pub(crate) fn fork_from_user_message(
        &self,
        conversation_id: &str,
        message_id: &str,
        text: &str,
    ) -> Result<ForkedConversation, StorageError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(StorageError::invalid("An edited message cannot be empty."));
        }
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        ensure_no_active_run(&transaction, conversation_id)?;
        let current_branch_id = selected_branch_id(&transaction, conversation_id)?;
        if !message_is_in_lineage(&transaction, &current_branch_id, message_id)? {
            return Err(invalid_branch_target());
        }
        let (parent_message_id, role, state): (Option<String>, String, String) = transaction
            .query_row(
                "SELECT parent_message_id, role, state FROM messages
                 WHERE id = ?1 AND conversation_id = ?2",
                params![message_id, conversation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or_else(invalid_branch_target)?;
        if role != "user" || state != "final" {
            return Err(invalid_branch_target());
        }

        let branch_number: i64 = transaction.query_row(
            "SELECT COUNT(*) + 1 FROM branches WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )?;
        let branch_id = uuid::Uuid::new_v4().to_string();
        let request_message_id = uuid::Uuid::new_v4().to_string();
        let created_at_ms = now_ms()?;
        transaction.execute(
            "INSERT INTO branches (id, conversation_id, name, created_at_ms) VALUES (?1, ?2, ?3, ?4)",
            params![
                &branch_id,
                conversation_id,
                format!("{BRANCH_NAME_PREFIX} {branch_number}"),
                created_at_ms
            ],
        )?;
        transaction.execute(
            "INSERT INTO messages
             (id, conversation_id, branch_id, parent_message_id, role, state, provider_id, model_id,
              created_at_ms, sequence, provider_run_id)
             VALUES (?1, ?2, ?3, ?4, 'user', 'final', NULL, NULL, ?5, 0, NULL)",
            params![
                &request_message_id,
                conversation_id,
                &branch_id,
                parent_message_id,
                created_at_ms
            ],
        )?;
        transaction.execute(
            "INSERT INTO message_blocks (id, message_id, ordinal, block_type, text_content)
             VALUES (?1, ?2, 0, 'text', ?3)",
            params![uuid::Uuid::new_v4().to_string(), &request_message_id, text],
        )?;
        transaction.execute(
            "INSERT INTO message_attachments (message_id, attachment_id, ordinal, attached_at_ms)
             SELECT ?1, attachment_id, ordinal, ?2 FROM message_attachments WHERE message_id = ?3",
            params![&request_message_id, created_at_ms, message_id],
        )?;
        transaction.execute(
            "UPDATE conversations
             SET current_branch_id = ?1, updated_at_ms = ?2, archived_at_ms = NULL
             WHERE id = ?3",
            params![&branch_id, created_at_ms, conversation_id],
        )?;
        let conversation = load_conversation_from_connection(&transaction, conversation_id)?;
        transaction.commit()?;
        Ok(ForkedConversation {
            conversation,
            request_message_id,
        })
    }

    /// Selects one existing branch after ensuring no provider response is active.
    pub(crate) fn select_branch(
        &self,
        conversation_id: &str,
        branch_id: &str,
    ) -> Result<StoredConversation, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        ensure_no_active_run(&transaction, conversation_id)?;
        let changed = transaction.execute(
            "UPDATE conversations SET current_branch_id = ?1
             WHERE id = ?2 AND profile_id = ?3 AND deleted_at_ms IS NULL
               AND EXISTS (
                   SELECT 1 FROM branches WHERE branches.id = ?1
                     AND branches.conversation_id = conversations.id
               )",
            params![branch_id, conversation_id, DEFAULT_PROFILE_ID],
        )?;
        if changed == 0 {
            return Err(StorageError::not_found(
                "That conversation branch is unavailable.",
            ));
        }
        let conversation = load_conversation_from_connection(&transaction, conversation_id)?;
        transaction.commit()?;
        Ok(conversation)
    }
}

/// Loads the selected branch for one editable local-profile conversation.
fn selected_branch_id(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<String, StorageError> {
    transaction
        .query_row(
            "SELECT current_branch_id FROM conversations
             WHERE id = ?1 AND profile_id = ?2 AND deleted_at_ms IS NULL",
            params![conversation_id, DEFAULT_PROFILE_ID],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| StorageError::not_found("That conversation no longer exists."))
}

/// Rejects branch changes while native generation owns an unfinished response.
fn ensure_no_active_run(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<(), StorageError> {
    let has_active_run: bool = transaction.query_row(
        "SELECT EXISTS (
             SELECT 1 FROM provider_runs WHERE conversation_id = ?1 AND state = 'running'
         )",
        [conversation_id],
        |row| row.get(0),
    )?;
    if has_active_run {
        return Err(StorageError::invalid(
            "Wait for the active response to finish before changing branches.",
        ));
    }
    Ok(())
}

/// Reports whether a message is visible by walking the selected branch leaf's parent chain.
fn message_is_in_lineage(
    transaction: &Transaction<'_>,
    branch_id: &str,
    message_id: &str,
) -> Result<bool, StorageError> {
    transaction
        .query_row(
            "WITH RECURSIVE lineage(id, parent_message_id) AS (
                 SELECT id, parent_message_id FROM messages
                 WHERE id = (
                     SELECT id FROM messages WHERE branch_id = ?1 ORDER BY sequence DESC LIMIT 1
                 )
                 UNION ALL
                 SELECT messages.id, messages.parent_message_id
                 FROM messages JOIN lineage ON messages.id = lineage.parent_message_id
             )
             SELECT EXISTS (SELECT 1 FROM lineage WHERE id = ?2)",
            params![branch_id, message_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

/// Creates the stable validation error for a non-visible or non-user fork target.
fn invalid_branch_target() -> StorageError {
    StorageError::invalid(
        "Only a user message on the selected branch can be edited or regenerated.",
    )
}
