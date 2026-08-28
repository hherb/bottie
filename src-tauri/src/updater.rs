//! Rust-owned production update checks, review state, cancellation, and installation.

use std::{future::Future, sync::Mutex, time::Duration};

use futures_util::future::{AbortHandle, AbortRegistration, Abortable};
use semver::Version;
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_updater::UpdaterExt;

const UPDATE_ENDPOINT: &str =
    "https://github.com/hherb/bottie/releases/latest/download/latest.json";
const UPDATE_PUBLIC_KEY: &str = include_str!("../../distribution/update/bottie-updater.pub");
const UPDATE_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_RELEASE_NOTES_CHARS: usize = 4_096;

/// Native-only updater operation and the exact version last reviewed by the user.
#[derive(Default)]
pub struct UpdaterState {
    operation: Mutex<Option<AbortHandle>>,
    reviewed_version: Mutex<Option<String>>,
}

/// Path-free update information that is safe to present in the WebView.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    /// Whether a strict upgrade is available.
    pub status: &'static str,
    /// Numeric version embedded in the running application.
    pub current_version: String,
    /// Numeric strict-upgrade version, if available.
    pub version: Option<String>,
    /// Bounded plain release notes, if present and free of transport metadata.
    pub notes: Option<String>,
}

/// Successful path-free outcome after the native installer accepts an update.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInstallResult {
    /// Stable terminal status for the user-controlled operation.
    pub status: &'static str,
    /// Exact reviewed version handed to the native installer.
    pub version: String,
}

/// Whether an in-flight native update operation received a cancellation request.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCancelResult {
    /// True only when a check or installation operation was active.
    pub cancellation_requested: bool,
}

/// Fixed updater failure safe for WebView IPC.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateError {
    /// Stable presentation code without upstream details.
    pub code: &'static str,
    /// Fixed actionable message without paths, URLs, signatures, or provider bytes.
    pub message: &'static str,
    /// Whether a later explicit user action may reasonably succeed.
    pub retryable: bool,
}

/// Candidate fields retained after Rust discards transport and signature metadata.
#[derive(Debug)]
pub(crate) struct CandidateMetadata {
    pub(crate) version: String,
    pub(crate) notes: Option<String>,
}

/// Closed classifications used to reduce updater failures before IPC.
#[derive(Clone, Copy, Debug)]
pub(crate) enum UpdateFailureKind {
    InvalidSignature,
    Timeout,
    Unavailable,
}

impl UpdaterState {
    /// Starts one cancellable operation and rejects concurrent update work.
    fn begin(&self) -> Result<AbortRegistration, UpdateError> {
        let mut operation = self.operation.lock().map_err(|_| internal_state_error())?;
        if operation.is_some() {
            return Err(UpdateError {
                code: "busy",
                message: "Another update action is already in progress.",
                retryable: true,
            });
        }
        let (handle, registration) = AbortHandle::new_pair();
        *operation = Some(handle);
        Ok(registration)
    }

    /// Clears the completed operation handle without exposing cancellation internals.
    fn finish(&self) {
        if let Ok(mut operation) = self.operation.lock() {
            *operation = None;
        }
    }

    /// Replaces the exact reviewed candidate version after a successful check.
    fn review(&self, version: Option<String>) -> Result<(), UpdateError> {
        *self
            .reviewed_version
            .lock()
            .map_err(|_| internal_state_error())? = version;
        Ok(())
    }

    /// Consumes the candidate reviewed by the user before installation begins.
    fn take_reviewed(&self) -> Result<String, UpdateError> {
        self.reviewed_version
            .lock()
            .map_err(|_| internal_state_error())?
            .take()
            .ok_or(UpdateError {
                code: "checkRequired",
                message: "Check for an update before installing it.",
                retryable: true,
            })
    }
}

