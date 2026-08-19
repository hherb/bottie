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

/// Draft provider endpoint submitted for a connection test.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderConnectionDraft {
    /// Stable provider identity.
    pub(crate) provider_id: String,
    /// Candidate local or remote base URL.
    pub(crate) base_url: String,
    /// Optional unsaved remote API key used only for this test.
    pub(crate) api_key: Option<String>,
}

/// Provider and model pair selected by the user.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderSelection {
    /// Stable provider identity.
    pub(crate) provider_id: String,
    /// Provider-owned model identity.
    pub(crate) model_id: String,
}

/// One remote-provider credential update submitted to the native vault.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderCredentialUpdate {
    /// Stable remote-provider identity.
    pub(crate) provider_id: String,
    /// Replacement key, or `None` to retain the current key.
    pub(crate) api_key: Option<String>,
    /// Whether the existing vault entry should be deleted.
    pub(crate) remove: bool,
}

/// Secret-free credential availability reported to the WebView.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderCredentialStatus {
    /// Stable remote-provider identity.
    pub(crate) provider_id: String,
    /// Whether the operating-system vault contains a key.
    pub(crate) configured: bool,
    /// Whether the credential is available in this process after authentication.
    pub(crate) unlocked: bool,
    /// Whether this platform requires biometric authentication for a locked credential.
    pub(crate) biometric_protected: bool,
}

/// Successful result of testing one draft provider endpoint.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderConnectionTest {
    /// Stable provider identity.
    pub(crate) provider_id: String,
    /// Normalized local or remote base URL.
    pub(crate) base_url: String,
    /// Number of models reported by the provider.
    pub(crate) model_count: usize,
    /// End-to-end connection-test duration in milliseconds.
    pub(crate) elapsed_ms: u64,
    /// User-readable connection summary.
    pub(crate) message: String,
}
