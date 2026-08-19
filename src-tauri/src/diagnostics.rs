//! Bounded, redacted session diagnostics for native provider activity.

use std::{
    collections::VecDeque,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

use crate::inference::{ProviderError, redact_diagnostic};

/// Maximum number of diagnostic records retained during one application session.
const DIAGNOSTIC_CAPACITY: usize = 100;

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
}
