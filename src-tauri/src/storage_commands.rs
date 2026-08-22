//! Narrow Tauri commands for Rust-owned durable conversation storage.

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{
    AppState,
    storage::{
        ConversationRetentionPeriod, ConversationRetentionPolicy, ConversationSearchResult,
        ConversationSummary, ForkedConversation, MessageState, NewStoredMessage, ResponseRating,
        SemanticIndexProgress, StorageError, StorageRecoveryStatus, StoredAttachment,
        StoredConversation, StoredMessage, StoredRole,
    },
};

mod attachments;
mod export;

pub(crate) use attachments::ingest_attachments;
pub(crate) use export::{
    export_conversation_batch_json, export_conversation_json, export_conversation_markdown,
};

const SQLITE_FILTER_NAME: &str = "SQLite database";
const SQLITE_EXTENSIONS: &[&str] = &["sqlite3", "db"];
const BACKUP_FILE_NAME: &str = "bottie-backup.sqlite3";

/// Result of one native backup Save-dialog interaction without exposing a filesystem path.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupOutcome {
    /// Whether a complete verified snapshot was written or the user cancelled the dialog.
    status: BackupStatus,
    /// Saved leaf filename, absent when the dialog was cancelled.
    file_name: Option<String>,
}

/// Stable native backup outcomes returned to the presentation layer.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum BackupStatus {
    /// The SQLite snapshot was written and verified successfully.
    Saved,
    /// The user closed the native dialog without selecting a destination.
    Cancelled,
}

/// Result of one native restore interaction without exposing any filesystem path.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreOutcome {
    /// Whether a validated backup was restored or the user cancelled the interaction.
    status: RestoreStatus,
    /// Selected backup's leaf filename, absent when the interaction was cancelled.
    file_name: Option<String>,
    /// Application-private safety file or directory name, absent when cancelled.
    preserved_copy_name: Option<String>,
}

/// Stable native restore outcomes returned to the presentation layer.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum RestoreStatus {
    /// The selected backup replaced the live store after validation and safety copy.
    Restored,
    /// The user closed either native dialog without completing a restore.
    Cancelled,
}

#[tauri::command]
/// Returns path-redacted corruption state and verified automatic-recovery availability.
pub(crate) fn get_storage_recovery_status(
    state: State<'_, AppState>,
) -> Result<StorageRecoveryStatus, StorageError> {
    state.conversations.recovery_status()
}

#[tauri::command]
/// Returns durable path-free progress for Bottie's built-in semantic memory index.
pub(crate) fn get_semantic_index_progress(
    state: State<'_, AppState>,
) -> Result<SemanticIndexProgress, StorageError> {
    state.conversations.semantic_index_progress()
}

#[tauri::command]
/// Returns the built-in local profile's durable Trash retention policy.
pub(crate) fn get_conversation_retention_policy(
    state: State<'_, AppState>,
) -> Result<ConversationRetentionPolicy, StorageError> {
    state.conversations.conversation_retention_policy()
}

#[tauri::command]
/// Saves one bounded Trash retention period for enforcement on a later healthy startup.
pub(crate) fn set_conversation_retention_period(
    period: ConversationRetentionPeriod,
    state: State<'_, AppState>,
) -> Result<ConversationRetentionPolicy, StorageError> {
    state
        .conversations
        .set_conversation_retention_period(period)
}

