use std::{fs, path::Path, time::Duration};

use serde::{Deserialize, Serialize};
use url::{Host, Url};

use super::ProviderError;

/// Built-in loopback root for oMLX.
pub const DEFAULT_OMLX_BASE_URL: &str = "http://127.0.0.1:8000/";
/// Built-in loopback root for Ollama.
pub const DEFAULT_OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434/";
/// Built-in OpenAI API root.
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1/";
/// Built-in Anthropic API root.
pub const DEFAULT_ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com/v1/";
/// Built-in fixed web-search provider retained for existing settings files.
pub const DEFAULT_WEB_SEARCH_PROVIDER_ID: &str = "brave";
/// Maximum time allowed to establish a provider connection.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// Maximum time allowed for model discovery and connection tests.
pub const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);
/// Maximum idle period while waiting for the next streaming response chunk.
pub const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(120);

/// Persisted provider configuration. Secrets are deliberately absent.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettings {
    /// Normalized oMLX loopback root.
    pub omlx_base_url: String,
    /// Normalized Ollama loopback root.
    pub ollama_base_url: String,
    /// Normalized OpenAI-compatible HTTPS API root.
    #[serde(default = "default_openai_base_url")]
    pub openai_base_url: String,
    /// Normalized Anthropic-compatible HTTPS API root.
    #[serde(default = "default_anthropic_base_url")]
    pub anthropic_base_url: String,
    /// Fixed native search adapter used by explicitly enabled Web calls.
    #[serde(default = "default_web_search_provider_id")]
    pub web_search_provider_id: String,
    #[serde(default)]
    /// Last successfully selected provider.
    pub last_provider_id: Option<String>,
    #[serde(default)]
    /// Last successfully selected provider-owned model.
    pub last_model_id: Option<String>,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            omlx_base_url: DEFAULT_OMLX_BASE_URL.into(),
            ollama_base_url: DEFAULT_OLLAMA_BASE_URL.into(),
            openai_base_url: DEFAULT_OPENAI_BASE_URL.into(),
            anthropic_base_url: DEFAULT_ANTHROPIC_BASE_URL.into(),
            web_search_provider_id: DEFAULT_WEB_SEARCH_PROVIDER_ID.into(),
            last_provider_id: None,
            last_model_id: None,
        }
    }
}

impl ProviderSettings {
    /// Validates and normalizes every persisted setting.
    pub fn normalized(self) -> Result<Self, ProviderError> {
        Ok(Self {
            omlx_base_url: validate_local_base_url("oMLX", &self.omlx_base_url)?.to_string(),
            ollama_base_url: validate_local_base_url("Ollama", &self.ollama_base_url)?.to_string(),
            openai_base_url: validate_remote_base_url("OpenAI-compatible", &self.openai_base_url)?
                .to_string(),
            anthropic_base_url: validate_remote_base_url(
                "Anthropic-compatible",
                &self.anthropic_base_url,
            )?
            .to_string(),
            web_search_provider_id: normalize_web_search_provider_id(&self.web_search_provider_id)?,
            last_provider_id: normalize_provider_id(self.last_provider_id)?,
            last_model_id: normalize_model_id(self.last_model_id)?,
        })
    }
}

/// Accepts only Bottie's fixed native web-search adapters.
fn normalize_web_search_provider_id(value: &str) -> Result<String, ProviderError> {
    match value.trim() {
        "brave" => Ok("brave".into()),
        "exa" => Ok("exa".into()),
        _ => Err(ProviderError::invalid_request(
            "Choose a supported web search engine.",
        )),
    }
}

/// Normalizes an optional remembered provider identity.
fn normalize_provider_id(value: Option<String>) -> Result<Option<String>, ProviderError> {
    match value.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some("omlx") => Ok(Some("omlx".into())),
        Some("ollama") => Ok(Some("ollama".into())),
        Some("openai") => Ok(Some("openai".into())),
        Some("anthropic") => Ok(Some("anthropic".into())),
        Some(_) => Err(ProviderError::invalid_request(
            "The remembered provider is not supported.",
        )),
    }
}