/// Checks Bottie's one fixed HTTPS manifest and stores only the reviewed strict-upgrade version.
#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    state: State<'_, UpdaterState>,
) -> Result<UpdateCheckResult, UpdateError> {
    let registration = state.begin()?;
    state.review(None)?;
    let result = cancellable(query_candidate(app), registration).await;
    state.finish();
    match result {
        Ok(candidate) => {
            let presented = present_candidate(env!("CARGO_PKG_VERSION"), candidate)?;
            state.review(presented.version.clone())?;
            Ok(presented)
        }
        Err(error) => {
            state.review(None)?;
            Err(error)
        }
    }
}

/// Rechecks and installs only the exact version returned by the user's preceding check.
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    state: State<'_, UpdaterState>,
) -> Result<UpdateInstallResult, UpdateError> {
    let registration = state.begin()?;
    let reviewed_version = match state.take_reviewed() {
        Ok(version) => version,
        Err(error) => {
            state.finish();
            return Err(error);
        }
    };
    let result = cancellable(
        install_reviewed_candidate(app, reviewed_version),
        registration,
    )
    .await;
    state.finish();
    result
}

/// Cancels one in-flight native check or installation without accepting an operation identifier.
#[tauri::command]
pub fn cancel_update_operation(state: State<'_, UpdaterState>) -> UpdateCancelResult {
    let cancellation_requested = state
        .operation
        .lock()
        .ok()
        .and_then(|operation| operation.as_ref().cloned())
        .is_some_and(|handle| {
            handle.abort();
            true
        });
    UpdateCancelResult {
        cancellation_requested,
    }
}

/// Runs one future behind the updater's explicit native cancellation boundary.
pub(crate) async fn cancellable<F, T>(
    future: F,
    registration: AbortRegistration,
) -> Result<T, UpdateError>
where
    F: Future<Output = Result<T, UpdateError>>,
{
    Abortable::new(future, registration)
        .await
        .unwrap_or_else(|_| Err(cancelled_error()))
}

/// Builds a path-free check result and independently rejects non-upgrade candidates.
pub(crate) fn present_candidate(
    current_version: &str,
    candidate: Option<CandidateMetadata>,
) -> Result<UpdateCheckResult, UpdateError> {
    let current = Version::parse(current_version).map_err(|_| internal_state_error())?;
    let Some(candidate) = candidate else {
        return Ok(UpdateCheckResult {
            status: "noUpdate",
            current_version: current.to_string(),
            version: None,
            notes: None,
        });
    };
    let version = Version::parse(&candidate.version).map_err(|_| invalid_version_error())?;
    if version <= current {
        return Err(invalid_version_error());
    }
    Ok(UpdateCheckResult {
        status: "updateAvailable",
        current_version: current.to_string(),
        version: Some(version.to_string()),
        notes: bounded_release_notes(candidate.notes),
    })
}

/// Reduces every updater failure to a fixed message without retaining its diagnostic input.
pub(crate) fn redact_updater_failure(kind: UpdateFailureKind, _diagnostic: &str) -> UpdateError {
    match kind {
        UpdateFailureKind::InvalidSignature => UpdateError {
            code: "invalidSignature",
            message: "The downloaded update could not be verified and was not installed.",
            retryable: false,
        },
        UpdateFailureKind::Timeout => UpdateError {
            code: "timeout",
            message: "The update service did not respond in time.",
            retryable: true,
        },
        UpdateFailureKind::Unavailable => UpdateError {
            code: "unavailable",
            message: "Bottie could not complete the update action.",
            retryable: true,
        },
    }
}

/// Checks the static endpoint while discarding its URLs, signatures, dates, and download metadata.
async fn query_candidate(app: AppHandle) -> Result<Option<CandidateMetadata>, UpdateError> {
    let updater = configured_updater(&app)?;
    updater
        .check()
        .await
        .map(|candidate| {
            candidate.map(|update| CandidateMetadata {
                version: update.version.to_string(),
                notes: update.body,
            })
        })
        .map_err(upstream_error)
}

