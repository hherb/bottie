//! Path-redacted native actions for assistant-owned generated PNGs.

use std::{fs, path::Path, process::Command};

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{
    AppState,
    storage::{StorageError, StoredConversation},
};

const PNG_FILTER_NAME: &str = "PNG image";
const PNG_EXTENSION: &str = "png";

/// Result of one generated-image native action without a filesystem path.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeneratedAssetActionOutcome {
    /// Stable action result.
    status: GeneratedAssetActionStatus,
    /// Exported leaf filename, absent for open and cancellation.
    file_name: Option<String>,
}

/// Stable native generated-image action outcomes.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum GeneratedAssetActionStatus {
    /// The operating system accepted the app-owned PNG for opening.
    Opened,
    /// The selected destination received the PNG.
    Saved,
    /// The user closed the native Save dialog.
    Cancelled,
}

#[tauri::command]
/// Opens one completed selected-branch PNG with the operating system default viewer.
pub(crate) fn open_generated_asset(
    asset_id: String,
    state: State<'_, AppState>,
) -> Result<GeneratedAssetActionOutcome, StorageError> {
    let asset = state.conversations.generated_asset_file(&asset_id)?;
    open_path(&asset.path)?;
    Ok(GeneratedAssetActionOutcome {
        status: GeneratedAssetActionStatus::Opened,
        file_name: None,
    })
}

#[tauri::command]
/// Exports one completed selected-branch PNG through a Rust-owned native Save dialog.
pub(crate) fn export_generated_asset(
    asset_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<GeneratedAssetActionOutcome, StorageError> {
    let asset = state.conversations.generated_asset_file(&asset_id)?;
    let selected = app
        .dialog()
        .file()
        .set_title("Export generated image")
        .set_file_name(&asset.file_name)
        .add_filter(PNG_FILTER_NAME, &[PNG_EXTENSION])
        .blocking_save_file();
    let Some(selected) = selected else {
        return Ok(GeneratedAssetActionOutcome {
            status: GeneratedAssetActionStatus::Cancelled,
            file_name: None,
        });
    };
    let destination = selected.into_path().map_err(|_| StorageError::export())?;
    fs::copy(&asset.path, &destination).map_err(|_| StorageError::export())?;
    Ok(GeneratedAssetActionOutcome {
        status: GeneratedAssetActionStatus::Saved,
        file_name: Some(leaf_name(&destination, &asset.file_name)),
    })
}

#[tauri::command]
/// Deletes one completed selected-branch generated output and returns the refreshed conversation.
pub(crate) fn delete_generated_asset(
    asset_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<StoredConversation>, StorageError> {
    state.conversations.generated_asset_file(&asset_id)?;
    let confirmed = app
        .dialog()
        .message(concat!(
            "Delete this generated image? Its private PNG bytes will also be removed ",
            "when no other output uses them."
        ))
        .title("Delete generated image")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Delete".into(),
            "Cancel".into(),
        ))
        .blocking_show();
    if !confirmed {
        return Ok(None);
    }
    state
        .conversations
        .delete_generated_asset(&asset_id)
        .map(Some)
}

/// Opens a fixed app-owned path without shell interpretation.
fn open_path(path: &Path) -> Result<(), StorageError> {
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("rundll32");
        command.arg("url.dll,FileProtocolHandler");
        command
    };
    command
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|_| StorageError::internal())
}

/// Returns only one selected leaf filename with a safe fallback.
fn leaf_name(path: &Path, fallback: &str) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(fallback)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn action_outcomes_serialize_without_native_paths_or_content_identity() {
        let saved = GeneratedAssetActionOutcome {
            status: GeneratedAssetActionStatus::Saved,
            file_name: Some("image.png".into()),
        };
        let opened = GeneratedAssetActionOutcome {
            status: GeneratedAssetActionStatus::Opened,
            file_name: None,
        };

        assert_eq!(
            serde_json::to_value(saved).expect("saved action should serialize"),
            json!({"status": "saved", "fileName": "image.png"})
        );
        assert_eq!(
            serde_json::to_value(opened).expect("open action should serialize"),
            json!({"status": "opened", "fileName": null})
        );
    }
}
