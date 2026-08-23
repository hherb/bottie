//! Bounded, redacted session diagnostics for native provider activity.

use std::{
    collections::VecDeque,
    fs,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::{
    AppState,
    inference::{ProviderError, redact_diagnostic},
};

/// Maximum number of diagnostic records retained during one application session.
const DIAGNOSTIC_CAPACITY: usize = 100;
/// Portable diagnostic document version, independent from storage schema versions.
const DIAGNOSTIC_EXPORT_VERSION: u8 = 1;
/// Stable export type discriminator for external readers.
const DIAGNOSTIC_EXPORT_FORMAT: &str = "bottie-local-diagnostics";
/// Native Save-dialog filter label for the portable document.
const DIAGNOSTIC_FILTER_NAME: &str = "JSON";
/// Native Save-dialog extension for the portable document.
const DIAGNOSTIC_EXTENSION: &str = "json";
/// Categories deliberately absent from every portable diagnostic document.
const DIAGNOSTIC_EXPORT_OMISSIONS: [&str; 6] = [
    "credentials_and_authentication_material",
    "provider_request_bodies",
    "provider_response_bodies",
    "raw_tool_arguments_and_results",
    "database_and_attachment_content",
    "native_filesystem_paths",
];

/// Shared asynchronous storage for bounded session diagnostics.
pub(crate) type Diagnostics = Arc<tauri::async_runtime::Mutex<VecDeque<DiagnosticEntry>>>;

/// One secret-redacted provider or generation diagnostic exposed to the interface.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticEntry {
    /// Milliseconds since the Unix epoch when the event was recorded.
    pub(crate) timestamp_ms: u64,
    /// Stable severity label used by the diagnostic presentation.
    pub(crate) level: &'static str,
    /// Short description of the recorded event.
    pub(crate) event: String,
    /// Provider identity when the event belongs to one provider.
    pub(crate) provider_id: Option<String>,
    /// Optional secret-redacted diagnostic detail.
    pub(crate) detail: Option<String>,
}

/// Prepared native-only diagnostic document and its normalized suggested filename.
#[derive(Debug)]
pub(crate) struct DiagnosticsFileExport {
    file_name: String,
    contents: String,
}

/// Result of one native diagnostic Save-dialog interaction without exposing a filesystem path.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsExportOutcome {
    /// Whether the document was written or the user cancelled the dialog.
    status: DiagnosticsExportStatus,
    /// Saved leaf filename, absent when the dialog was cancelled.
    file_name: Option<String>,
}

/// Stable native diagnostic export outcomes returned to the presentation layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum DiagnosticsExportStatus {
    /// The portable diagnostic document was written successfully.
    Saved,
    /// The user closed the native dialog without selecting a destination.
    Cancelled,
}

/// Stable path- and content-redacted failure returned by the export command.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsExportError {
    /// Stable machine-readable failure category.
    code: &'static str,
    /// Human-readable failure safe to show in the interface.
    message: &'static str,
}

/// Versioned portable document containing only the already-bounded session event fields.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsExportDocument {
    format: &'static str,
    version: u8,
    scope: &'static str,
    generated_at_ms: u64,
    omitted: &'static [&'static str],
    events: Vec<DiagnosticEntry>,
}

/// Appends one diagnostic while evicting the oldest record at capacity.
pub(crate) async fn record_diagnostic(
    diagnostics: &Diagnostics,
    level: &'static str,
    event: impl Into<String>,
    provider_id: Option<&str>,
    detail: Option<&str>,
) {
    let mut entries = diagnostics.lock().await;
    if entries.len() == DIAGNOSTIC_CAPACITY {
        entries.pop_front();
    }
    entries.push_back(DiagnosticEntry {
        timestamp_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        level,
        event: event.into(),
        provider_id: provider_id.map(str::to_owned),
        detail: detail.map(redact_diagnostic),
    });
}

/// Prepares one deterministic JSON snapshot while reapplying the native redaction boundary.
fn prepare_diagnostics_export(
    entries: Vec<DiagnosticEntry>,
    generated_at_ms: u64,
) -> Result<DiagnosticsFileExport, DiagnosticsExportError> {
    if entries.is_empty() {
        return Err(DiagnosticsExportError {
            code: "invalid_request",
            message: "There are no session diagnostics to export.",
        });
    }
    let events = entries
        .into_iter()
        .map(|mut entry| {
            entry.event = redact_diagnostic(&entry.event);
            entry.detail = entry.detail.as_deref().map(redact_diagnostic);
            entry
        })
        .collect();
    let document = DiagnosticsExportDocument {
        format: DIAGNOSTIC_EXPORT_FORMAT,
        version: DIAGNOSTIC_EXPORT_VERSION,
        scope: "current_session",
        generated_at_ms,
        omitted: &DIAGNOSTIC_EXPORT_OMISSIONS,
        events,
    };
    let mut contents = serde_json::to_string_pretty(&document).map_err(|_| export_failure())?;
    contents.push('\n');
    Ok(DiagnosticsFileExport {
        file_name: diagnostics_export_file_name(generated_at_ms),
        contents,
    })
}

