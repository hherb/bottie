//! Profile-owned last-open conversation selection.

use rusqlite::{OptionalExtension, params};

use super::{
    ConversationStore, DEFAULT_PROFILE_ID, StorageError, StoredConversation,
    load_conversation_from_connection,
};

impl ConversationStore {
    /// Opens one conversation and atomically records it as the profile's selected thread.
    pub(crate) fn open_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<StoredConversation, StorageError> {
        let mut connection = self.open()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let conversation = load_conversation_from_connection(&transaction, conversation_id)?;
        transaction.execute(
            "UPDATE profiles SET last_open_conversation_id = ?1 WHERE id = ?2",
            params![conversation_id, DEFAULT_PROFILE_ID],
        )?;
        transaction.commit()?;
        Ok(conversation)
    }

    /// Loads the exact selected conversation, or the durable blank-new-chat state.
    pub(crate) fn load_last_open_conversation(
        &self,
    ) -> Result<Option<StoredConversation>, StorageError> {
        let connection = self.open()?;
        let conversation_id: Option<String> = connection
            .query_row(
                "SELECT last_open_conversation_id FROM profiles WHERE id = ?1",
                [DEFAULT_PROFILE_ID],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        conversation_id
            .map(|id| load_conversation_from_connection(&connection, &id))
            .transpose()
    }

    /// Persists that the profile intentionally has a blank new-chat view open.
    pub(crate) fn clear_last_open_conversation(&self) -> Result<(), StorageError> {
        let connection = self.open()?;
        connection.execute(
            "UPDATE profiles SET last_open_conversation_id = NULL WHERE id = ?1",
            [DEFAULT_PROFILE_ID],
        )?;
        Ok(())
    }
}
