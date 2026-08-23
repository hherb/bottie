//! Native Save-dialog commands for path-redacted conversation document export.

use serde::Serialize;
use std::path::Path;

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::{AppState, storage::StorageError};

const MARKDOWN_FILTER_NAME: &str = "Markdown";
const MARKDOWN_EXTENSION: &str = "md";
const JSON_FILTER_NAME: &str = "JSON";
const JSON_EXTENSION: &str = "json";
const BUNDLE_FILTER_NAME: &str = "Bottie export bundle";
const BUNDLE_EXTENSION: &str = "zip";

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
    /// The requested conversation document was written successfully.
    Saved,
    /// The user closed the native dialog without selecting a destination.
    Cancelled,
}

#[tauri::command]
/// Saves the selected lineage as Markdown plus referenced attachment bytes through a native dialog.
pub(crate) async fn export_conversation_markdown(
    conversation_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ConversationExportOutcome, StorageError> {
    let export = state
        .conversations
        .prepare_markdown_export(&conversation_id)?;
    save_export(
        &app,
        export,
        "Export conversation as Markdown",
        MARKDOWN_FILTER_NAME,
        MARKDOWN_EXTENSION,
    )
}

#[tauri::command]
/// Saves the selected lineage as versioned JSON plus referenced attachment bytes through a native dialog.
pub(crate) async fn export_conversation_json(
    conversation_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ConversationExportOutcome, StorageError> {
    let export = state.conversations.prepare_json_export(&conversation_id)?;
    save_export(
        &app,
        export,
        "Export conversation as JSON",
        JSON_FILTER_NAME,
        JSON_EXTENSION,
    )
}

#[tauri::command]
/// Saves all eligible selected lineages and referenced attachment bytes as one portable export.
pub(crate) async fn export_conversation_batch_json(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ConversationExportOutcome, StorageError> {
    let conversations = state.conversations.clone();
    let export =
        tauri::async_runtime::spawn_blocking(move || conversations.prepare_batch_json_export())
            .await
            .map_err(|_| StorageError::export())??;
    save_export(
        &app,
        export,
        "Export all conversations as JSON",
        JSON_FILTER_NAME,
        JSON_EXTENSION,
    )
}

/// Opens one format-filtered native dialog, writes the prepared payload, and returns only its leaf filename.
fn save_export(
    app: &AppHandle,
    export: crate::storage::ConversationFileExport,
    title: &str,
    filter_name: &str,
    extension: &str,
) -> Result<ConversationExportOutcome, StorageError> {
    let (filter_name, extension) = if export.is_bundle() {
        (BUNDLE_FILTER_NAME, BUNDLE_EXTENSION)
    } else {
        (filter_name, extension)
    };
    let selected = app
        .dialog()
        .file()
        .set_title(title)
        .set_file_name(&export.file_name)
        .add_filter(filter_name, &[extension])
        .blocking_save_file();
    let Some(selected) = selected else {
        return Ok(cancelled_export());
    };
    let path = selected.into_path().map_err(|_| StorageError::export())?;
    export.write_to(&path)?;
    Ok(saved_export(&path, &export.file_name))
}

/// Builds the neutral outcome for a cancelled native export picker.
fn cancelled_export() -> ConversationExportOutcome {
    ConversationExportOutcome {
        status: ConversationExportStatus::Cancelled,
        file_name: None,
    }
}

/// Builds a saved export outcome from only the selected leaf filename.
fn saved_export(path: &Path, fallback: &str) -> ConversationExportOutcome {
    ConversationExportOutcome {
        status: ConversationExportStatus::Saved,
        file_name: Some(
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(fallback)
                .to_owned(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn export_outcomes_serialize_only_status_and_leaf_filename() {
        let saved = saved_export(
            Path::new("/Users/alice/Documents/private/conversation.zip"),
            "bottie-conversation.zip",
        );

        assert_eq!(
            serde_json::to_value(saved).expect("saved outcome should serialize"),
            json!({"status": "saved", "fileName": "conversation.zip"})
        );
        assert_eq!(
            serde_json::to_value(cancelled_export()).expect("cancelled outcome should serialize"),
            json!({"status": "cancelled", "fileName": null})
        );
    }
}