/// Writes a prepared document when a destination exists, otherwise returns a clean cancellation.
fn write_diagnostics_export(
    selected_path: Option<PathBuf>,
    export: DiagnosticsFileExport,
) -> Result<DiagnosticsExportOutcome, DiagnosticsExportError> {
    let Some(path) = selected_path else {
        return Ok(DiagnosticsExportOutcome {
            status: DiagnosticsExportStatus::Cancelled,
            file_name: None,
        });
    };
    fs::write(&path, export.contents).map_err(|_| export_failure())?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&export.file_name)
        .to_owned();
    Ok(DiagnosticsExportOutcome {
        status: DiagnosticsExportStatus::Saved,
        file_name: Some(file_name),
    })
}

/// Builds a portable ASCII filename from the UTC generation date.
fn diagnostics_export_file_name(generated_at_ms: u64) -> String {
    let date = i64::try_from(generated_at_ms)
        .ok()
        .and_then(DateTime::<Utc>::from_timestamp_millis)
        .map(|timestamp| timestamp.format("%Y-%m-%d").to_string());
    match date {
        Some(date) => format!("bottie-diagnostics-{date}.json"),
        None => "bottie-diagnostics.json".into(),
    }
}

/// Creates the single stable failure used for serialization and native file-write faults.
fn export_failure() -> DiagnosticsExportError {
    DiagnosticsExportError {
        code: "internal",
        message: "Bottie could not save the diagnostics export.",
    }
}

#[tauri::command]
/// Saves the current bounded diagnostic session as versioned redacted JSON through a native dialog.
pub(crate) async fn export_diagnostics(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DiagnosticsExportOutcome, DiagnosticsExportError> {
    let entries = state.diagnostics.lock().await.iter().cloned().collect();
    let generated_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let export = prepare_diagnostics_export(entries, generated_at_ms)?;
    let selected = app
        .dialog()
        .file()
        .set_title("Export redacted Bottie diagnostics")
        .set_file_name(&export.file_name)
        .add_filter(DIAGNOSTIC_FILTER_NAME, &[DIAGNOSTIC_EXTENSION])
        .blocking_save_file();
    let selected_path = selected
        .map(|path| path.into_path().map_err(|_| export_failure()))
        .transpose()?;
    write_diagnostics_export(selected_path, export)
}

/// Redacts diagnostic detail attached to a normalized provider error.
pub(crate) fn sanitized(mut error: ProviderError) -> ProviderError {
    error.diagnostic = error.diagnostic.as_deref().map(redact_diagnostic);
    error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_history_is_bounded_and_redacted() {
        tauri::async_runtime::block_on(async {
            let diagnostics = Diagnostics::default();
            for index in 0..=DIAGNOSTIC_CAPACITY {
                record_diagnostic(
                    &diagnostics,
                    "info",
                    format!("event {index}"),
                    Some("ollama"),
                    Some("token=secret"),
                )
                .await;
            }

            let entries = diagnostics.lock().await;
            assert_eq!(entries.len(), DIAGNOSTIC_CAPACITY);
            assert_eq!(
                entries.front().map(|entry| entry.event.as_str()),
                Some("event 1")
            );
            assert_eq!(
                entries.back().and_then(|entry| entry.detail.as_deref()),
                Some("token=[redacted]")
            );
        });
    }

    #[test]
    fn diagnostic_ipc_entries_have_an_exact_redacted_shape() {
        tauri::async_runtime::block_on(async {
            let diagnostics = Diagnostics::default();
            record_diagnostic(
                &diagnostics,
                "error",
                "Native boundary rejected input",
                Some("openai"),
                Some("token=test-secret path=/Users/alice/private.txt"),
            )
            .await;
            let entries = diagnostics.lock().await;
            let value = serde_json::to_value(entries.front().expect("entry should exist"))
                .expect("diagnostic entry should serialize");

            assert_eq!(value["level"], "error");
            assert_eq!(value["event"], "Native boundary rejected input");
            assert_eq!(value["providerId"], "openai");
            assert_eq!(value["detail"], "token=[redacted] path=[redacted]");
            assert!(value["timestampMs"].is_u64());
            assert_eq!(
                value
                    .as_object()
                    .expect("diagnostic should be an object")
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>(),
                ["detail", "event", "level", "providerId", "timestampMs"]
            );
            let serialized = serde_json::to_string(&value).expect("diagnostic should serialize");
            assert!(!serialized.contains("test-secret"));
            assert!(!serialized.contains("/Users/alice"));
        });
    }
}

#[cfg(test)]
mod export_tests;
