//! Narrow Tauri commands for Rust-owned durable conversation storage.

use tauri::State;

use crate::{
    AppState,
    storage::{
        ConversationSummary, NewStoredMessage, StorageError, StoredConversation, StoredMessage,
    },
};

#[tauri::command]
/// Lists recent conversations for the built-in local profile.
pub(crate) fn list_conversations(
    state: State<'_, AppState>,
) -> Result<Vec<ConversationSummary>, StorageError> {
    state.conversations.list_conversations()
}

#[tauri::command]
/// Creates one empty durable conversation with its initial main branch.
pub(crate) fn create_conversation(
    title: String,
    state: State<'_, AppState>,
) -> Result<StoredConversation, StorageError> {
    state.conversations.create_conversation(&title)
}

#[tauri::command]
/// Loads one durable conversation and its ordered main-branch messages.
pub(crate) fn load_conversation(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<StoredConversation, StorageError> {
    state.conversations.load_conversation(&conversation_id)
}

#[tauri::command]
/// Appends one immutable message and its content blocks to a conversation.
pub(crate) fn append_conversation_message(
    message: NewStoredMessage,
    state: State<'_, AppState>,
) -> Result<StoredMessage, StorageError> {
    state.conversations.append_message(message)
}
