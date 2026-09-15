//! Exact durable request reconstruction for failed and cancelled image retries.

use rusqlite::{Connection, OptionalExtension, params};

use super::{
    GeneratedAssetStatus, GeneratedImageProvenance, GeneratedImageRequestOptions,
    StartedGeneratedImage, insert_generated_image_message, load_generated_message,
    load_message_generated_assets, selected_branch_without_active_generation,
    validate_output_count,
};
use crate::storage::{ConversationStore, DEFAULT_PROFILE_ID, StorageError};

/// Native-only request reconstructed from one exact terminal image message.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GeneratedImageRetry {
    /// Conversation owning the terminal request.
    pub(super) conversation_id: String,
    /// Selected branch that must still own the terminal request.
    pub(super) branch_id: String,
    /// Durable user message whose text is the exact generation prompt.
    pub(super) request_message_id: String,
    /// Exact durable user prompt retained behind the native boundary.
    pub(crate) prompt: String,
    /// Exact output count retained by the terminal request.
    pub(crate) output_count: u8,
    /// Exact model, backend, and seed retained by every terminal output.
    pub(crate) provenance: GeneratedImageProvenance,
    /// Exact dimensions and prompt-extension policy retained by the request.
    pub(crate) options: GeneratedImageRequestOptions,
}

impl ConversationStore {
    /// Reconstructs one selected terminal request without appending a new pending message.
    pub(crate) fn inspect_generated_image_retry(
        &self,
        message_id: &str,
    ) -> Result<GeneratedImageRetry, StorageError> {
        load_retry_request(&self.open()?, message_id)
    }

    /// Starts a fresh run from one selected failed or cancelled durable image request.
    pub(crate) fn retry_generated_image_message(
        &self,
        message_id: &str,
    ) -> Result<StartedGeneratedImage, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let retry = load_retry_request(&transaction, message_id)?;
        let branch_id =
            selected_branch_without_active_generation(&transaction, &retry.conversation_id)?;
        if branch_id != retry.branch_id {
            return Err(StorageError::invalid(
                "Select that image response branch before retrying it.",
            ));
        }
        let new_message_id = insert_generated_image_message(
            &transaction,
            &retry.conversation_id,
            &branch_id,
            &retry.request_message_id,
            retry.output_count,
            &retry.provenance,
            &retry.options,
        )?;
        let message =
            load_generated_message(&transaction, &retry.conversation_id, &new_message_id)?;
        transaction.commit()?;
        Ok(StartedGeneratedImage {
            message,
            output_count: retry.output_count,
            prompt: retry.prompt,
            options: retry.options,
            provenance: retry.provenance,
        })
    }
}

/// Loads one selected terminal request while rejecting incomplete or mixed provenance.
fn load_retry_request(
    connection: &Connection,
    message_id: &str,
) -> Result<GeneratedImageRetry, StorageError> {
    let row = load_retry_row(connection, message_id)?;
    let assets = load_message_generated_assets(connection, message_id)?;
    let first = assets
        .first()
        .ok_or_else(|| StorageError::invalid("That image response cannot be retried."))?;
    if assets.iter().any(|asset| {
        !matches!(
            asset.status,
            GeneratedAssetStatus::Failed | GeneratedAssetStatus::Cancelled
        ) || asset.provider_id != first.provider_id
            || asset.model_id != first.model_id
            || asset.execution != first.execution
            || asset.seed != first.seed
    }) {
        return Err(StorageError::invalid(
            "That image response cannot be retried.",
        ));
    }
    let output_count = u8::try_from(assets.len()).map_err(|_| StorageError::internal())?;
    validate_output_count(output_count)?;
    Ok(GeneratedImageRetry {
        conversation_id: row.0,
        branch_id: row.1,
        request_message_id: row.2,
        prompt: row.3,
        output_count,
        provenance: GeneratedImageProvenance::new(
            first.provider_id.clone(),
            first.model_id.clone(),
            first.execution,
            first.seed,
        )?,
        options: GeneratedImageRequestOptions::new(row.4, row.5, row.6)?,
    })
}

/// Loads the selected terminal message and its exact durable prompt/options row.
fn load_retry_row(
    connection: &Connection,
    message_id: &str,
) -> Result<(String, String, String, String, u32, u32, bool), StorageError> {
    connection
        .query_row(
            "SELECT messages.conversation_id, messages.branch_id,
                    generated_image_requests.request_message_id, message_blocks.text_content,
                    generated_image_requests.width, generated_image_requests.height,
                    generated_image_requests.prompt_extend
             FROM messages
             JOIN conversations ON conversations.id = messages.conversation_id
             JOIN generated_image_requests ON generated_image_requests.message_id = messages.id
             JOIN message_blocks
               ON message_blocks.message_id = generated_image_requests.request_message_id
              AND message_blocks.ordinal = 0 AND message_blocks.block_type = 'text'
             WHERE messages.id = ?1 AND messages.role = 'assistant'
               AND messages.state IN ('failed', 'cancelled')
               AND messages.branch_id = conversations.current_branch_id
               AND conversations.profile_id = ?2 AND conversations.deleted_at_ms IS NULL
               AND NOT EXISTS (
                   SELECT 1 FROM messages AS later
                   WHERE later.branch_id = messages.branch_id AND later.sequence > messages.sequence
               )",
            params![message_id, DEFAULT_PROFILE_ID],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StorageError::invalid("That image response cannot be retried."))
}
