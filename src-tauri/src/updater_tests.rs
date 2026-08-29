//! Focused contracts for Bottie's Rust-owned production updater boundary.

use std::time::Duration;

use serde_json::to_string;

use super::updater::{
    CandidateMetadata, UpdateFailureKind, cancellable, present_candidate, redact_updater_failure,
};

#[test]
fn presents_no_update_without_exposing_transport_metadata() {
    let result = present_candidate("0.9.0", None).expect("no update should be a successful result");

    assert_eq!(result.status, "noUpdate");
    assert_eq!(result.current_version, "0.9.0");
    assert_eq!(result.version, None);
    assert_eq!(result.notes, None);
    let serialized = to_string(&result).expect("result should serialize");
    assert!(!serialized.contains("url"));
    assert!(!serialized.contains("signature"));
}

#[test]
fn presents_only_bounded_metadata_for_a_strict_upgrade() {
    let result = present_candidate(
        "0.9.0",
        Some(CandidateMetadata {
            version: "0.10.0".into(),
            notes: Some("A calm production update.".into()),
        }),
    )
    .expect("a newer numeric release should be accepted");

    assert_eq!(result.status, "updateAvailable");
    assert_eq!(result.version.as_deref(), Some("0.10.0"));
    assert_eq!(result.notes.as_deref(), Some("A calm production update."));
}

#[test]
fn omits_transport_links_and_bounds_release_notes() {
    for notes in [
        "Read https://example.invalid/private/latest.json",
        "Open /Users/example/private.key",
        r"Open C:\\Users\\example\\private.key",
        "Visit www.example.invalid",
        "Contact mailto:private@example.invalid",
    ] {
        let linked = present_candidate(
            "0.9.0",
            Some(CandidateMetadata {
                version: "0.10.0".into(),
                notes: Some(notes.into()),
            }),
        )
        .expect("the candidate version should remain valid");
        assert_eq!(linked.notes, None);
    }

    let bounded = present_candidate(
        "0.9.0",
        Some(CandidateMetadata {
            version: "0.10.0".into(),
            notes: Some("n".repeat(5_000)),
        }),
    )
    .expect("long release notes should be bounded");
    let notes = bounded.notes.expect("bounded notes should remain present");
    assert_eq!(notes.chars().count(), 4_096);
    assert!(notes.ends_with('…'));
}

#[test]
fn rejects_downgrades_and_equal_versions() {
    for version in ["0.9.0", "0.8.9"] {
        let error = present_candidate(
            "0.9.0",
            Some(CandidateMetadata {
                version: version.into(),
                notes: None,
            }),
        )
        .expect_err("non-upgrade candidates must fail closed");

        assert_eq!(error.code, "invalidVersion");
        assert!(!error.retryable);
    }
}

#[test]
fn maps_signature_timeout_and_internal_failures_to_path_free_errors() {
    let fixtures = [
        (UpdateFailureKind::InvalidSignature, "invalidSignature"),
        (UpdateFailureKind::Timeout, "timeout"),
        (UpdateFailureKind::Unavailable, "unavailable"),
    ];
    for (kind, code) in fixtures {
        let error = redact_updater_failure(
            kind,
            "https://github.com/hherb/bottie/releases/download/v9/private.sig /Users/example/private.key",
        );
        let serialized = to_string(&error).expect("error should serialize");
        assert_eq!(error.code, code);
        assert!(!serialized.contains("github.com"));
        assert!(!serialized.contains("/Users/"));
        assert!(!serialized.contains("private.sig"));
    }
}

#[tokio::test]
async fn cancellation_returns_one_fixed_path_free_error() {
    let (abort_handle, registration) = futures_util::future::AbortHandle::new_pair();
    let work = async {
        tokio::time::sleep(Duration::from_secs(60)).await;
        Ok::<_, super::updater::UpdateError>(())
    };
    abort_handle.abort();

    let error = cancellable(work, registration)
        .await
        .expect_err("aborted updater work should be cancelled");

    assert_eq!(error.code, "cancelled");
    assert!(error.retryable);
    assert!(!to_string(&error).unwrap().contains("Abort"));
}
