//! Kastellan settings validation and persistence tests.

use super::*;

fn temporary_config_path() -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("bottie-kastellan-config-{}", uuid::Uuid::new_v4()));
    directory.join(CONFIG_FILE)
}

#[test]
fn blank_drafts_mean_kastellan_on_this_machine() {
    let config = KastellanConfig::from_drafts(Some("  "), Some("")).unwrap();
    assert_eq!(config, KastellanConfig::default());
    assert_eq!(config.host(), DEFAULT_HOST);
    assert!(config.is_local());
    assert_eq!(
        config.settings(),
        KastellanSettings {
            host: DEFAULT_HOST.into(),
            cli_path: None,
            remote: false
        }
    );
}

#[test]
fn an_explicit_default_host_is_stored_as_the_default() {
    assert_eq!(
        KastellanConfig::from_drafts(Some(" localhost "), None)
            .unwrap()
            .host,
        None
    );
}

#[test]
fn a_remote_host_is_trimmed_and_reported_as_remote() {
    let config = KastellanConfig::from_drafts(Some(" dgx "), Some("~/bin/kastellan-cli")).unwrap();
    assert_eq!(config.host.as_deref(), Some("dgx"));
    assert!(config.settings().remote);
    assert_eq!(
        config.cli_path(),
        Some("~/bin/kastellan-cli"),
        "a remote path is the remote shell's to expand"
    );
}

#[test]
fn rejects_hosts_ssh_would_misread() {
    for host in ["-oProxyCommand=evil", "dgx host", "dg\nx", &"a".repeat(254)] {
        assert!(
            KastellanConfig::from_drafts(Some(host), None).is_err(),
            "{host:?} should be rejected"
        );
    }
    for host in ["dgx", "10.0.0.3", "user@dgx.example.org", "::1"] {
        assert!(
            KastellanConfig::from_drafts(Some(host), None).is_ok(),
            "{host:?} should be accepted"
        );
    }
}

#[test]
fn a_local_cli_path_must_be_absolute_and_free_of_control_characters() {
    assert!(KastellanConfig::from_drafts(None, Some("kastellan-cli")).is_err());
    assert!(
        KastellanConfig::from_drafts(None, Some("/opt/k/kastellan-cli\n")).is_ok(),
        "trailing newline trims"
    );
    assert!(KastellanConfig::from_drafts(None, Some("/opt/k\u{7}/kastellan-cli")).is_err());
    assert!(KastellanConfig::from_drafts(None, Some("/opt/k/kastellan-cli")).is_ok());
}

#[test]
fn saving_round_trips_and_leaves_no_staging_file() {
    let path = temporary_config_path();
    let config = KastellanConfig::from_drafts(Some("dgx"), None).unwrap();
    save_config(&path, &config).unwrap();
    assert_eq!(load_config(&path).unwrap(), config);
    let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(leftovers, [CONFIG_FILE], "no staging file may remain");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap().trim(),
        "{\n  \"host\": \"dgx\"\n}"
    );

    save_config(&path, &KastellanConfig::default()).unwrap();
    assert_eq!(
        load_config(&path).unwrap(),
        KastellanConfig::default(),
        "saving defaults replaces the host"
    );
}

#[test]
fn a_missing_file_means_defaults_and_a_bad_one_is_reported() {
    let path = temporary_config_path();
    assert_eq!(load_config(&path).unwrap(), KastellanConfig::default());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    for bad in [
        r#"{"hostname":"dgx"}"#,
        "{",
        r#"{"host":"-oProxyCommand=x"}"#,
    ] {
        std::fs::write(&path, bad).unwrap();
        assert!(load_config(&path).is_err(), "{bad} should be rejected");
    }
}

#[test]
fn localhost_is_recognised_in_any_case() {
    let config = KastellanConfig::from_drafts(Some("LocalHost"), None).unwrap();
    assert_eq!(config.host, None, "stored as the default");
    assert!(
        KastellanConfig {
            host: Some("LOCALHOST".into()),
            cli_path: None
        }
        .is_local()
    );
}