#[tauri::command]
/// Resets only derived vectors under restore-safe worker coordination, then resumes indexing.
pub(crate) async fn reindex_semantic_memory(
    state: State<'_, AppState>,
) -> Result<SemanticIndexProgress, StorageError> {
    let _management = state.storage_management.lock().await;
    let conversations = state.conversations.clone();
    let semantic_indexing = state.semantic_indexing.clone();
    let progress = tauri::async_runtime::spawn_blocking(move || {
        let _semantic_pause = semantic_indexing.pause();
        conversations.reset_semantic_index()
    })
    .await
    .map_err(|_| StorageError::internal())??;
    state.semantic_indexing.wake();
    Ok(progress)
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
/// Saves a consistent snapshot with embedded attachment bytes through a Rust-owned native dialog.
pub(crate) async fn backup_conversation_store(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BackupOutcome, StorageError> {
    let _management = state.storage_management.lock().await;
    let selected = app
        .dialog()
        .file()
        .set_title("Back up Bottie local data")
        .set_file_name(BACKUP_FILE_NAME)
        .add_filter(SQLITE_FILTER_NAME, SQLITE_EXTENSIONS)
        .blocking_save_file();
    let Some(selected) = selected else {
        return Ok(BackupOutcome {
            status: BackupStatus::Cancelled,
            file_name: None,
        });
    };
    let path = selected.into_path().map_err(|_| StorageError::backup())?;
    let conversations = state.conversations.clone();
    let backup_path = path.clone();
    tauri::async_runtime::spawn_blocking(move || conversations.backup_to(&backup_path))
        .await
        .map_err(|_| StorageError::backup())??;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(BACKUP_FILE_NAME)
        .to_owned();
    Ok(BackupOutcome {
        status: BackupStatus::Saved,
        file_name: Some(file_name),
    })
}

#[tauri::command]
/// Restores a validated Bottie backup after creating an application-private safety snapshot.
pub(crate) async fn restore_conversation_store(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RestoreOutcome, StorageError> {
    let _management = state.storage_management.lock().await;
    if !state.runs.lock().await.is_empty() {
        return Err(StorageError::restore_while_active());
    }
    let selected = app
        .dialog()
        .file()
        .set_title("Restore Bottie local data")
        .add_filter(SQLITE_FILTER_NAME, SQLITE_EXTENSIONS)
        .blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(cancelled_restore());
    };
    let path = selected
        .into_path()
        .map_err(|_| StorageError::invalid_backup())?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("selected backup")
        .to_owned();
    let recovery_required = state.conversations.is_recovery_required();
    let preservation_detail = if recovery_required {
        "Bottie will preserve the damaged database files before replacement."
    } else {
        "A pre-restore safety copy will be created automatically."
    };
    let confirmed = app
        .dialog()
        .message(format!(
            "Restore {file_name}? This replaces the current Bottie conversations. \
             {preservation_detail}"
        ))
        .title("Restore Bottie local data")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Restore".into(),
            "Cancel".into(),
        ))
        .blocking_show();
    if !confirmed {
        return Ok(cancelled_restore());
    }
    if !state.runs.lock().await.is_empty() {
        return Err(StorageError::restore_while_active());
    }
    let conversations = state.conversations.clone();
    let attachment_processing = state.attachment_processing.clone();
    let semantic_indexing = state.semantic_indexing.clone();
    let safety_path = conversations.restore_preservation_path()?;
    let restore_path = path.clone();
    let worker_safety_path = safety_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _processing_pause = attachment_processing.pause();
        let _semantic_pause = semantic_indexing.pause();
        conversations.restore_from(&restore_path, &worker_safety_path)
    })
    .await
    .map_err(|_| StorageError::restore())??;
    state.attachment_processing.wake();
    state.semantic_indexing.wake();
    let preserved_copy_name = safety_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Bottie preserved local data")
        .to_owned();
    Ok(RestoreOutcome {
        status: RestoreStatus::Restored,
        file_name: Some(file_name),
        preserved_copy_name: Some(preserved_copy_name),
    })
}

