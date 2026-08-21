//! Native picker and path-redacted attachment-ingestion command.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::{
    AppState,
    storage::{IngestedAttachment, MAX_ATTACHMENT_SELECTION_COUNT, StorageError},
};

/// Result of one native multi-file picker interaction.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachmentIngestOutcome {
    /// Whether files were selected or the picker was cancelled.
    status: AttachmentPickerStatus,
    /// Successfully retained safe attachment metadata.
    attachments: Vec<IngestedAttachment>,
    /// Individually rejected selections without source paths.
    rejections: Vec<AttachmentRejection>,
}

/// Stable picker outcome returned to the presentation layer.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum AttachmentPickerStatus {
    /// At least one local file was selected for bounded ingestion.
    Selected,
    /// The user closed the native picker without choosing files.
    Cancelled,
}

/// Path-redacted failure for one selected file while other selections continue.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentRejection {
    /// Sanitized or generic leaf label for the rejected selection.
    display_name: String,
    /// Stable, user-readable policy or read failure.
    message: String,
}

#[tauri::command]
/// Selects and ingests bounded local files without returning any filesystem path.
pub(crate) async fn ingest_attachments(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AttachmentIngestOutcome, StorageError> {
    let selected = app
        .dialog()
        .file()
        .set_title("Attach local files")
        .blocking_pick_files();
    let Some(selected) = selected else {
        return Ok(AttachmentIngestOutcome {
            status: AttachmentPickerStatus::Cancelled,
            attachments: Vec::new(),
            rejections: Vec::new(),
        });
    };
    if selected.len() > MAX_ATTACHMENT_SELECTION_COUNT {
        return Err(StorageError::invalid(format!(
            "Choose at most {MAX_ATTACHMENT_SELECTION_COUNT} files at a time."
        )));
    }
    let paths = selected
        .into_iter()
        .map(|selected| {
            selected
                .into_path()
                .map_err(|_| StorageError::attachment_read())
        })
        .collect::<Result<Vec<PathBuf>, StorageError>>()?;
    let _management = state.storage_management.lock().await;
    let conversations = state.conversations.clone();
    let outcome =
        tauri::async_runtime::spawn_blocking(move || ingest_selected_paths(&conversations, paths))
            .await
            .map_err(|_| StorageError::internal())?;
    state.attachment_processing.wake();
    Ok(outcome)
}

/// Ingests selections independently so one policy rejection does not discard valid peers.
fn ingest_selected_paths(
    conversations: &crate::storage::ConversationStore,
    paths: Vec<PathBuf>,
) -> AttachmentIngestOutcome {
    let mut attachments = Vec::new();
    let mut rejections = Vec::new();
    for path in paths {
        match conversations.ingest_attachment(&path) {
            Ok(attachment) => attachments.push(attachment),
            Err(error) => rejections.push(AttachmentRejection {
                display_name: path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(crate::storage::attachments::safe_display_name)
                    .unwrap_or_else(|| "attachment".into()),
                message: error.message,
            }),
        }
    }
    AttachmentIngestOutcome {
        status: AttachmentPickerStatus::Selected,
        attachments,
        rejections,
    }
}
