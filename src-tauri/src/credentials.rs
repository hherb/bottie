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
const AUTHENTICATION_REASON: &str =
    "unlock saved cloud, search, and connector credentials for this Bottie session";
const AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(60);

/// Stable native provider identities allowed to own credential-vault entries.
pub(crate) const NATIVE_CREDENTIAL_IDS: [&str; 4] = ["openai", "anthropic", "brave", "exa"];
/// Vault identity reserved for the first-party Localmail connector token.
pub(crate) const LOCALMAIL_CREDENTIAL_ID: &str = "localmail";
/// Every credential Bottie warms after the single app-session authentication.
const NATIVE_SESSION_CREDENTIAL_IDS: [&str; 5] = [
    "openai",
    "anthropic",
    "brave",
    "exa",
    LOCALMAIL_CREDENTIAL_ID,
];

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
struct CredentialSession {
    secrets: HashMap<String, String>,
    authenticated: bool,
}

#[derive(Default)]
pub(crate) struct SystemCredentialStore {
    session: Mutex<CredentialSession>,
}

impl SystemCredentialStore {
    /// Builds a native keyring entry without exposing its contents.
    fn entry(service: &str, provider_id: &str) -> Result<Entry, ProviderError> {
        validate_native_credential_provider(provider_id)?;
        Entry::new(service, provider_id).map_err(vault_error)
    }

    /// Returns the process-only cache after handling poisoned-lock failures safely.
    fn session(&self) -> Result<MutexGuard<'_, CredentialSession>, ProviderError> {
        self.session.lock().map_err(|_| {
            ProviderError::internal("The credential session could not be accessed.", None)
        })
    }

    /// Authenticates once and warms every configured credential into process-only memory.
    pub(crate) fn warm_session(&self) -> Result<usize, ProviderError> {
        let mut session = self.session()?;
        warm_configured_credentials(
            &mut session,
            &NATIVE_SESSION_CREDENTIAL_IDS,
            Self::configured_in_vault,
            authenticate_with_biometrics,
            Self::read_secret,
        )
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
        if self.session()?.secrets.contains_key(provider_id) {
            return Ok(true);
        }
        Self::configured_in_vault(provider_id)
    }

    fn unlocked(&self, provider_id: &str) -> Result<bool, ProviderError> {
        validate_native_credential_provider(provider_id)?;
        Ok(self.session()?.secrets.contains_key(provider_id))
    }

    fn biometric_protected(&self) -> bool {
        cfg!(target_os = "macos")
    }

    fn get(&self, provider_id: &str) -> Result<Option<String>, ProviderError> {
        validate_native_credential_provider(provider_id)?;
        let mut session = self.session()?;
        if let Some(secret) = session.secrets.get(provider_id).cloned() {
            return Ok(Some(secret));
        }
        warm_configured_credentials(
            &mut session,
            &[provider_id],
            Self::configured_in_vault,
            authenticate_with_biometrics,
            Self::read_secret,
        )?;
        Ok(session.secrets.get(provider_id).cloned())
    }

    fn set(&self, provider_id: &str, api_key: &str) -> Result<(), ProviderError> {
        validate_native_credential_provider(provider_id)?;
        let api_key = api_key.trim();
        if api_key.is_empty() {
            return Err(ProviderError::invalid_request("API keys cannot be empty."));
        }
        let configured = Self::configured_in_vault(provider_id)?;
        let mut session = self.session()?;
        let authorized = session.authenticated || session.secrets.contains_key(provider_id);
        if requires_authentication(configured, authorized) {
            authenticate_with_biometrics()?;
            session.authenticated = true;
        }
        Self::entry(SERVICE_NAME, provider_id)?
            .set_password(api_key)
            .map_err(vault_error)?;
        Self::entry(STATUS_SERVICE_NAME, provider_id)?
            .set_password(CONFIGURED_MARKER)
            .map_err(vault_error)?;
        session.secrets.insert(provider_id.into(), api_key.into());
        Ok(())
    }

    fn delete(&self, provider_id: &str) -> Result<(), ProviderError> {
        validate_native_credential_provider(provider_id)?;
        let configured = Self::configured_in_vault(provider_id)?;
        let mut session = self.session()?;
        let authorized = session.authenticated || session.secrets.contains_key(provider_id);
        if requires_authentication(configured, authorized) {
            authenticate_with_biometrics()?;
            session.authenticated = true;
        }
        delete_entry(SERVICE_NAME, provider_id)?;
        delete_entry(STATUS_SERVICE_NAME, provider_id)?;
        session.secrets.remove(provider_id);
        Ok(())
    }
}

/// Warms configured secrets while coalescing all reads behind one session authentication.
fn warm_configured_credentials<C, A, R>(
    session: &mut CredentialSession,
    provider_ids: &[&str],
    mut configured: C,
    mut authenticate: A,
    mut read_secret: R,
) -> Result<usize, ProviderError>
where
    C: FnMut(&str) -> Result<bool, ProviderError>,
    A: FnMut() -> Result<(), ProviderError>,
    R: FnMut(&str) -> Result<Option<String>, ProviderError>,
{
    let configured_ids = provider_ids
        .iter()
        .copied()
        .filter(|provider_id| !session.secrets.contains_key(*provider_id))
        .map(|provider_id| configured(provider_id).map(|saved| (provider_id, saved)))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter_map(|(provider_id, saved)| saved.then_some(provider_id))
        .collect::<Vec<_>>();
    if configured_ids.is_empty() {
        return Ok(0);
    }
    if !session.authenticated {
        authenticate()?;
        session.authenticated = true;
    }
    let mut warmed = 0;
    for provider_id in configured_ids {
        if let Some(secret) = read_secret(provider_id)? {
            session.secrets.insert(provider_id.into(), secret);
            warmed += 1;
        }
    }
    Ok(warmed)
}

/// Returns whether an existing locked credential needs explicit authentication.
fn requires_authentication(configured: bool, authorized: bool) -> bool {
    configured && !authorized
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
#[path = "credentials_tests.rs"]
mod tests;