#[tauri::command]
/// Restores the newest verified app-private automatic snapshot after native confirmation.
pub(crate) async fn restore_latest_automatic_backup(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RestoreOutcome, StorageError> {
    let _management = state.storage_management.lock().await;
    if !state.runs.lock().await.is_empty() {
        return Err(StorageError::restore_while_active());
    }
    let status = state.conversations.recovery_status()?;
    if status.latest_automatic_backup_at_ms.is_none() {
        return Err(StorageError::no_automatic_backup());
    }
    let confirmed = app
        .dialog()
        .message(
            "Restore the latest verified automatic backup? Bottie will preserve the damaged database files first.",
        )
        .title("Recover Bottie local data")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Restore".into(),
            "Cancel".into(),
        ))
        .blocking_show();
    if !confirmed {
        return Ok(cancelled_restore());
    }
    if !state.runs.lock().await.is_empty() {
        return Err(StorageError::restore_while_active());
    }
    let conversations = state.conversations.clone();
    let attachment_processing = state.attachment_processing.clone();
    let semantic_indexing = state.semantic_indexing.clone();
    let preservation = conversations.restore_preservation_path()?;
    let worker_preservation = preservation.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _processing_pause = attachment_processing.pause();
        let _semantic_pause = semantic_indexing.pause();
        conversations.restore_latest_automatic_backup(&worker_preservation)
    })
    .await
    .map_err(|_| StorageError::restore())??;
    state.attachment_processing.wake();
    state.semantic_indexing.wake();
    let preserved_copy_name = preservation
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Bottie preserved damaged data")
        .to_owned();
    Ok(RestoreOutcome {
        status: RestoreStatus::Restored,
        file_name: Some("latest automatic backup".into()),
        preserved_copy_name: Some(preserved_copy_name),
    })
}

/// Builds the neutral outcome shared by file-picker and confirmation cancellation.
fn cancelled_restore() -> RestoreOutcome {
    RestoreOutcome {
        status: RestoreStatus::Cancelled,
        file_name: None,
        preserved_copy_name: None,
    }
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
    attachment_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<StoredMessage, StorageError> {
    let stored = state.conversations.append_message_with_attachments(
        NewStoredMessage {
            conversation_id,
            role: StoredRole::User,
            text,
            reasoning: None,
            state: MessageState::Final,
            provider_id: None,
            model_id: None,
        },
        &attachment_ids,
    )?;
    state.semantic_indexing.wake();
    Ok(stored)
}

#[tauri::command]
/// Adds retained files to ordered conversation scope without exposing content or paths.
pub(crate) fn add_conversation_attachments(
    conversation_id: String,
    attachment_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<StoredAttachment>, StorageError> {
    state
        .conversations
        .add_conversation_attachments(&conversation_id, &attachment_ids)
}

#[tauri::command]
/// Detaches one file from conversation scope without deleting retained content.
pub(crate) fn remove_conversation_attachment(
    conversation_id: String,
    attachment_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<StoredAttachment>, StorageError> {
    state
        .conversations
        .remove_conversation_attachment(&conversation_id, &attachment_id)
}

#[tauri::command]
/// Detaches one retained file from a visible user message without deleting its content.
pub(crate) fn remove_conversation_message_attachment(
    conversation_id: String,
    message_id: String,
    attachment_id: String,
    state: State<'_, AppState>,
) -> Result<StoredMessage, StorageError> {
    state
        .conversations
        .remove_message_attachment(&conversation_id, &message_id, &attachment_id)
}

#[tauri::command]
/// Forks a visible user request onto a newly selected branch for editing or regeneration.
pub(crate) fn branch_conversation_message(
    conversation_id: String,
    message_id: String,
    text: String,
    state: State<'_, AppState>,
) -> Result<ForkedConversation, StorageError> {
    let forked =
        state
            .conversations
            .fork_from_user_message(&conversation_id, &message_id, &text)?;
    state.semantic_indexing.wake();
    Ok(forked)
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
/// Excludes or restores one active or archived conversation in native long-term memory.
pub(crate) fn set_conversation_memory_excluded(
    conversation_id: String,
    excluded: bool,
    state: State<'_, AppState>,
) -> Result<ConversationSummary, StorageError> {
    let summary = state
        .conversations
        .set_conversation_memory_excluded(&conversation_id, excluded)?;
    state.semantic_indexing.wake();
    Ok(summary)
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

#[tauri::command]
/// Permanently deletes one trashed conversation and its conversation-owned records.
pub(crate) fn forget_conversation(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<(), StorageError> {
    state.conversations.forget_conversation(&conversation_id)?;
    state.semantic_indexing.wake();
    Ok(())
}
