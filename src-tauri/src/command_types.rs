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
#[serde(deny_unknown_fields, rename_all = "camelCase")]
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
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ProviderSelection {
    /// Stable provider identity.
    pub(crate) provider_id: String,
    /// Provider-owned model identity.
    pub(crate) model_id: String,
}

/// One native-provider credential update submitted to the native vault.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ProviderCredentialUpdate {
    /// Stable native-provider identity.
    pub(crate) provider_id: String,
    /// Replacement key, or `None` to retain the current key.
    pub(crate) api_key: Option<String>,
    /// Whether the existing vault entry should be deleted.
    pub(crate) remove: bool,
}

/// Secret-free native-provider credential availability reported to the WebView.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderCredentialStatus {
    /// Stable native-provider identity.
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

/// Draft credential used only for one fixed native web-search connection probe.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct WebSearchConnectionDraft {
    /// Stable web-search provider identity.
    pub(crate) provider_id: String,
    /// Optional unsaved API key used only for this connection test.
    pub(crate) api_key: Option<String>,
}

/// Successful result of one fixed native web-search provider connection test.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebSearchConnectionTest {
    /// Stable web-search provider identity.
    pub(crate) provider_id: String,
    /// End-to-end connection-test duration in milliseconds.
    pub(crate) elapsed_ms: u64,
    /// User-readable connection summary without provider results.
    pub(crate) message: String,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn credential_updates_reject_unknown_or_malformed_webview_fields() {
        let valid = serde_json::from_value::<ProviderCredentialUpdate>(json!({
            "providerId": "openai",
            "apiKey": null,
            "remove": false
        }))
        .expect("the existing credential update shape should remain valid");
        let unknown = serde_json::from_value::<ProviderCredentialUpdate>(json!({
            "providerId": "openai",
            "apiKey": "test-only-key",
            "remove": false,
            "filesystemPath": "/Users/alice/.secrets"
        }));
        let malformed = serde_json::from_value::<ProviderCredentialUpdate>(json!({
            "providerId": "openai",
            "apiKey": null,
            "remove": "yes"
        }));

        assert_eq!(valid.provider_id, "openai");
        assert_eq!(valid.api_key, None);
        assert!(!valid.remove);
        assert!(unknown.is_err());
        assert!(malformed.is_err());
    }

    #[test]
    fn provider_connection_drafts_reject_extra_native_authority() {
        let provider = serde_json::from_value::<ProviderConnectionDraft>(json!({
            "providerId": "openai",
            "baseUrl": "https://api.example/v1/",
            "apiKey": null,
            "databasePath": "/tmp/bottie.sqlite3"
        }));
        let search = serde_json::from_value::<WebSearchConnectionDraft>(json!({
            "providerId": "brave",
            "apiKey": null,
            "shellCommand": "open /tmp"
        }));
        let selection = serde_json::from_value::<ProviderSelection>(json!({
            "providerId": "ollama",
            "modelId": "qwen3:latest",
            "filesystemPath": "/tmp/model"
        }));

        assert!(provider.is_err());
        assert!(search.is_err());
        assert!(selection.is_err());
    }

    #[test]
    fn credential_status_serializes_only_secret_free_flags() {
        let status = ProviderCredentialStatus {
            provider_id: "openai".into(),
            configured: true,
            unlocked: false,
            biometric_protected: true,
        };

        assert_eq!(
            serde_json::to_value(status).expect("credential status should serialize"),
            json!({
                "providerId": "openai",
                "configured": true,
                "unlocked": false,
                "biometricProtected": true
            })
        );
    }
}
