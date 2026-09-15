//! Shared selected-request gates for generated-image creation and editing.

use rusqlite::{OptionalExtension, Transaction, params};

use crate::storage::{DEFAULT_PROFILE_ID, StorageError};

/// Loads one exact final user prompt that is still the selected branch leaf.
pub(in crate::storage) fn selected_image_prompt(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    request_message_id: &str,
    branch_id: &str,
) -> Result<String, StorageError> {
    transaction
        .query_row(
            "SELECT message_blocks.text_content FROM messages
             JOIN message_blocks ON message_blocks.message_id = messages.id
                AND message_blocks.ordinal = 0 AND message_blocks.block_type = 'text'
             WHERE messages.id = ?1 AND messages.conversation_id = ?2
               AND messages.branch_id = ?3 AND messages.role = 'user' AND messages.state = 'final'
               AND NOT EXISTS (
                   SELECT 1 FROM messages AS later
                   WHERE later.branch_id = messages.branch_id AND later.sequence > messages.sequence
               )",
            params![request_message_id, conversation_id, branch_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| {
            StorageError::invalid("That image prompt is no longer the selected request.")
        })
}

/// Selects an active conversation branch with no text or image generation already pending.
pub(in crate::storage) fn selected_branch_without_active_generation(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<String, StorageError> {
    let branch_id = transaction
        .query_row(
            "SELECT current_branch_id FROM conversations
             WHERE id = ?1 AND profile_id = ?2 AND deleted_at_ms IS NULL",
            params![conversation_id, DEFAULT_PROFILE_ID],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| StorageError::not_found("That conversation no longer exists."))?;
    let active: bool = transaction.query_row(
        "SELECT EXISTS (
             SELECT 1 FROM provider_runs WHERE conversation_id = ?1 AND state = 'running'
             UNION ALL
             SELECT 1 FROM generated_assets JOIN messages ON messages.id = generated_assets.message_id
             WHERE messages.conversation_id = ?1 AND generated_assets.status = 'pending'
         )",
        [conversation_id],
        |row| row.get(0),
    )?;
    if active {
        return Err(StorageError::invalid(
            "Wait for the active response to finish before generating an image.",
        ));
    }
    Ok(branch_id)
}
