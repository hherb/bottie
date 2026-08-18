use std::{fs, path::Path, time::Duration};

use serde::{Deserialize, Serialize};
use url::{Host, Url};

use super::ProviderError;

pub const DEFAULT_OMLX_BASE_URL: &str = "http://127.0.0.1:8000/";
pub const DEFAULT_OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434/";
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
pub const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);
pub const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(120);

/// Persisted configuration for local inference providers. Secrets are deliberately absent.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettings {
    pub omlx_base_url: String,
    pub ollama_base_url: String,
    #[serde(default)]
    pub last_provider_id: Option<String>,
    #[serde(default)]
    pub last_model_id: Option<String>,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            omlx_base_url: DEFAULT_OMLX_BASE_URL.into(),
            ollama_base_url: DEFAULT_OLLAMA_BASE_URL.into(),
            last_provider_id: None,
            last_model_id: None,
        }
    }
}

impl ProviderSettings {
    pub fn normalized(self) -> Result<Self, ProviderError> {
        Ok(Self {
            omlx_base_url: validate_local_base_url("oMLX", &self.omlx_base_url)?.to_string(),
            ollama_base_url: validate_local_base_url("Ollama", &self.ollama_base_url)?.to_string(),
            last_provider_id: normalize_provider_id(self.last_provider_id)?,
            last_model_id: normalize_model_id(self.last_model_id)?,
        })
    }
}

fn normalize_provider_id(value: Option<String>) -> Result<Option<String>, ProviderError> {
    match value.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some("omlx") => Ok(Some("omlx".into())),
        Some("ollama") => Ok(Some("ollama".into())),
        Some(_) => Err(ProviderError::invalid_request(
            "The remembered provider is not supported.",
        )),
    }
}

fn normalize_model_id(value: Option<String>) -> Result<Option<String>, ProviderError> {
    match value.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(model_id) if model_id.len() <= 512 => Ok(Some(model_id.into())),
        Some(_) => Err(ProviderError::invalid_request(
            "The remembered model identifier is too long.",
        )),
    }
}

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
            "{provider_name} must use a root HTTP loopback endpoint without credentials, query parameters, or fragments."
        )));
    }
    url.set_path("/");
    Ok(url)
}

pub fn load_provider_settings(path: &Path) -> Result<ProviderSettings, ProviderError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ProviderSettings::default());
        }
        Err(error) => {
            return Err(ProviderError::internal(
                "Could not read local provider settings.",
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
            "Could not encode local provider settings.",
            Some(error.to_string()),
        )
    })?;
    fs::write(path, bytes).map_err(|error| {
        ProviderError::internal(
            "Could not save local provider settings.",
            Some(error.to_string()),
        )
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