/// Validates a credential-free HTTPS API root for a remote compatible provider.
pub fn validate_remote_base_url(
    provider_name: &str,
    candidate: &str,
) -> Result<Url, ProviderError> {
    let mut url = Url::parse(candidate.trim()).map_err(|_| {
        ProviderError::invalid_request(format!(
            "Enter a complete {provider_name} API root such as https://api.example.com/v1/."
        ))
    })?;
    let has_credentials = !url.username().is_empty() || url.password().is_some();
    if url.scheme() != "https"
        || url.host().is_none()
        || has_credentials
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(ProviderError::invalid_request(format!(
            "{provider_name} must use an HTTPS API root without credentials, query parameters, or fragments."
        )));
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

fn default_openai_base_url() -> String {
    DEFAULT_OPENAI_BASE_URL.into()
}

fn default_anthropic_base_url() -> String {
    DEFAULT_ANTHROPIC_BASE_URL.into()
}

fn default_web_search_provider_id() -> String {
    DEFAULT_WEB_SEARCH_PROVIDER_ID.into()
}

/// Normalizes and bounds an optional remembered model identity.
fn normalize_model_id(value: Option<String>) -> Result<Option<String>, ProviderError> {
    match value.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(model_id) if model_id.len() <= 512 => Ok(Some(model_id.into())),
        Some(_) => Err(ProviderError::invalid_request(
            "The remembered model identifier is too long.",
        )),
    }
}

/// Validates a root HTTP(S) loopback endpoint and returns its normalized URL.
pub fn validate_local_base_url(provider_name: &str, candidate: &str) -> Result<Url, ProviderError> {
    let mut url = Url::parse(candidate.trim()).map_err(|_| {
        ProviderError::invalid_request(format!(
            "Enter a complete {provider_name} endpoint such as http://127.0.0.1:8000/."
        ))
    })?;
    let is_http = matches!(url.scheme(), "http" | "https");
    let is_loopback = match url.host() {
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        None => false,
    };
    let has_credentials = !url.username().is_empty() || url.password().is_some();
    let has_extra_parts =
        url.query().is_some() || url.fragment().is_some() || !matches!(url.path(), "" | "/");
    if !is_http || !is_loopback || has_credentials || has_extra_parts {
        return Err(ProviderError::invalid_request(format!(
            concat!(
                "{provider_name} must use a root HTTP loopback endpoint without credentials, ",
                "query parameters, or fragments."
            ),
            provider_name = provider_name,
        )));
    }
    url.set_path("/");
    Ok(url)
}

/// Loads and validates provider settings, returning defaults when the file does not exist.
pub fn load_provider_settings(path: &Path) -> Result<ProviderSettings, ProviderError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ProviderSettings::default());
        }
        Err(error) => {
            return Err(ProviderError::internal(
                "Could not read provider settings.",
                Some(error.to_string()),
            ));
        }
    };
    serde_json::from_slice::<ProviderSettings>(&bytes)
        .map_err(|error| {
            ProviderError::internal(
                "Local provider settings are malformed; defaults will be used.",
                Some(error.to_string()),
            )
        })?
        .normalized()
}

/// Persists non-secret provider settings as formatted JSON.
pub fn save_provider_settings(
    path: &Path,
    settings: &ProviderSettings,
) -> Result<(), ProviderError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ProviderError::internal(
                "Could not create the local settings directory.",
                Some(error.to_string()),
            )
        })?;
    }
    let bytes = serde_json::to_vec_pretty(settings).map_err(|error| {
        ProviderError::internal(
            "Could not encode provider settings.",
            Some(error.to_string()),
        )
    })?;
    fs::write(path, bytes).map_err(|error| {
        ProviderError::internal("Could not save provider settings.", Some(error.to_string()))
    })
}

/// Removes credential-shaped values before a diagnostic crosses into UI state.
pub fn redact_diagnostic(value: &str) -> String {
    let mut redacted = value.to_owned();
    for marker in ["api_key=", "apikey=", "token=", "access_token="] {
        redacted = redact_after_marker(&redacted, marker);
    }
    redact_bearer_tokens(&redacted)
}

/// Redacts the value following one case-insensitive credential marker.
fn redact_after_marker(value: &str, marker: &str) -> String {
    let mut result = value.to_owned();
    let mut search_from = 0;
    loop {
        let lower = result[search_from..].to_ascii_lowercase();
        let Some(relative_start) = lower.find(marker) else {
            break;
        };
        let value_start = search_from + relative_start + marker.len();
        let value_end = result[value_start..]
            .find(['&', ' ', '\n', '\r'])
            .map(|offset| value_start + offset)
            .unwrap_or(result.len());
        result.replace_range(value_start..value_end, "[redacted]");
        search_from = value_start + "[redacted]".len();
    }
    result
}

