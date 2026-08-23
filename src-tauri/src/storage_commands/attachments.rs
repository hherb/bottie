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
        return Ok(cancelled_ingest());
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

/// Builds the path-free neutral outcome for native picker cancellation.
fn cancelled_ingest() -> AttachmentIngestOutcome {
    AttachmentIngestOutcome {
        status: AttachmentPickerStatus::Cancelled,
        attachments: Vec::new(),
        rejections: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::storage::ConversationStore;

    /// Creates one isolated store without opening the live application database.
    fn isolated_store() -> (PathBuf, ConversationStore) {
        let directory =
            std::env::temp_dir().join(format!("bottie-ipc-attachments-{}", uuid::Uuid::new_v4()));
        let store = ConversationStore::initialize(directory.join("bottie.sqlite3"))
            .expect("isolated storage should initialize");
        (directory, store)
    }

    #[test]
    fn cancelled_ingestion_has_an_exact_empty_ipc_shape() {
        assert_eq!(
            serde_json::to_value(cancelled_ingest()).expect("cancellation should serialize"),
            json!({"status": "cancelled", "attachments": [], "rejections": []})
        );
    }

    #[test]
    fn selected_ingestion_omits_paths_hashes_bytes_and_extracted_content() {
        let (directory, store) = isolated_store();
        let private_directory = directory.join("private source");
        std::fs::create_dir_all(&private_directory).expect("fixture directory should exist");
        let accepted_path = private_directory.join("notes.md");
        let missing_path = private_directory.join("missing.txt");
        std::fs::write(&accepted_path, b"test-only attachment content")
            .expect("fixture should be written");

        let outcome = ingest_selected_paths(&store, vec![accepted_path.clone(), missing_path]);
        let value = serde_json::to_value(outcome).expect("ingestion outcome should serialize");
        let serialized = serde_json::to_string(&value).expect("outcome JSON should serialize");
        let attachment = &value["attachments"][0];
        let keys = attachment
            .as_object()
            .expect("attachment should be an object")
            .keys()
            .cloned()
            .collect::<Vec<_>>();

        assert_eq!(value["status"], "selected");
        assert_eq!(value["attachments"].as_array().map(Vec::len), Some(1));
        assert_eq!(value["rejections"].as_array().map(Vec::len), Some(1));
        assert_eq!(attachment["displayName"], "notes.md");
        assert_eq!(
            keys,
            [
                "byteSize",
                "displayName",
                "duplicate",
                "extraction",
                "id",
                "indexing",
                "mimeType",
                "normalization"
            ]
        );
        assert!(!serialized.contains(&directory.to_string_lossy().to_string()));
        assert!(!serialized.contains("test-only attachment content"));
        assert!(!serialized.contains("sha256"));
        assert!(!serialized.contains("sourcePath"));
        assert!(!serialized.contains("extractedText"));

        std::fs::remove_dir_all(directory).expect("fixture should be removed");
    }
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
