//! Restart recovery for generated-image work left pending by an earlier process.

use rusqlite::params;

use super::*;

impl ConversationStore {
    /// Converts pending image work left by an earlier process into a stable retryable failure.
    pub(crate) fn recover_interrupted_generated_images(&self) -> Result<usize, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let message_ids = {
            let mut statement = transaction.prepare(
                "SELECT DISTINCT messages.id FROM messages
                 JOIN generated_assets ON generated_assets.message_id = messages.id
                 WHERE messages.role = 'assistant' AND messages.state = 'partial'
                   AND generated_assets.status = 'pending' ORDER BY messages.id",
            )?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        let recovered_at_ms = now_ms()?;
        for message_id in &message_ids {
            transaction.execute(
                "UPDATE generated_assets
                 SET status = 'failed', error_code = 'interrupted', updated_at_ms = ?1
                 WHERE message_id = ?2 AND status = 'pending'",
                params![recovered_at_ms, message_id],
            )?;
            transaction.execute(
                "UPDATE messages SET state = 'failed' WHERE id = ?1 AND state = 'partial'",
                [message_id],
            )?;
            transaction.execute(
                "UPDATE message_blocks SET text_content = 'Image generation interrupted.'
                 WHERE message_id = ?1 AND ordinal = 0 AND block_type = 'text'",
                [message_id],
            )?;
            memory_chunks::refresh_message_chunks(&transaction, message_id)?;
        }
        transaction.commit()?;
        Ok(message_ids.len())
    }
}
