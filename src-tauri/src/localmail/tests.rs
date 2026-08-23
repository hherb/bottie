//! Focused Localmail configuration and native-boundary tests.

use std::{collections::HashMap, fs, sync::Mutex};

use super::*;

#[derive(Default)]
struct TestCredentialStore {
    values: Mutex<HashMap<String, String>>,
}

impl CredentialStore for TestCredentialStore {
    fn configured(&self, provider_id: &str) -> Result<bool, ProviderError> {
        Ok(self
            .values
            .lock()
            .expect("credential lock")
            .contains_key(provider_id))
    }

    fn unlocked(&self, provider_id: &str) -> Result<bool, ProviderError> {
        self.configured(provider_id)
    }

    fn biometric_protected(&self) -> bool {
        true
    }

    fn get(&self, provider_id: &str) -> Result<Option<String>, ProviderError> {
        Ok(self
            .values
            .lock()
            .expect("credential lock")
            .get(provider_id)
            .cloned())
    }

    fn set(&self, provider_id: &str, api_key: &str) -> Result<(), ProviderError> {
        self.values
            .lock()
            .expect("credential lock")
            .insert(provider_id.into(), api_key.into());
        Ok(())
    }

    fn delete(&self, provider_id: &str) -> Result<(), ProviderError> {
        self.values
            .lock()
            .expect("credential lock")
            .remove(provider_id);
        Ok(())
    }
}

#[test]
fn localmail_origins_are_https_roots_without_embedded_request_data() {
    assert_eq!(
        normalize_origin(" https://mail.example:8443 ").expect("valid origin"),
        "https://mail.example:8443/"
    );
    for invalid in [
        "http://mail.example/",
        "https://user:secret@mail.example/",
        "https://mail.example/v1",
        "https://mail.example/?token=secret",
        "https://mail.example/#setup",
        "file:///tmp/localmail",
    ] {
        assert!(normalize_origin(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn fingerprints_and_bearer_tokens_are_closed_and_bounded() {
    assert_eq!(
        normalize_certificate_sha256(&"A".repeat(CERTIFICATE_SHA256_HEX_LENGTH))
            .expect("valid fingerprint"),
        "a".repeat(CERTIFICATE_SHA256_HEX_LENGTH)
    );
    assert!(normalize_certificate_sha256("abcd").is_err());
    assert_eq!(
        normalize_bearer_token(" token-value ").expect("token"),
        "token-value"
    );
    assert!(normalize_bearer_token("").is_err());
    assert!(normalize_bearer_token("line\nbreak").is_err());
    assert!(normalize_bearer_token(&"x".repeat(MAX_BEARER_TOKEN_LENGTH + 1)).is_err());
}

#[test]
fn persisted_connection_contains_no_bearer_token() {
    let directory = std::env::temp_dir().join(format!("bottie-localmail-{}", uuid::Uuid::new_v4()));
    let path = directory.join("localmail.json");
    let credentials = TestCredentialStore::default();
    let status = update_connection(
        &path,
        &credentials,
        LocalmailConnectionUpdate {
            origin: "https://mail.example:8443".into(),
            certificate_sha256: "b".repeat(CERTIFICATE_SHA256_HEX_LENGTH),
            bearer_token: Some("vault-only-token".into()),
            remove_token: false,
        },
    )
    .expect("connection should save");

    assert!(status.credential_configured);
    assert!(status.credential_unlocked);
    assert!(status.biometric_protected);
    let persisted = fs::read_to_string(&path).expect("persisted connector settings");
    assert!(persisted.contains("https://mail.example:8443/"));
    assert!(persisted.contains(&"b".repeat(CERTIFICATE_SHA256_HEX_LENGTH)));
    assert!(!persisted.contains("vault-only-token"));
    assert_eq!(
        credentials
            .get(LOCALMAIL_CREDENTIAL_ID)
            .expect("credential read")
            .as_deref(),
        Some("vault-only-token")
    );
    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn connection_status_survives_reopen_and_token_removal() {
    let directory = std::env::temp_dir().join(format!("bottie-localmail-{}", uuid::Uuid::new_v4()));
    let path = directory.join("localmail.json");
    let credentials = TestCredentialStore::default();
    update_connection(
        &path,
        &credentials,
        LocalmailConnectionUpdate {
            origin: "https://mail.example".into(),
            certificate_sha256: "c".repeat(CERTIFICATE_SHA256_HEX_LENGTH),
            bearer_token: Some("first-token".into()),
            remove_token: false,
        },
    )
    .expect("initial connection");
    let status = connection_status(&path, &credentials).expect("reopened status");
    assert_eq!(status.origin.as_deref(), Some("https://mail.example/"));
    assert!(status.credential_configured);

    let removed = update_connection(
        &path,
        &credentials,
        LocalmailConnectionUpdate {
            origin: status.origin.expect("origin"),
            certificate_sha256: status.certificate_sha256.expect("pin"),
            bearer_token: None,
            remove_token: true,
        },
    )
    .expect("token removal");
    assert!(!removed.credential_configured);
    assert!(load_config(&path).expect("config reload").is_some());
    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn replacement_and_removal_cannot_be_requested_together() {
    let directory = std::env::temp_dir().join(format!("bottie-localmail-{}", uuid::Uuid::new_v4()));
    let path = directory.join("localmail.json");
    let credentials = TestCredentialStore::default();
    let error = update_connection(
        &path,
        &credentials,
        LocalmailConnectionUpdate {
            origin: "https://mail.example".into(),
            certificate_sha256: "d".repeat(CERTIFICATE_SHA256_HEX_LENGTH),
            bearer_token: Some("new-token".into()),
            remove_token: true,
        },
    )
    .expect_err("conflicting credential update must fail");

    assert_eq!(
        error.message,
        "Choose either a replacement Localmail token or token removal."
    );
    assert!(!path.exists());
}

#[test]
#[ignore = "requires a live Localmail HTTPS server on its default loopback port"]
fn live_localmail_identity_probe_is_bounded_and_path_free() {
    let result = tauri::async_runtime::block_on(inspect_server("https://127.0.0.1:8443"))
        .expect("live Localmail identity probe");

    assert_eq!(result.origin, "https://127.0.0.1:8443/");
    assert_eq!(result.api_major, LOCALMAIL_API_MAJOR);
    assert!(!result.server_version.is_empty());
    assert_eq!(
        result.certificate_sha256.len(),
        CERTIFICATE_SHA256_HEX_LENGTH
    );
}
