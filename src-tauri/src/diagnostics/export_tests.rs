//! Portable local-diagnostics export contract tests.

use std::path::PathBuf;

use serde_json::{Value, json};

use super::{
    DiagnosticEntry, Diagnostics, prepare_diagnostics_export, record_diagnostic,
    write_diagnostics_export,
};

const GENERATED_AT_MS: u64 = 1_724_371_200_000;

/// Builds one diagnostic entry without depending on the system clock.
fn entry(timestamp_ms: u64, event: &str) -> DiagnosticEntry {
    DiagnosticEntry {
        timestamp_ms,
        level: "info",
        event: event.into(),
        provider_id: Some("ollama".into()),
        detail: Some("2 models in 4 ms".into()),
    }
}

#[test]
fn renders_a_versioned_deterministic_session_document() {
    let export = prepare_diagnostics_export(
        vec![entry(20, "Second recorded"), entry(10, "First recorded")],
        GENERATED_AT_MS,
    )
    .expect("non-empty diagnostics should export");
    let document: Value =
        serde_json::from_str(&export.contents).expect("export should be valid JSON");

    assert_eq!(export.file_name, "bottie-diagnostics-2024-08-23.json");
    assert_eq!(document["format"], "bottie-local-diagnostics");
    assert_eq!(document["version"], 1);
    assert_eq!(document["scope"], "current_session");
    assert_eq!(document["generatedAtMs"], GENERATED_AT_MS);
    assert_eq!(document["events"][0]["event"], "Second recorded");
    assert_eq!(document["events"][1]["event"], "First recorded");
    assert_eq!(document["events"][0]["providerId"], "ollama");
    assert_eq!(document["events"][0]["detail"], "2 models in 4 ms");
    assert_eq!(document["omitted"].as_array().map(Vec::len), Some(6));
    assert!(export.contents.ends_with('\n'));
}

#[test]
fn rejects_an_empty_session_with_a_stable_error() {
    let error = prepare_diagnostics_export(Vec::new(), GENERATED_AT_MS)
        .expect_err("empty diagnostics should not open a Save dialog");

    assert_eq!(error.code, "invalid_request");
    assert_eq!(error.message, "There are no session diagnostics to export.");
}

#[test]
fn export_reapplies_redaction_to_secret_path_and_content_shaped_detail() {
    tauri::async_runtime::block_on(async {
        let diagnostics = Diagnostics::default();
        record_diagnostic(
            &diagnostics,
            "error",
            "Credential access failed",
            Some("openai"),
            Some("token=top-secret"),
        )
        .await;
        record_diagnostic(
            &diagnostics,
            "error",
            "Native file read failed",
            None,
            Some("read failed at /Users/alice/Documents/private.txt"),
        )
        .await;
        record_diagnostic(
            &diagnostics,
            "error",
            "Native file read failed",
            None,
            Some("read failed at D:\\Private\\notes.txt"),
        )
        .await;
        record_diagnostic(
            &diagnostics,
            "error",
            "Provider request failed",
            Some("openai"),
            Some("request_body={\"prompt\":\"private patient text\"}"),
        )
        .await;
        let entries = diagnostics.lock().await.iter().cloned().collect();
        let export = prepare_diagnostics_export(entries, GENERATED_AT_MS)
            .expect("redacted diagnostics should export");

        assert!(!export.contents.contains("top-secret"));
        assert!(!export.contents.contains("private patient text"));
        assert!(!export.contents.contains("/Users/alice"));
        assert!(!export.contents.contains("D:\\\\Private"));
        assert!(export.contents.contains("[redacted]"));
    });
}

#[test]
fn cancellation_writes_nothing_and_returns_no_filename() {
    let directory =
        std::env::temp_dir().join(format!("bottie-diagnostics-{}", uuid::Uuid::new_v4()));
    let unexpected_path = directory.join("unexpected.json");
    let export = prepare_diagnostics_export(vec![entry(10, "Recorded")], GENERATED_AT_MS)
        .expect("diagnostics should prepare");
    let outcome = write_diagnostics_export(None::<PathBuf>, export)
        .expect("dialog cancellation should be successful");

    assert_eq!(outcome.status, super::DiagnosticsExportStatus::Cancelled);
    assert_eq!(outcome.file_name, None);
    assert_eq!(
        serde_json::to_value(outcome).expect("cancellation should serialize"),
        json!({"status": "cancelled", "fileName": null})
    );
    assert!(!unexpected_path.exists());
}

#[test]
fn writes_exact_utf8_json_and_returns_only_the_selected_leaf_name() {
    let directory =
        std::env::temp_dir().join(format!("bottie-diagnostics-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("shared diagnostics.json");
    let export = prepare_diagnostics_export(vec![entry(10, "Café recorded")], GENERATED_AT_MS)
        .expect("diagnostics should prepare");
    let expected = export.contents.clone();
    let outcome =
        write_diagnostics_export(Some(path.clone()), export).expect("export should write");

    assert_eq!(outcome.status, super::DiagnosticsExportStatus::Saved);
    assert_eq!(
        outcome.file_name.as_deref(),
        Some("shared diagnostics.json")
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), expected);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn file_write_failures_serialize_without_the_selected_path() {
    let directory =
        std::env::temp_dir().join(format!("bottie-diagnostics-{}", uuid::Uuid::new_v4()));
    let path = directory
        .join("missing parent")
        .join("private diagnostics.json");
    let export = prepare_diagnostics_export(vec![entry(10, "Recorded")], GENERATED_AT_MS)
        .expect("diagnostics should prepare");

    let error = write_diagnostics_export(Some(path.clone()), export)
        .expect_err("missing destination parent should fail safely");
    let serialized = serde_json::to_string(&error).expect("error should serialize");

    assert_eq!(
        serde_json::to_value(error).expect("error should serialize as a value"),
        json!({
            "code": "internal",
            "message": "Bottie could not save the diagnostics export."
        })
    );
    assert!(!serialized.contains(&path.to_string_lossy().to_string()));
}
