//! Durable local quality ratings for assistant responses.

use rusqlite::{OptionalExtension, params};

use super::{
    ConversationStore, DEFAULT_PROFILE_ID, ResponseRating, StorageError, StoredRole, now_ms,
};

impl ConversationStore {
    /// Sets or clears one rating after validating the exact local assistant response target.
    pub(crate) fn rate_response(
        &self,
        conversation_id: &str,
        message_id: &str,
        rating: Option<ResponseRating>,
    ) -> Result<Option<ResponseRating>, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let role = transaction
            .query_row(
                "WITH RECURSIVE lineage(id, parent_message_id) AS (
                     SELECT messages.id, messages.parent_message_id FROM messages
                     WHERE messages.id = (
                         SELECT selected.id FROM messages AS selected
                         JOIN conversations ON conversations.current_branch_id = selected.branch_id
                         WHERE conversations.id = ?2 AND conversations.profile_id = ?3
                           AND conversations.deleted_at_ms IS NULL
                         ORDER BY selected.sequence DESC LIMIT 1
                     )
                     UNION ALL
                     SELECT messages.id, messages.parent_message_id
                     FROM messages JOIN lineage ON messages.id = lineage.parent_message_id
                 )
                 SELECT messages.role FROM lineage JOIN messages ON messages.id = lineage.id
                 WHERE messages.id = ?1 AND messages.conversation_id = ?2",
                params![message_id, conversation_id, DEFAULT_PROFILE_ID],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| StorageError::not_found("That assistant response no longer exists."))?;
        if StoredRole::from_database(&role)? != StoredRole::Assistant {
            return Err(StorageError::invalid(
                "Only an assistant response can be rated.",
            ));
        }
        if let Some(rating) = rating {
            transaction.execute(
                "INSERT INTO response_ratings (message_id, rating, updated_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(message_id) DO UPDATE
                 SET rating = excluded.rating, updated_at_ms = excluded.updated_at_ms",
                params![message_id, rating.as_str(), now_ms()?],
            )?;
        } else {
            transaction.execute(
                "DELETE FROM response_ratings WHERE message_id = ?1",
                [message_id],
            )?;
        }
        transaction.commit()?;
        Ok(rating)
    }
}