/// Redacts bearer credential values without exposing them to the WebView.
fn redact_bearer_tokens(value: &str) -> String {
    let marker = "bearer ";
    let mut result = value.to_owned();
    let mut search_from = 0;
    loop {
        let lower = result[search_from..].to_ascii_lowercase();
        let Some(relative_start) = lower.find(marker) else {
            break;
        };
        let value_start = search_from + relative_start + marker.len();
        let value_end = result[value_start..]
            .find([' ', '\n', '\r', ','])
            .map(|offset| value_start + offset)
            .unwrap_or(result.len());
        result.replace_range(value_start..value_end, "[redacted]");
        search_from = value_start + "[redacted]".len();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_and_normalizes_root_loopback_endpoints() {
        assert_eq!(
            validate_local_base_url("oMLX", "http://localhost:8123")
                .unwrap()
                .as_str(),
            "http://localhost:8123/"
        );
        assert!(validate_local_base_url("oMLX", "http://[::1]:8000/").is_ok());
    }

    #[test]
    fn rejects_non_loopback_and_credential_shaped_endpoints() {
        for endpoint in [
            "https://example.com/",
            "file:///tmp/models",
            "http://user:secret@127.0.0.1:8000/",
            "http://127.0.0.1:8000/v1",
            "http://127.0.0.1:8000/?token=secret",
            "http://127.0.0.1:8000/#models",
        ] {
            assert!(
                validate_local_base_url("oMLX", endpoint).is_err(),
                "{endpoint}"
            );
        }
    }

    #[test]
    fn accepts_https_compatible_roots_and_rejects_unsafe_remote_urls() {
        assert_eq!(
            validate_remote_base_url("OpenAI-compatible", "https://gateway.example/v1")
                .unwrap()
                .as_str(),
            "https://gateway.example/v1/"
        );
        for endpoint in [
            "http://api.example.com/v1/",
            "https://user:secret@api.example.com/v1/",
            "https://api.example.com/v1/?token=secret",
            "file:///tmp/provider",
        ] {
            assert!(
                validate_remote_base_url("remote", endpoint).is_err(),
                "{endpoint}"
            );
        }
    }

    #[test]
    fn redacts_secret_shaped_diagnostics() {
        let diagnostic = "request token=alpha&next=1 Authorization: Bearer beta api_key=gamma";
        let redacted = redact_diagnostic(diagnostic);
        assert!(!redacted.contains("alpha"));
        assert!(!redacted.contains("beta"));
        assert!(!redacted.contains("gamma"));
        assert!(redacted.matches("[redacted]").count() >= 3);
    }

    #[test]
    fn settings_round_trip_without_secrets() {
        let directory =
            std::env::temp_dir().join(format!("bottie-settings-{}", uuid::Uuid::new_v4()));
        let path = directory.join("providers.json");
        let settings = ProviderSettings::default();
        save_provider_settings(&path, &settings).unwrap();
        assert_eq!(load_provider_settings(&path).unwrap(), settings);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn older_settings_files_load_without_a_remembered_selection() {
        let settings: ProviderSettings = serde_json::from_str(
            r#"{"omlxBaseUrl":"http://127.0.0.1:8000/","ollamaBaseUrl":"http://127.0.0.1:11434/"}"#,
        )
        .unwrap();
        assert_eq!(settings.last_provider_id, None);
        assert_eq!(settings.last_model_id, None);
        assert_eq!(settings.openai_base_url, DEFAULT_OPENAI_BASE_URL);
        assert_eq!(settings.anthropic_base_url, DEFAULT_ANTHROPIC_BASE_URL);
        assert_eq!(settings.web_search_provider_id, "brave");
    }

    #[test]
    fn accepts_only_fixed_web_search_provider_identities() {
        let exa = ProviderSettings {
            web_search_provider_id: "exa".into(),
            ..ProviderSettings::default()
        };
        assert_eq!(exa.normalized().unwrap().web_search_provider_id, "exa");

        let unsupported = ProviderSettings {
            web_search_provider_id: "custom".into(),
            ..ProviderSettings::default()
        };
        assert!(unsupported.normalized().is_err());
    }

    #[test]
    fn validates_a_remembered_provider_and_model_pair() {
        let settings = ProviderSettings {
            last_provider_id: Some("ollama".into()),
            last_model_id: Some("qwen3:latest".into()),
            ..ProviderSettings::default()
        }
        .normalized()
        .unwrap();
        assert_eq!(settings.last_provider_id.as_deref(), Some("ollama"));
        assert_eq!(settings.last_model_id.as_deref(), Some("qwen3:latest"));

        let invalid = ProviderSettings {
            last_provider_id: Some("remote".into()),
            ..ProviderSettings::default()
        };
        assert!(invalid.normalized().is_err());
    }
}
