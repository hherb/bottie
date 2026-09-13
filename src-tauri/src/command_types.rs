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

/// Candidate Qwen Image setup checked without issuing a billable generation request.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ImageGenerationSetupDraft {
    /// Credential-free DashScope or Model Studio workspace API root.
    pub(crate) base_url: String,
    /// Optional unsaved Model Studio API key used only for this validation.
    pub(crate) api_key: Option<String>,
}

/// Exact image-provider capability status safe to return to the WebView.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageGenerationSetupStatus {
    /// Stable Bottie image-provider identity.
    pub(crate) provider_id: &'static str,
    /// Exact provider-owned checkpoint identity.
    pub(crate) model_id: &'static str,
    /// Normalized credential-free API root.
    pub(crate) base_url: String,
    /// Whether this concrete adapter executes locally or through a cloud service.
    pub(crate) execution: &'static str,
    /// Whether text-to-image generation is implemented.
    pub(crate) generation: bool,
    /// Whether the selected model supports instruction-based image editing.
    pub(crate) editing: bool,
    /// Maximum output count accepted by one request.
    pub(crate) max_outputs: u8,
    /// Maximum output pixel area accepted by the provider.
    pub(crate) max_pixels: u64,
    /// User-readable validation result that states no billable request was made.
    pub(crate) message: &'static str,
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
        let image = serde_json::from_value::<ImageGenerationSetupDraft>(json!({
            "baseUrl": "https://dashscope-intl.aliyuncs.com/api/v1/",
            "apiKey": null,
            "outputPath": "/tmp/generated.png"
        }));

        assert!(provider.is_err());
        assert!(search.is_err());
        assert!(selection.is_err());
        assert!(image.is_err());
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

    #[test]
    fn image_setup_status_is_exact_and_path_free() {
        let status = ImageGenerationSetupStatus {
            provider_id: "qwen-image",
            model_id: "qwen-image-2.0-2026-03-03",
            base_url: "https://workspace.ap-southeast-1.maas.aliyuncs.com/api/v1/".into(),
            execution: "cloud",
            generation: true,
            editing: true,
            max_outputs: 6,
            max_pixels: 4_194_304,
            message: "Qwen Image setup is structurally valid; no billed request was sent.",
        };

        let value = serde_json::to_value(status).expect("status should serialize");
        assert_eq!(value["modelId"], "qwen-image-2.0-2026-03-03");
        assert_eq!(value["execution"], "cloud");
        assert!(value.get("apiKey").is_none());
        assert!(value.get("filesystemPath").is_none());
    }
}