/// Rechecks the endpoint, rejects candidate changes, then verifies and invokes the native installer.
async fn install_reviewed_candidate(
    app: AppHandle,
    reviewed_version: String,
) -> Result<UpdateInstallResult, UpdateError> {
    let updater = configured_updater(&app)?;
    let update = updater
        .check()
        .await
        .map_err(upstream_error)?
        .ok_or(UpdateError {
            code: "changed",
            message: "The reviewed update is no longer available. Check again before installing.",
            retryable: true,
        })?;
    let presented = present_candidate(
        env!("CARGO_PKG_VERSION"),
        Some(CandidateMetadata {
            version: update.version.to_string(),
            notes: None,
        }),
    )?;
    if presented.version.as_deref() != Some(reviewed_version.as_str()) {
        return Err(UpdateError {
            code: "changed",
            message: "The available update changed. Review it again before installing.",
            retryable: true,
        });
    }
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(upstream_error)?;
    Ok(UpdateInstallResult {
        status: "installed",
        version: reviewed_version,
    })
}

/// Builds one timeout-bounded updater using only Rust-owned immutable trust inputs.
fn configured_updater(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, UpdateError> {
    let endpoint = UPDATE_ENDPOINT
        .parse()
        .map_err(|_| internal_state_error())?;
    app.updater_builder()
        .endpoints(vec![endpoint])
        .map_err(upstream_error)?
        .pubkey(UPDATE_PUBLIC_KEY.trim())
        .timeout(UPDATE_TIMEOUT)
        .build()
        .map_err(upstream_error)
}

/// Removes control bytes, links, paths, and excess text from optional presentation notes.
fn bounded_release_notes(notes: Option<String>) -> Option<String> {
    let notes = notes?;
    let lowercase = notes.to_ascii_lowercase();
    if notes.contains('/')
        || notes.contains('\\')
        || lowercase.contains("www.")
        || lowercase.contains("mailto:")
    {
        return None;
    }
    let mut bounded = notes
        .trim()
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .take(MAX_RELEASE_NOTES_CHARS + 1)
        .collect::<String>();
    if bounded.is_empty() {
        return None;
    }
    if bounded.chars().count() > MAX_RELEASE_NOTES_CHARS {
        bounded = bounded.chars().take(MAX_RELEASE_NOTES_CHARS - 1).collect();
        bounded.push('…');
    }
    Some(bounded)
}

/// Classifies an upstream error using its native-only diagnostic, then discards the diagnostic.
fn upstream_error(error: tauri_plugin_updater::Error) -> UpdateError {
    let diagnostic = error.to_string();
    let kind = match &error {
        tauri_plugin_updater::Error::Minisign(_)
        | tauri_plugin_updater::Error::Base64(_)
        | tauri_plugin_updater::Error::SignatureUtf8(_) => UpdateFailureKind::InvalidSignature,
        tauri_plugin_updater::Error::Reqwest(error) if error.is_timeout() => {
            UpdateFailureKind::Timeout
        }
        tauri_plugin_updater::Error::Network(message)
            if message.to_ascii_lowercase().contains("timeout") =>
        {
            UpdateFailureKind::Timeout
        }
        _ => UpdateFailureKind::Unavailable,
    };
    redact_updater_failure(kind, &diagnostic)
}

/// Returns the fixed cancellation error shared by check and installation.
fn cancelled_error() -> UpdateError {
    UpdateError {
        code: "cancelled",
        message: "The update action was cancelled.",
        retryable: true,
    }
}

/// Returns one fail-closed candidate-version error.
fn invalid_version_error() -> UpdateError {
    UpdateError {
        code: "invalidVersion",
        message: "The update service returned a version that is not newer than this Bottie installation.",
        retryable: false,
    }
}

/// Returns one fixed synchronization or embedded-configuration error.
fn internal_state_error() -> UpdateError {
    UpdateError {
        code: "internal",
        message: "Bottie's native update state is unavailable.",
        retryable: true,
    }
}
