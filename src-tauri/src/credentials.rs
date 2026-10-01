//! Operating-system credential-vault access for native provider API keys.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, mpsc},
    time::Duration,
};

use crate::{command_types::ProviderCredentialStatus, inference::ProviderError};

#[cfg(not(target_os = "macos"))]
use keyring::v1::{Entry, Error as KeyringError};

const SERVICE_NAME: &str = "com.hherb.bottie.provider-api-keys";
#[cfg(not(target_os = "macos"))]
const STATUS_SERVICE_NAME: &str = "com.hherb.bottie.provider-api-key-status";
#[cfg(not(target_os = "macos"))]
const CONFIGURED_MARKER: &str = "configured";
const AUTHENTICATION_REASON: &str =
    "unlock saved cloud, search, image, and connector credentials for this Bottie session";
const AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(60);

/// Stable native provider identities allowed to own credential-vault entries.
pub(crate) const NATIVE_CREDENTIAL_IDS: [&str; 5] = [
    "openai",
    "anthropic",
    "brave",
    "exa",
    crate::image_generation::QWEN_IMAGE_PROVIDER_ID,
];
/// Vault identity reserved for the first-party Localmail connector API key.
pub(crate) const LOCALMAIL_CREDENTIAL_ID: &str = "localmail";
/// Every credential Bottie warms after the single app-session authentication.
const NATIVE_SESSION_CREDENTIAL_IDS: [&str; 6] = [
    "openai",
    "anthropic",
    "brave",
    "exa",
    crate::image_generation::QWEN_IMAGE_PROVIDER_ID,
    LOCALMAIL_CREDENTIAL_ID,
];

/// Uses an explicit test draft without consulting or unlocking any saved credential.
pub(crate) fn draft_or_saved_credential<F>(
    draft: Option<String>,
    read_saved: F,
) -> Result<Option<String>, ProviderError>
where
    F: FnOnce() -> Result<Option<String>, ProviderError>,
{
    match draft.filter(|value| !value.trim().is_empty()) {
        Some(value) => Ok(Some(value)),
        None => read_saved(),
    }
}

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
    #[cfg(any(target_os = "macos", test))]
    bundle: Option<macos::CredentialBundle>,
}

#[derive(Default)]
pub(crate) struct SystemCredentialStore {
    session: Mutex<CredentialSession>,
}

impl SystemCredentialStore {
    /// Builds a native keyring entry without exposing its contents.
    #[cfg(not(target_os = "macos"))]
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
        #[cfg(target_os = "macos")]
        {
            let mut session = self.session()?;
            return self.warm_macos_session(&mut session);
        }
        #[cfg(not(target_os = "macos"))]
        let mut session = self.session()?;
        #[cfg(not(target_os = "macos"))]
        warm_configured_credentials(
            &mut session,
            &NATIVE_SESSION_CREDENTIAL_IDS,
            Self::configured_in_vault,
            authenticate_with_biometrics,
            Self::read_secret,
        )
    }

    #[cfg(target_os = "macos")]
    /// Loads all secrets from the single item under the existing session mutex.
    fn warm_macos_session(&self, session: &mut CredentialSession) -> Result<usize, ProviderError> {
        if session.bundle.is_some() {
            return Ok(0);
        }
        let configured = !macos::configured_ids()?.is_empty();
        if !configured {
            session.bundle = Some(macos::empty_bundle()?);
            return Ok(0);
        }
        macos::warm_bundle(
            session,
            configured,
            authenticate_with_biometrics,
            macos::load,
        )
    }

    #[cfg(target_os = "macos")]
    /// Commits one bundle replacement before updating the process-only cache.
    fn change_macos_credential(
        &self,
        provider_id: &str,
        api_key: Option<&str>,
    ) -> Result<(), ProviderError> {
        let mut session = self.session()?;
        self.warm_macos_session(&mut session)?;
        // Retiring legacy identities prevents deleted or replaced values from resurfacing.
        macos::change_bundle(&mut session, provider_id, api_key, macos::save)?;
        if api_key.is_none() {
            // A legacy item's ACL may reject deletion without another password prompt.
            // The committed retirement record already prevents it from being used again.
            let _ = macos::delete_legacy(provider_id);
        }
        Ok(())
    }

    /// Checks public macOS item attributes or the legacy marker on other platforms.
    fn configured_in_vault(provider_id: &str) -> Result<bool, ProviderError> {
        #[cfg(target_os = "macos")]
        {
            return Ok(macos::configured_ids()?.contains(provider_id));
        }
        #[cfg(not(target_os = "macos"))]
        let marker = Self::entry(STATUS_SERVICE_NAME, provider_id)?;
        #[cfg(not(target_os = "macos"))]
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
    #[cfg(not(target_os = "macos"))]
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
        #[cfg(target_os = "macos")]
        {
            self.warm_macos_session(&mut session)?;
            if !session.secrets.contains_key(provider_id) && Self::configured_in_vault(provider_id)?
            {
                return Err(macos::legacy_locked_error());
            }
        }
        #[cfg(not(target_os = "macos"))]
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
        #[cfg(target_os = "macos")]
        {
            return self.change_macos_credential(provider_id, Some(api_key));
        }
        #[cfg(not(target_os = "macos"))]
        {
            let mut session = self.session()?;
            let configured = Self::configured_in_vault(provider_id)?;
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
    }

    fn delete(&self, provider_id: &str) -> Result<(), ProviderError> {
        validate_native_credential_provider(provider_id)?;
        #[cfg(target_os = "macos")]
        {
            return self.change_macos_credential(provider_id, None);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let mut session = self.session()?;
            let configured = Self::configured_in_vault(provider_id)?;
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
}

/// Warms configured secrets while coalescing all reads behind one session authentication.
#[cfg(any(not(target_os = "macos"), test))]
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
#[cfg(any(not(target_os = "macos"), test))]
fn requires_authentication(configured: bool, authorized: bool) -> bool {
    configured && !authorized
}

/// Removes one vault entry while treating an already-absent value as success.
#[cfg(not(target_os = "macos"))]
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

    let policy = LAPolicy::DeviceOwnerAuthentication;
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
        "Device authentication is unavailable. Configure a login password or Touch ID in System Settings.",
    )
}

/// Maps a cancelled or unsuccessful authentication without exposing framework diagnostics.
fn biometric_error() -> ProviderError {
    ProviderError::invalid_request("Device authentication did not unlock the saved credentials.")
}

/// Maps keyring failures without returning secret material or platform debug payloads.
#[cfg(not(target_os = "macos"))]
fn vault_error(_error: KeyringError) -> ProviderError {
    ProviderError::internal(
        "The operating-system credential vault could not complete the request.",
        None,
    )
}

#[cfg(test)]
#[path = "credentials_tests.rs"]
mod tests;

#[cfg(any(target_os = "macos", test))]
#[path = "credentials_macos.rs"]
mod macos;
