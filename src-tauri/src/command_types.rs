//! Narrow data-transfer types used by native application commands.

use serde::{Deserialize, Serialize};

/// Static identity and storage information for the running native application.
#[derive(Serialize)]
pub(crate) struct AppInfo {
    /// Application display name.
    pub(crate) name: &'static str,
    /// Package version compiled into the native application.
    pub(crate) version: &'static str,
    /// Current storage-routing label.
    pub(crate) storage: &'static str,
}

/// Draft local-provider endpoint submitted for a connection test.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderConnectionDraft {
    /// Stable local-provider identity.
    pub(crate) provider_id: String,
    /// Candidate loopback base URL.
    pub(crate) base_url: String,
}

/// Provider and model pair selected by the user.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderSelection {
    /// Stable local-provider identity.
    pub(crate) provider_id: String,
    /// Provider-owned model identity.
    pub(crate) model_id: String,
}

/// Successful result of testing one draft local-provider endpoint.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderConnectionTest {
    /// Stable local-provider identity.
    pub(crate) provider_id: String,
    /// Normalized loopback base URL.
    pub(crate) base_url: String,
    /// Number of models reported by the provider.
    pub(crate) model_count: usize,
    /// End-to-end connection-test duration in milliseconds.
    pub(crate) elapsed_ms: u64,
    /// User-readable connection summary.
    pub(crate) message: String,
}
