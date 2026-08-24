use std::collections::HashSet;

use serde_json::json;

use super::*;

/// In-memory status fixture that panics if status collection tries to read secret values.
struct StatusCredentialStore {
    configured: HashSet<String>,
    unlocked: HashSet<String>,
    fail_status: bool,
}

impl CredentialStore for StatusCredentialStore {
    fn configured(&self, provider_id: &str) -> Result<bool, ProviderError> {
        if self.fail_status {
            return Err(ProviderError::internal(
                "leaky fixture failure",
                Some("token=test-secret path=/Users/alice/vault".into()),
            ));
        }
        Ok(self.configured.contains(provider_id))
    }

    fn unlocked(&self, provider_id: &str) -> Result<bool, ProviderError> {
        Ok(self.unlocked.contains(provider_id))
    }

    fn biometric_protected(&self) -> bool {
        true
    }

    fn get(&self, _provider_id: &str) -> Result<Option<String>, ProviderError> {
        panic!("credential status must not read a secret")
    }

    fn set(&self, _provider_id: &str, _api_key: &str) -> Result<(), ProviderError> {
        panic!("credential status must not write a secret")
    }

    fn delete(&self, _provider_id: &str) -> Result<(), ProviderError> {
        panic!("credential status must not delete a secret")
    }
}

#[test]
fn credential_accounts_are_limited_to_native_providers() {
    assert!(validate_native_credential_provider("openai").is_ok());
    assert!(validate_native_credential_provider("anthropic").is_ok());
    assert!(validate_native_credential_provider("brave").is_ok());
    assert!(validate_native_credential_provider("exa").is_ok());
    assert!(validate_native_credential_provider("localmail").is_ok());
    assert!(validate_native_credential_provider("ollama").is_err());
    assert!(validate_native_credential_provider("").is_err());
}

#[test]
fn only_existing_locked_credentials_require_authentication() {
    assert!(requires_authentication(true, false));
    assert!(!requires_authentication(false, false));
    assert!(!requires_authentication(true, true));
}

#[test]
fn warms_every_configured_credential_after_one_session_authentication() {
    let mut session = CredentialSession::default();
    let configured = HashSet::from(["openai", "brave", "localmail"]);
    let mut authentication_count = 0;
    let mut read_ids = Vec::new();

    warm_configured_credentials(
        &mut session,
        &NATIVE_SESSION_CREDENTIAL_IDS,
        |provider_id| Ok(configured.contains(provider_id)),
        || {
            authentication_count += 1;
            Ok(())
        },
        |provider_id| {
            read_ids.push(provider_id.to_owned());
            Ok(Some(format!("secret-{provider_id}")))
        },
    )
    .expect("configured credentials should warm the native session");

    assert_eq!(authentication_count, 1);
    assert_eq!(read_ids, ["openai", "brave", "localmail"]);
    assert_eq!(session.secrets.len(), 3);
    assert!(session.authenticated);

    warm_configured_credentials(
        &mut session,
        &NATIVE_SESSION_CREDENTIAL_IDS,
        |provider_id| Ok(configured.contains(provider_id)),
        || panic!("an authenticated session must not prompt again"),
        |_| panic!("cached credentials must not be read again"),
    )
    .expect("a warm session should be reusable");
}

#[test]
fn skips_session_authentication_when_no_credentials_are_configured() {
    let mut session = CredentialSession::default();

    warm_configured_credentials(
        &mut session,
        &NATIVE_SESSION_CREDENTIAL_IDS,
        |_| Ok(false),
        || panic!("an empty vault must not prompt"),
        |_| panic!("an empty vault must not be read"),
    )
    .expect("an empty vault should be a no-op");

    assert!(!session.authenticated);
    assert!(session.secrets.is_empty());
}

#[test]
fn reports_saved_and_absent_credentials_as_exact_secret_free_metadata() {
    let credentials = StatusCredentialStore {
        configured: HashSet::from(["openai".into(), "brave".into()]),
        unlocked: HashSet::from(["openai".into()]),
        fail_status: false,
    };

    let statuses = provider_credential_statuses(&credentials)
        .expect("credential status should remain metadata-only");

    assert_eq!(
        serde_json::to_value(statuses).expect("statuses should serialize"),
        json!([
            {
                "providerId": "openai",
                "configured": true,
                "unlocked": true,
                "biometricProtected": true
            },
            {
                "providerId": "anthropic",
                "configured": false,
                "unlocked": false,
                "biometricProtected": true
            },
            {
                "providerId": "brave",
                "configured": true,
                "unlocked": false,
                "biometricProtected": true
            },
            {
                "providerId": "exa",
                "configured": false,
                "unlocked": false,
                "biometricProtected": true
            }
        ])
    );
}

#[test]
fn redacts_credential_status_failures_before_ipc() {
    let credentials = StatusCredentialStore {
        configured: HashSet::new(),
        unlocked: HashSet::new(),
        fail_status: true,
    };

    let error = match provider_credential_statuses(&credentials) {
        Err(error) => error,
        Ok(_) => panic!("vault status failure should remain redacted"),
    };
    let serialized = serde_json::to_string(&error).expect("provider error should serialize");

    assert_eq!(error.code.as_str(), "internal");
    assert_eq!(
        error.message,
        "The operating-system credential vault could not report its status."
    );
    assert_eq!(error.diagnostic, None);
    assert!(!serialized.contains("test-secret"));
    assert!(!serialized.contains("/Users/alice"));
}

#[test]
fn rejects_unknown_status_identities_before_store_access() {
    let credentials = StatusCredentialStore {
        configured: HashSet::new(),
        unlocked: HashSet::new(),
        fail_status: true,
    };

    let error = match provider_credential_status(&credentials, "filesystem") {
        Err(error) => error,
        Ok(_) => panic!("unknown status identity should fail closed"),
    };

    assert_eq!(error.code.as_str(), "invalid_request");
    assert_eq!(error.diagnostic, None);
}
