//! Operator-chosen Kastellan transport settings, persisted as `kastellan.json` in the app config directory.
//!
//! The file is optional: absent means "Kastellan on this machine, CLI at its default install path". Every value is
//! validated with the same rules whether it arrives from the file, the Settings dialog, or a connection test, so a
//! hand-edited file can never produce an invocation the dialog would have refused.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::inference::ProviderError;

/// File name inside the application config directory.
pub(crate) const CONFIG_FILE: &str = "kastellan.json";
/// Host used when none is configured.
pub(crate) const DEFAULT_HOST: &str = "localhost";
/// `kastellan-cli` location relative to the home directory, as installed by `kastellan-cli install`.
pub(crate) const DEFAULT_CLI_RELATIVE_PATH: &str = ".local/bin/kastellan-cli";
/// The same default for a remote host, expanded by the remote shell.
pub(crate) const DEFAULT_REMOTE_CLI_PATH: &str = "\"$HOME/.local/bin/kastellan-cli\"";
/// Longest accepted host: the DNS name limit, which also bounds ssh aliases sensibly.
const MAX_HOST_CHARACTERS: usize = 253;
/// Longest accepted CLI path.
const MAX_CLI_PATH_CHARACTERS: usize = 1024;

/// Persisted settings; an absent field means its default.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct KastellanConfig {
    /// Host running Kastellan: loopback runs the CLI directly, anything else is an `ssh` destination.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) host: Option<String>,
    /// CLI path. Locally an absolute path; remotely interpreted by the remote shell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cli_path: Option<String>,
}

/// Secret-free settings shown in the Settings dialog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KastellanSettings {
    /// Effective host, including the default.
    pub(crate) host: String,
    /// Explicit CLI path, when one is configured.
    pub(crate) cli_path: Option<String>,
    /// Whether requests travel over `ssh`.
    pub(crate) remote: bool,
}

impl KastellanConfig {
    /// Builds validated settings from dialog drafts; blank fields fall back to their defaults.
    pub(crate) fn from_drafts(
        host: Option<&str>,
        cli_path: Option<&str>,
    ) -> Result<Self, ProviderError> {
        let host = non_blank(host).filter(|host| !host.eq_ignore_ascii_case(DEFAULT_HOST));
        let config = Self {
            host: host.map(str::to_owned),
            cli_path: non_blank(cli_path).map(str::to_owned),
        };
        config.validate()?;
        Ok(config)
    }

    /// Returns the configured host, defaulting to the local machine.
    pub(crate) fn host(&self) -> &str {
        non_blank(self.host.as_deref()).unwrap_or(DEFAULT_HOST)
    }

    /// Returns the explicit CLI path, if any.
    pub(crate) fn cli_path(&self) -> Option<&str> {
        non_blank(self.cli_path.as_deref())
    }

    /// Whether requests run the CLI on this machine rather than over `ssh`.
    pub(crate) fn is_local(&self) -> bool {
        is_loopback_host(self.host())
    }

    /// Checks every value against the rules the transport depends on.
    pub(crate) fn validate(&self) -> Result<(), ProviderError> {
        validate_host(self.host())?;
        if let Some(path) = self.cli_path() {
            validate_cli_path(path, self.is_local())?;
        }
        Ok(())
    }

    /// Presents the effective settings.
    pub(crate) fn settings(&self) -> KastellanSettings {
        KastellanSettings {
            host: self.host().to_owned(),
            cli_path: self.cli_path().map(str::to_owned),
            remote: !self.is_local(),
        }
    }
}

/// Reports whether a host names this machine, so the CLI runs without `ssh`.
pub(crate) fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost") || matches!(host, "127.0.0.1" | "::1")
}

/// Rejects hosts that `ssh` would parse as options or that cannot be a single destination argument.
pub(crate) fn validate_host(host: &str) -> Result<(), ProviderError> {
    if host.chars().count() > MAX_HOST_CHARACTERS {
        return Err(ProviderError::invalid_request(
            "The Kastellan host is too long.",
        ));
    }
    if host.starts_with('-') || host.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(ProviderError::invalid_request(
            "The Kastellan host must be a host name, address, or ssh alias without spaces.",
        ));
    }
    Ok(())
}

/// Rejects CLI paths that could break the command line; a local path must be absolute because the
/// desktop app's `PATH` is not the shell's.
pub(crate) fn validate_cli_path(path: &str, local: bool) -> Result<(), ProviderError> {
    if path.chars().count() > MAX_CLI_PATH_CHARACTERS || path.chars().any(char::is_control) {
        return Err(ProviderError::invalid_request(
            "The Kastellan CLI path is too long or contains control characters.",
        ));
    }
    if local && !Path::new(path).is_absolute() {
        return Err(ProviderError::invalid_request(
            "Use an absolute path for kastellan-cli on this computer, such as the one `which kastellan-cli` prints.",
        ));
    }
    Ok(())
}

/// Reads the optional config; a missing file means defaults, a malformed or invalid one is an error.
pub(crate) fn load_config(path: &Path) -> Result<KastellanConfig, ProviderError> {
    let config: KastellanConfig = match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
            ProviderError::invalid_request(format!("kastellan.json is invalid: {error}"))
        })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => KastellanConfig::default(),
        Err(error) => {
            return Err(ProviderError::internal(
                "kastellan.json could not be read.",
                Some(error.to_string()),
            ));
        }
    };
    config.validate()?;
    Ok(config)
}

/// Replaces the config atomically so a crash never leaves a truncated file behind.
pub(crate) fn save_config(path: &Path, config: &KastellanConfig) -> Result<(), ProviderError> {
    let failed = |error: std::io::Error| {
        ProviderError::internal(
            "Kastellan settings could not be saved.",
            Some(error.to_string()),
        )
    };
    config.validate()?;
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory).map_err(failed)?;
    }
    let bytes = serde_json::to_vec_pretty(config).map_err(|error| {
        ProviderError::internal(
            "Kastellan settings could not be encoded.",
            Some(error.to_string()),
        )
    })?;
    let staged = staging_path(path);
    let written = write_synced(&staged, &bytes).and_then(|()| std::fs::rename(&staged, path));
    written.map_err(|error| {
        let _ = std::fs::remove_file(&staged);
        failed(error)
    })
}

/// Writes and flushes one file so a crash after the rename cannot leave it empty.
fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Unique sibling file the new config is written to before it replaces the old one.
fn staging_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.tmp", uuid::Uuid::new_v4()));
    path.with_file_name(name)
}

/// Returns the trimmed value when it is not blank.
fn non_blank(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
