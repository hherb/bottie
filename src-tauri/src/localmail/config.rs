//! Secret-free Localmail connector configuration persistence.

use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::inference::ProviderError;

use super::{normalize_certificate_sha256, normalize_origin};

/// Persisted Localmail endpoint and user-confirmed certificate pin.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LocalmailConfig {
    /// Normalized HTTPS origin without credentials or request-specific parts.
    pub(super) origin: String,
    /// Lowercase SHA-256 fingerprint of the explicitly confirmed leaf certificate.
    pub(super) certificate_sha256: String,
}

/// Reads a persisted secret-free connector configuration, if one exists.
pub(super) fn load_config(path: &Path) -> Result<Option<LocalmailConfig>, ProviderError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(config_error("read")),
    };
    let config: LocalmailConfig =
        serde_json::from_slice(&bytes).map_err(|_| config_error("read"))?;
    Ok(Some(LocalmailConfig {
        origin: normalize_origin(&config.origin)?,
        certificate_sha256: normalize_certificate_sha256(&config.certificate_sha256)?,
    }))
}

/// Persists only the normalized origin and certificate fingerprint.
pub(super) fn save_config(path: &Path, config: &LocalmailConfig) -> Result<(), ProviderError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| config_error("save"))?;
    }
    let bytes = serde_json::to_vec_pretty(config).map_err(|_| config_error("save"))?;
    fs::write(path, bytes).map_err(|_| config_error("save"))
}

/// Returns a fixed path-free configuration failure.
fn config_error(action: &str) -> ProviderError {
    ProviderError::internal(
        format!("Bottie could not {action} the Localmail connection settings."),
        None,
    )
}
