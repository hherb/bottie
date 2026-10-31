//! Native backup and restore commands, which hold the storage-management lock across their dialogs.

use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use super::outcomes::{
    BackupOutcome, RestoreOutcome, cancelled_backup, cancelled_restore, leaf_name, restored,
    saved_backup,
};
use crate::{
    AppState,
    diagnostics::record_diagnostic,
    run_cancellation::{IdleGeneration, lock_idle_generation},
    storage::{StorageError, staging_cleanup_warning, sweep_manual_backup_staging},
};

const SQLITE_FILTER_NAME: &str = "SQLite database";
const SQLITE_EXTENSIONS: &[&str] = &["sqlite3", "db"];
const BACKUP_FILE_NAME: &str = "bottie-backup.sqlite3";

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
        return Ok(cancelled_backup());
    };
    let path = selected.into_path().map_err(|_| StorageError::backup())?;
    let conversations = state.conversations.clone();
    let backup_path = path.clone();
    let (cleanup_failed, backup) = tauri::async_runtime::spawn_blocking(move || {
        let sweep = sweep_manual_backup_staging(&backup_path);
        (sweep.failed, conversations.backup_to(&backup_path))
    })
    .await
    .map_err(|_| StorageError::backup())?;
    // Reported before the backup's own result, so a failed backup still names the staging it could not clear.
    if let Some(detail) = staging_cleanup_warning(
        cleanup_failed,
        "backup",
        "the next backup into that folder retries them",
    ) {
        record_diagnostic(
            &state.diagnostics,
            "warn",
            "Backup staging cleanup incomplete",
            None,
            Some(&detail),
        )
        .await;
    }
    let skipped = backup?;
    if skipped > 0 {
        let detail = format!("{skipped} missing attachment or image file(s) left out");
        record_diagnostic(
            &state.diagnostics,
            "warn",
            "Backup saved without missing files",
            None,
            Some(&detail),
        )
        .await;
    }
    Ok(saved_backup(&path, BACKUP_FILE_NAME, skipped))
}

#[tauri::command]
/// Restores a validated Bottie backup after creating an application-private safety snapshot.
pub(crate) async fn restore_conversation_store(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RestoreOutcome, StorageError> {
    let _management = state.storage_management.lock().await;
    ensure_no_active_generation(&state).await?;
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
    let file_name = leaf_name(&path, "selected backup");
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
    let _idle = ensure_no_active_generation(&state).await?;
    let conversations = state.conversations.clone();
    let attachment_processing = state.attachment_processing.clone();
    let semantic_indexing = state.semantic_indexing.clone();
    let safety_path = conversations.restore_preservation_path()?;
    let restore_path = path.clone();
    let worker_safety_path = safety_path.clone();
    let missing_files = tauri::async_runtime::spawn_blocking(move || {
        let _processing_pause = attachment_processing.pause()?;
        let _semantic_pause = semantic_indexing.pause()?;
        conversations.restore_from(&restore_path, &worker_safety_path)?;
        Ok::<_, StorageError>(conversations.missing_content_files())
    })
    .await
    .map_err(|_| StorageError::restore())??;
    let missing_files = reported_missing_files(&state, missing_files).await;
    state.attachment_processing.wake();
    state.semantic_indexing.wake();
    Ok(restored(
        file_name,
        &safety_path,
        "Bottie preserved local data",
        missing_files,
        background_work_stopped(&state),
    ))
}

#[tauri::command]
/// Restores the newest verified app-private automatic snapshot after native confirmation.
pub(crate) async fn restore_latest_automatic_backup(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RestoreOutcome, StorageError> {
    let _management = state.storage_management.lock().await;
    ensure_no_active_generation(&state).await?;
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
    let _idle = ensure_no_active_generation(&state).await?;
    let conversations = state.conversations.clone();
    let attachment_processing = state.attachment_processing.clone();
    let semantic_indexing = state.semantic_indexing.clone();
    let preservation = conversations.restore_preservation_path()?;
    let worker_preservation = preservation.clone();
    let missing_files = tauri::async_runtime::spawn_blocking(move || {
        let _processing_pause = attachment_processing.pause()?;
        let _semantic_pause = semantic_indexing.pause()?;
        conversations.restore_latest_automatic_backup(&worker_preservation)?;
        Ok::<_, StorageError>(conversations.missing_content_files())
    })
    .await
    .map_err(|_| StorageError::restore())??;
    let missing_files = reported_missing_files(&state, missing_files).await;
    state.attachment_processing.wake();
    state.semantic_indexing.wake();
    Ok(restored(
        "latest automatic backup".into(),
        &preservation,
        "Bottie preserved damaged data",
        missing_files,
        background_work_stopped(&state),
    ))
}

/// Refuses a store replacement while any chat or image run could still write to the live store.
///
/// Hold the returned guard across the swap: new runs wait for it to drop, so none starts against the old store.
async fn ensure_no_active_generation(state: &AppState) -> Result<IdleGeneration<'_>, StorageError> {
    lock_idle_generation(&state.runs, &state.image_runs)
        .await
        .ok_or_else(StorageError::restore_while_active)
}

/// Whether a worker stopped, so the restored data's pending attachments and search index wait for a restart (#224).
///
/// Call after `wake()`, which also marks a worker it finds gone.
fn background_work_stopped(state: &AppState) -> bool {
    state.attachment_processing.is_stopped() || state.semantic_indexing.is_stopped()
}

/// Counts files the just-restored data lacks. The restore already succeeded, so a failed count is
/// `None` plus a `warn` diagnostic naming its error code rather than a failed restore (#228, #230).
async fn reported_missing_files(
    state: &AppState,
    counted: Result<usize, StorageError>,
) -> Option<usize> {
    match counted {
        Ok(count) => Some(count),
        Err(error) => {
            let detail = format!(
                "Bottie could not count attachment or image files missing from the restored data ({}).",
                error.code
            );
            record_diagnostic(
                &state.diagnostics,
                "warn",
                "Restored data not fully checked",
                None,
                Some(&detail),
            )
            .await;
            None
        }
    }
}
