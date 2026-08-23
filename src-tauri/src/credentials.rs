//! Operating-system credential-vault access for native provider API keys.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, mpsc},
    time::Duration,
};

use keyring::v1::{Entry, Error as KeyringError};

use crate::{command_types::ProviderCredentialStatus, inference::ProviderError};

const SERVICE_NAME: &str = "com.hherb.bottie.provider-api-keys";
const STATUS_SERVICE_NAME: &str = "com.hherb.bottie.provider-api-key-status";
const CONFIGURED_MARKER: &str = "configured";
const AUTHENTICATION_REASON: &str = "unlock cloud and connector credentials";
const AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(60);

/// Stable native provider identities allowed to own credential-vault entries.
pub(crate) const NATIVE_CREDENTIAL_IDS: [&str; 4] = ["openai", "anthropic", "brave", "exa"];
/// Vault identity reserved for the first-party Localmail connector token.
pub(crate) const LOCALMAIL_CREDENTIAL_ID: &str = "localmail";

/// Returns secret-free status for each WebView-visible credential without reading a vault value.
pub(crate) fn provider_credential_statuses(
    credentials: &dyn CredentialStore,
) -> Result<Vec<ProviderCredentialStatus>, ProviderError> {
    NATIVE_CREDENTIAL_IDS
        .into_iter()
        .map(|provider_id| provider_credential_status(credentials, provider_id))
        .collect()
}

/// Returns one path- and secret-free credential status for command responses.
pub(crate) fn provider_credential_status(
    credentials: &dyn CredentialStore,
    provider_id: &str,
) -> Result<ProviderCredentialStatus, ProviderError> {
    validate_native_credential_provider(provider_id)?;
    let configured = credentials
        .configured(provider_id)
        .map_err(|_| credential_status_error())?;
    let unlocked = credentials
        .unlocked(provider_id)
        .map_err(|_| credential_status_error())?;
    Ok(ProviderCredentialStatus {
        provider_id: provider_id.into(),
        configured,
        unlocked,
        biometric_protected: credentials.biometric_protected(),
    })
}

/// Maps vault-status failures without forwarding keyring, path, or credential detail.
fn credential_status_error() -> ProviderError {
    ProviderError::internal(
        "The operating-system credential vault could not report its status.",
        None,
    )
}

/// Narrow secret-store contract used by native provider orchestration.
pub(crate) trait CredentialStore: Send + Sync {
    /// Returns whether a provider has a saved credential without exposing it.
    fn configured(&self, provider_id: &str) -> Result<bool, ProviderError>;

    /// Returns whether a saved credential is unlocked in this process.
    fn unlocked(&self, provider_id: &str) -> Result<bool, ProviderError>;

    /// Returns whether this platform gates credential reads with biometrics.
    fn biometric_protected(&self) -> bool;

    /// Returns a provider API key after any required session authentication.
    fn get(&self, provider_id: &str) -> Result<Option<String>, ProviderError>;

    /// Replaces a provider API key in the operating-system vault.
    fn set(&self, provider_id: &str, api_key: &str) -> Result<(), ProviderError>;

    /// Removes a provider API key from the operating-system vault.
    fn delete(&self, provider_id: &str) -> Result<(), ProviderError>;
}

/// Credential store backed by the platform password manager and a session biometric gate.
#[derive(Default)]
pub(crate) struct SystemCredentialStore {
    cache: Mutex<HashMap<String, String>>,
}

impl SystemCredentialStore {
    /// Builds a native keyring entry without exposing its contents.
    fn entry(service: &str, provider_id: &str) -> Result<Entry, ProviderError> {
        validate_native_credential_provider(provider_id)?;
        Entry::new(service, provider_id).map_err(vault_error)
    }

    /// Returns the process-only cache after handling poisoned-lock failures safely.
    fn cache(&self) -> Result<MutexGuard<'_, HashMap<String, String>>, ProviderError> {
        self.cache.lock().map_err(|_| {
            ProviderError::internal("The credential session could not be accessed.", None)
        })
    }

    /// Migrates pre-biometric entries to a secret-free configured marker.
    fn configured_in_vault(provider_id: &str) -> Result<bool, ProviderError> {
        let marker = Self::entry(STATUS_SERVICE_NAME, provider_id)?;
        match marker.get_password() {
            Ok(_) => Ok(true),
            Err(KeyringError::NoEntry) => {
                let secret = Self::entry(SERVICE_NAME, provider_id)?;
                match secret.get_password() {
                    Ok(value) => {
                        drop(value);
                        marker
                            .set_password(CONFIGURED_MARKER)
                            .map_err(vault_error)?;
                        Ok(true)
                    }
                    Err(KeyringError::NoEntry) => Ok(false),
                    Err(error) => Err(vault_error(error)),
                }
            }
            Err(error) => Err(vault_error(error)),
        }
    }

    /// Reads one secret after the caller has satisfied the biometric policy.
    fn read_secret(provider_id: &str) -> Result<Option<String>, ProviderError> {
        match Self::entry(SERVICE_NAME, provider_id)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(KeyringError::NoEntry) => {
                let _ = Self::entry(STATUS_SERVICE_NAME, provider_id)?.delete_credential();
                Ok(None)
            }
            Err(error) => Err(vault_error(error)),
        }
    }
}

impl CredentialStore for SystemCredentialStore {
    fn configured(&self, provider_id: &str) -> Result<bool, ProviderError> {
        validate_native_credential_provider(provider_id)?;
        if self.cache()?.contains_key(provider_id) {
            return Ok(true);
        }
        Self::configured_in_vault(provider_id)
    }

