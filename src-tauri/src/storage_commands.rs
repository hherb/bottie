//! Narrow Tauri commands for Rust-owned durable conversation storage.

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::{
    AppState,
    storage::{
        ConversationSearchResult, ConversationSummary, ForkedConversation, MessageState,
        NewStoredMessage, ResponseRating, StorageError, StoredConversation, StoredMessage,
        StoredRole,
    },
};

const MARKDOWN_FILTER_NAME: &str = "Markdown";
const MARKDOWN_EXTENSION: &str = "md";

/// Result of one native Save-dialog interaction without exposing a filesystem path.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConversationExportOutcome {
    /// Whether a file was written or the user cancelled the dialog.
    status: ConversationExportStatus,
    /// Saved leaf filename, absent when the dialog was cancelled.
    file_name: Option<String>,
}

/// Stable native export outcomes returned to the presentation layer.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ConversationExportStatus {
    /// The Markdown document was written successfully.
    Saved,
    /// The user closed the native dialog without selecting a destination.
    Cancelled,
}

#[tauri::command]
/// Lists recent conversations for the built-in local profile.
pub(crate) fn list_conversations(
    state: State<'_, AppState>,
) -> Result<Vec<ConversationSummary>, StorageError> {
    state.conversations.list_conversations()
}

#[tauri::command]
/// Searches active and archived conversation titles and user-visible message text.
pub(crate) fn search_conversations(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<ConversationSearchResult>, StorageError> {
    state.conversations.search_conversations(&query)
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
    state.conversations.open_conversation(&conversation_id)
}

#[tauri::command]
/// Saves the selected visible lineage as UTF-8 Markdown through a Rust-owned native dialog.
pub(crate) async fn export_conversation_markdown(
    conversation_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ConversationExportOutcome, StorageError> {
    let export = state
        .conversations
        .prepare_markdown_export(&conversation_id)?;
    let selected = app
        .dialog()
        .file()
        .set_title("Export conversation as Markdown")
        .set_file_name(&export.file_name)
        .add_filter(MARKDOWN_FILTER_NAME, &[MARKDOWN_EXTENSION])
        .blocking_save_file();
    let Some(selected) = selected else {
        return Ok(ConversationExportOutcome {
            status: ConversationExportStatus::Cancelled,
            file_name: None,
        });
    };
    let path = selected.into_path().map_err(|_| StorageError::export())?;
    export.write_to(&path)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&export.file_name)
        .to_owned();
    Ok(ConversationExportOutcome {
        status: ConversationExportStatus::Saved,
        file_name: Some(file_name),
    })
}

#[tauri::command]
/// Loads the exact conversation selected by the built-in local profile, when present.
pub(crate) fn load_last_open_conversation(
    state: State<'_, AppState>,
) -> Result<Option<StoredConversation>, StorageError> {
    state.conversations.load_last_open_conversation()
}

#[tauri::command]
/// Records an intentional blank new-chat view for the built-in local profile.
pub(crate) fn clear_last_open_conversation(state: State<'_, AppState>) -> Result<(), StorageError> {
    state.conversations.clear_last_open_conversation()
}

#[tauri::command]
/// Appends one final user-authored message through the narrow WebView storage boundary.
pub(crate) fn append_conversation_message(
    conversation_id: String,
    text: String,
    state: State<'_, AppState>,
) -> Result<StoredMessage, StorageError> {
    state.conversations.append_message(NewStoredMessage {
        conversation_id,
        role: StoredRole::User,
        text,
        reasoning: None,
        state: MessageState::Final,
        provider_id: None,
        model_id: None,
    })
}

#[tauri::command]
/// Forks a visible user request onto a newly selected branch for editing or regeneration.
pub(crate) fn branch_conversation_message(
    conversation_id: String,
    message_id: String,
    text: String,
    state: State<'_, AppState>,
) -> Result<ForkedConversation, StorageError> {
    state
        .conversations
        .fork_from_user_message(&conversation_id, &message_id, &text)
}

#[tauri::command]
/// Selects one durable branch and returns its reconstructed message lineage.
pub(crate) fn select_conversation_branch(
    conversation_id: String,
    branch_id: String,
    state: State<'_, AppState>,
) -> Result<StoredConversation, StorageError> {
    state
        .conversations
        .select_branch(&conversation_id, &branch_id)
}

#[tauri::command]
/// Sets or clears the local quality rating for one durable assistant response.
pub(crate) fn rate_conversation_response(
    conversation_id: String,
    message_id: String,
    rating: Option<ResponseRating>,
    state: State<'_, AppState>,
) -> Result<Option<ResponseRating>, StorageError> {
    state
        .conversations
        .rate_response(&conversation_id, &message_id, rating)
}

#[tauri::command]
/// Renames one active or archived conversation.
pub(crate) fn rename_conversation(
    conversation_id: String,
    title: String,
    state: State<'_, AppState>,
) -> Result<ConversationSummary, StorageError> {
    state
        .conversations
        .rename_conversation(&conversation_id, &title)
}

#[tauri::command]
/// Moves one conversation into or out of the archive.
pub(crate) fn set_conversation_archived(
    conversation_id: String,
    archived: bool,
    state: State<'_, AppState>,
) -> Result<ConversationSummary, StorageError> {
    state
        .conversations
        .set_conversation_archived(&conversation_id, archived)
}

#[tauri::command]
/// Moves one conversation to recoverable trash.
pub(crate) fn delete_conversation(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<ConversationSummary, StorageError> {
    state.conversations.delete_conversation(&conversation_id)
}

#[tauri::command]
/// Restores one trashed conversation to the active recent list.
pub(crate) fn restore_conversation(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<ConversationSummary, StorageError> {
    state.conversations.restore_conversation(&conversation_id)
}
