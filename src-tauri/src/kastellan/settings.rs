//! Settings commands: read, save, and test the Kastellan host without persisting a draft until Save.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use tauri::{AppHandle, Manager};

use super::{
    config::{CONFIG_FILE, KastellanConfig, KastellanSettings, load_config, save_config},
    transport::{Action, interpret_probe, plan_invocation, run_invocation},
};
use crate::inference::ProviderError;

/// Bound on one connection test: the ssh connect timeout plus a database round trip, with headroom.
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// Resolves `kastellan.json` inside the application config directory.
pub(crate) fn config_path(app: &AppHandle) -> Result<PathBuf, ProviderError> {
    app.path()
        .app_config_dir()
        .map(|directory| directory.join(CONFIG_FILE))
        .map_err(|error| {
            ProviderError::internal(
                "The settings folder is unavailable.",
                Some(error.to_string()),
            )
        })
}

/// Returns the local home folder used for the default CLI path.
pub(crate) fn home_directory() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Runs the side-effect-free probe for one configuration and describes the result.
pub(crate) async fn probe(
    config: &KastellanConfig,
    home: Option<&Path>,
) -> Result<String, ProviderError> {
    let invocation = plan_invocation(config, Action::Probe, home)?;
    let output = run_invocation(&invocation, PROBE_TIMEOUT).await?;
    interpret_probe(
        output.status.code(),
        !config.is_local(),
        config.host(),
        &output.stderr,
    )
}

#[tauri::command]
/// Returns the effective Kastellan settings, including defaults.
pub(crate) async fn get_kastellan_settings(
    app: AppHandle,
) -> Result<KastellanSettings, ProviderError> {
    Ok(load_config(&config_path(&app)?)?.settings())
}

#[tauri::command]
/// Validates and saves the Kastellan host and optional CLI path; blank values restore the defaults.
pub(crate) async fn update_kastellan_settings(
    app: AppHandle,
    host: Option<String>,
    cli_path: Option<String>,
) -> Result<KastellanSettings, ProviderError> {
    let config = KastellanConfig::from_drafts(host.as_deref(), cli_path.as_deref())?;
    save_config(&config_path(&app)?, &config)?;
    Ok(config.settings())
}

#[tauri::command]
/// Tests unsaved drafts by reading the Kastellan operator inbox; nothing is submitted and nothing is saved.
pub(crate) async fn test_kastellan_connection(
    host: Option<String>,
    cli_path: Option<String>,
) -> Result<String, ProviderError> {
    let config = KastellanConfig::from_drafts(host.as_deref(), cli_path.as_deref())?;
    probe(&config, home_directory().as_deref()).await
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn fake_cli(body: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("bottie-kastellan-probe-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("kastellan-cli");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn with_cli(cli: &Path) -> KastellanConfig {
        KastellanConfig::from_drafts(None, Some(&cli.to_string_lossy())).unwrap()
    }

    #[tokio::test]
    async fn a_reachable_kastellan_passes_the_probe() {
        let cli = fake_cli("exit 0");
        assert_eq!(
            probe(&with_cli(&cli), None).await.unwrap(),
            "Reached Kastellan on this computer."
        );
    }

    #[tokio::test]
    async fn an_unreachable_database_or_missing_cli_fails_the_probe() {
        let cli = fake_cli("echo 'inbox: db connect failed' >&2; exit 1");
        let error = probe(&with_cli(&cli), None).await.unwrap_err();
        assert!(
            error.message.contains("db connect failed"),
            "{}",
            error.message
        );
        let missing =
            KastellanConfig::from_drafts(None, Some("/nonexistent/kastellan-cli")).unwrap();
        assert!(probe(&missing, None).await.is_err());
    }
}