    fn unlocked(&self, provider_id: &str) -> Result<bool, ProviderError> {
        validate_native_credential_provider(provider_id)?;
        Ok(self.cache()?.contains_key(provider_id))
    }

    fn biometric_protected(&self) -> bool {
        cfg!(target_os = "macos")
    }

    fn get(&self, provider_id: &str) -> Result<Option<String>, ProviderError> {
        validate_native_credential_provider(provider_id)?;
        if let Some(secret) = self.cache()?.get(provider_id).cloned() {
            return Ok(Some(secret));
        }
        if !Self::configured_in_vault(provider_id)? {
            return Ok(None);
        }
        authenticate_with_biometrics()?;
        let secret = Self::read_secret(provider_id)?;
        if let Some(value) = &secret {
            self.cache()?.insert(provider_id.into(), value.clone());
        }
        Ok(secret)
    }

    fn set(&self, provider_id: &str, api_key: &str) -> Result<(), ProviderError> {
        validate_native_credential_provider(provider_id)?;
        let api_key = api_key.trim();
        if api_key.is_empty() {
            return Err(ProviderError::invalid_request("API keys cannot be empty."));
        }
        let configured = Self::configured_in_vault(provider_id)?;
        let unlocked = self.cache()?.contains_key(provider_id);
        if requires_authentication(configured, unlocked) {
            authenticate_with_biometrics()?;
        }
        Self::entry(SERVICE_NAME, provider_id)?
            .set_password(api_key)
            .map_err(vault_error)?;
        Self::entry(STATUS_SERVICE_NAME, provider_id)?
            .set_password(CONFIGURED_MARKER)
            .map_err(vault_error)?;
        self.cache()?.insert(provider_id.into(), api_key.into());
        Ok(())
    }

    fn delete(&self, provider_id: &str) -> Result<(), ProviderError> {
        validate_native_credential_provider(provider_id)?;
        let configured = Self::configured_in_vault(provider_id)?;
        let unlocked = self.cache()?.contains_key(provider_id);
        if requires_authentication(configured, unlocked) {
            authenticate_with_biometrics()?;
        }
        delete_entry(SERVICE_NAME, provider_id)?;
        delete_entry(STATUS_SERVICE_NAME, provider_id)?;
        self.cache()?.remove(provider_id);
        Ok(())
    }
}

/// Returns whether an existing locked credential needs explicit authentication.
fn requires_authentication(configured: bool, unlocked: bool) -> bool {
    configured && !unlocked
}

/// Removes one vault entry while treating an already-absent value as success.
fn delete_entry(service: &str, provider_id: &str) -> Result<(), ProviderError> {
    match SystemCredentialStore::entry(service, provider_id)?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(error) => Err(vault_error(error)),
    }
}

/// Rejects local and unknown identities before they can address the credential vault.
fn validate_native_credential_provider(provider_id: &str) -> Result<(), ProviderError> {
    if NATIVE_CREDENTIAL_IDS.contains(&provider_id) || provider_id == LOCALMAIL_CREDENTIAL_ID {
        Ok(())
    } else {
        Err(ProviderError::invalid_request(
            "Choose a supported native provider credential.",
        ))
    }
}

#[cfg(target_os = "macos")]
/// Authenticates the device owner with Touch ID before reading an existing credential.
fn authenticate_with_biometrics() -> Result<(), ProviderError> {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSError, NSString};
    use objc2_local_authentication::{LAContext, LAPolicy};

    let policy = LAPolicy::DeviceOwnerAuthenticationWithBiometrics;
    let reason = NSString::from_str(AUTHENTICATION_REASON);
    let (sender, receiver) = mpsc::sync_channel(1);
    let reply = RcBlock::new(move |success: Bool, _error: *mut NSError| {
        let _ = sender.send(success.as_bool());
    });

    // SAFETY: `LAContext` and the immutable reason outlive the callback because this
    // function waits for its completion. The framework owns callback scheduling.
    let context = unsafe { LAContext::new() };
    // SAFETY: The policy is a framework-defined constant and the returned error is
    // consumed without dereferencing raw Objective-C pointers.
    unsafe {
        context
            .canEvaluatePolicy_error(policy)
            .map_err(|_| biometric_unavailable())?;
        context.evaluatePolicy_localizedReason_reply(policy, &reason, &reply);
    }

    match receiver.recv_timeout(AUTHENTICATION_TIMEOUT) {
        Ok(true) => Ok(()),
        Ok(false) | Err(_) => Err(biometric_error()),
    }
}

#[cfg(not(target_os = "macos"))]
/// Leaves platform credential-manager policy unchanged where biometric support is not implemented.
fn authenticate_with_biometrics() -> Result<(), ProviderError> {
    Ok(())
}

/// Maps unavailable biometric hardware or enrollment to a useful user action.
fn biometric_unavailable() -> ProviderError {
    ProviderError::invalid_request(
        "Touch ID is unavailable. Configure biometrics in System Settings before using cloud credentials.",
    )
}

/// Maps a cancelled or unsuccessful authentication without exposing framework diagnostics.
fn biometric_error() -> ProviderError {
    ProviderError::invalid_request("Touch ID did not unlock the cloud credential.")
}

/// Maps keyring failures without returning secret material or platform debug payloads.
fn vault_error(_error: KeyringError) -> ProviderError {
    ProviderError::internal(
        "The operating-system credential vault could not complete the request.",
        None,
    )
}

#[cfg(test)]
mod tests {
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
}
