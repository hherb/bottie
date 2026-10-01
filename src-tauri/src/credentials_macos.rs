//! One macOS keychain item for the entire native credential session.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::{CredentialSession, NATIVE_SESSION_CREDENTIAL_IDS};
use crate::inference::ProviderError;

const BUNDLE_SERVICE: &str = "com.hherb.bottie.provider-credential-bundle-v2";
const BUNDLE_ACCOUNT: &str = "native-session";
const MAX_BUNDLE_BYTES: usize = 64 * 1_024;

/// Secret payload and legacy retirement records, confined to native memory and the OS vault.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CredentialBundle {
    pub(super) secrets: HashMap<String, String>,
    pub(super) retired_legacy_ids: HashSet<String>,
}

/// Public item attributes used for status without retrieving any password bytes.
#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CredentialIndex {
    configured_ids: HashSet<String>,
    retired_legacy_ids: HashSet<String>,
}

/// Validates public metadata before it can decide whether authentication or legacy migration is needed.
fn decode_index(value: &str) -> Result<CredentialIndex, ProviderError> {
    if value.len() > MAX_BUNDLE_BYTES {
        return Err(bundle_error());
    }
    let index: CredentialIndex = serde_json::from_str(value).map_err(|_| bundle_error())?;
    if index
        .configured_ids
        .iter()
        .chain(index.retired_legacy_ids.iter())
        .any(|id| !NATIVE_SESSION_CREDENTIAL_IDS.contains(&id.as_str()))
    {
        return Err(bundle_error());
    }
    Ok(index)
}

/// Copies only accessible legacy values, leaving replaced or deleted identities retired.
fn migrate_legacy<R>(bundle: &mut CredentialBundle, mut read: R) -> Result<bool, ProviderError>
where
    R: FnMut(&str) -> Result<Option<String>, ProviderError>,
{
    let mut migrated = false;
    for id in NATIVE_SESSION_CREDENTIAL_IDS {
        if bundle.retired_legacy_ids.contains(id) || bundle.secrets.contains_key(id) {
            continue;
        }
        if let Some(secret) = read(id)? {
            bundle.secrets.insert(id.into(), secret);
            bundle.retired_legacy_ids.insert(id.into());
            migrated = true;
        }
    }
    Ok(migrated)
}

/// Reads one bundle after authentication, then migrates only non-interactively accessible legacy entries.
pub(super) fn warm_bundle<A, L>(
    session: &mut CredentialSession,
    configured: bool,
    mut authenticate: A,
    mut load: L,
) -> Result<usize, ProviderError>
where
    A: FnMut() -> Result<(), ProviderError>,
    L: FnMut() -> Result<CredentialBundle, ProviderError>,
{
    if session.bundle.is_some() {
        return Ok(0);
    }
    if !configured {
        session.bundle = Some(CredentialBundle::default());
        return Ok(0);
    }
    if !session.authenticated {
        authenticate()?;
        session.authenticated = true;
    }
    let bundle = load()?;
    let warmed = bundle.secrets.len();
    session.secrets = bundle.secrets.clone();
    session.bundle = Some(bundle);
    Ok(warmed)
}

/// Commits replacement or retirement before publishing either change to the session cache.
pub(super) fn change_bundle<S>(
    session: &mut CredentialSession,
    provider_id: &str,
    api_key: Option<&str>,
    save: S,
) -> Result<(), ProviderError>
where
    S: FnOnce(&CredentialBundle) -> Result<(), ProviderError>,
{
    let mut bundle = session.bundle.clone().ok_or_else(bundle_error)?;
    if let Some(api_key) = api_key {
        bundle.secrets.insert(provider_id.into(), api_key.into());
    } else {
        bundle.secrets.remove(provider_id);
    }
    bundle.retired_legacy_ids.insert(provider_id.into());
    save(&bundle)?;
    session.secrets = bundle.secrets.clone();
    session.bundle = Some(bundle);
    Ok(())
}

/// Rejects corrupt, oversized, or foreign native-vault payloads without including their data in errors.
fn decode_bundle(bytes: &[u8]) -> Result<CredentialBundle, ProviderError> {
    if bytes.len() > MAX_BUNDLE_BYTES {
        return Err(bundle_error());
    }
    let bundle: CredentialBundle = serde_json::from_slice(bytes).map_err(|_| bundle_error())?;
    if bundle
        .secrets
        .keys()
        .chain(bundle.retired_legacy_ids.iter())
        .any(|id| !NATIVE_SESSION_CREDENTIAL_IDS.contains(&id.as_str()))
        || bundle
            .secrets
            .values()
            .any(|secret| secret.trim().is_empty())
    {
        return Err(bundle_error());
    }
    Ok(bundle)
}

/// Explains how to replace an old entry without starting a sequence of per-item password dialogs.
pub(super) fn legacy_locked_error() -> ProviderError {
    ProviderError::invalid_request(
        "This credential is in the older keychain vault. Re-save its API key in Settings to use the single-unlock vault.",
    )
}

/// Returns a fixed error without keychain diagnostics, paths, or payloads.
fn bundle_error() -> ProviderError {
    ProviderError::internal(
        "The operating-system credential vault could not complete the request.",
        None,
    )
}

#[cfg(target_os = "macos")]
mod native {
    use std::sync::Mutex;

    use core_foundation::{
        base::{CFType, TCFType},
        data::CFData,
        string::CFString,
    };
    use security_framework::{
        item::{
            ItemAddOptions, ItemAddValue, ItemClass, ItemSearchOptions, ItemUpdateOptions,
            ItemUpdateValue, Location, SearchResult, update_item,
        },
        os::macos::keychain::SecKeychain,
        passwords::{PasswordOptions, generic_password},
    };
    use security_framework_sys::{
        base::{errSecAuthFailed, errSecDuplicateItem, errSecItemNotFound},
        item::kSecAttrComment,
        keychain::{SecKeychainGetUserInteractionAllowed, SecKeychainSetUserInteractionAllowed},
    };

    use super::*;

    // The legacy Keychain Services interaction switch is process-wide. Serialize every Bottie
    // vault operation and restore its previous value before releasing this lock.
    const ERR_SEC_INTERACTION_NOT_ALLOWED: i32 = -25308;

    static KEYCHAIN_ACCESS: Mutex<()> = Mutex::new(());

    /// Restores the process-wide legacy-keychain UI policy on every exit, including errors.
    struct SilentKeychain(u8);

    impl SilentKeychain {
        /// Disables legacy password dialogs for metadata and best-effort migration only.
        fn enter() -> Result<Self, ProviderError> {
            let mut previous = 0;
            // SAFETY: The output pointer is valid and all Bottie keychain operations hold
            // KEYCHAIN_ACCESS for this guard's entire lifetime.
            unsafe {
                if SecKeychainGetUserInteractionAllowed(&mut previous) != 0
                    || SecKeychainSetUserInteractionAllowed(0) != 0
                {
                    return Err(bundle_error());
                }
            }
            Ok(Self(previous))
        }
    }

    impl Drop for SilentKeychain {
        fn drop(&mut self) {
            // SAFETY: Restore the value captured under KEYCHAIN_ACCESS before unlocking it.
            unsafe { SecKeychainSetUserInteractionAllowed(self.0) };
        }
    }

    /// Builds an attributes-only query; password data is never requested for status.
    fn metadata(service: &str, account: &str) -> Result<Vec<SearchResult>, ProviderError> {
        let mut query = ItemSearchOptions::new();
        query
            .class(ItemClass::generic_password())
            .service(service)
            .account(account)
            .load_attributes(true)
            .limit(1);
        match query.search() {
            Ok(items) => Ok(items),
            Err(error) if error.code() == errSecItemNotFound => Ok(Vec::new()),
            Err(_) => Err(bundle_error()),
        }
    }

    /// Returns the public bundle index without reading or unlocking its secret payload.
    fn index() -> Result<CredentialIndex, ProviderError> {
        let items = metadata(BUNDLE_SERVICE, BUNDLE_ACCOUNT)?;
        let Some(SearchResult::Dict(attributes)) = items.first() else {
            return Ok(CredentialIndex::default());
        };
        // SAFETY: This is the Security framework's static CFString attribute key; the
        // returned CFType is checked before interpreting it as a string.
        let key = unsafe { CFString::wrap_under_get_rule(kSecAttrComment) }.into_CFType();
        let raw = attributes
            .find(key.as_CFTypeRef())
            .ok_or_else(bundle_error)?;
        // SAFETY: The dictionary owns this CFType; retain it before checking its type.
        let value = unsafe { CFType::wrap_under_get_rule(*raw) };
        let comment = value
            .downcast::<CFString>()
            .ok_or_else(bundle_error)?
            .to_string();
        decode_index(&comment)
    }

    /// Lists configured identities from public bundle and legacy attributes only.
    pub(in crate::credentials) fn configured_ids() -> Result<HashSet<String>, ProviderError> {
        let _lock = KEYCHAIN_ACCESS.lock().map_err(|_| bundle_error())?;
        let _silent = SilentKeychain::enter()?;
        let index = index()?;
        let mut ids = index.configured_ids;
        for id in NATIVE_SESSION_CREDENTIAL_IDS {
            if !index.retired_legacy_ids.contains(id)
                && !metadata(super::super::SERVICE_NAME, id)?.is_empty()
            {
                ids.insert(id.into());
            }
        }
        Ok(ids)
    }

    /// Retains deletion tombstones from an empty bundle without retrieving any secret bytes.
    pub(in crate::credentials) fn empty_bundle() -> Result<CredentialBundle, ProviderError> {
        let _lock = KEYCHAIN_ACCESS.lock().map_err(|_| bundle_error())?;
        let _silent = SilentKeychain::enter()?;
        let index = index()?;
        if !index.configured_ids.is_empty() {
            return Err(bundle_error());
        }
        Ok(CredentialBundle {
            secrets: HashMap::new(),
            retired_legacy_ids: index.retired_legacy_ids,
        })
    }

    /// Atomically writes secret data and its public index without first reading a password.
    fn save_locked(bundle: &CredentialBundle) -> Result<(), ProviderError> {
        save_to_keychain(bundle, None)
    }

    /// Uses the login keychain in production; an explicit isolated keychain supports native regression tests.
    fn save_to_keychain(
        bundle: &CredentialBundle,
        keychain: Option<&SecKeychain>,
    ) -> Result<(), ProviderError> {
        let bytes = serde_json::to_vec(bundle).map_err(|_| bundle_error())?;
        decode_bundle(&bytes)?;
        let index = CredentialIndex {
            configured_ids: bundle.secrets.keys().cloned().collect(),
            retired_legacy_ids: bundle.retired_legacy_ids.clone(),
        };
        let comment = serde_json::to_string(&index).map_err(|_| bundle_error())?;
        let data = CFData::from_buffer(&bytes);
        let mut options = ItemAddOptions::new(ItemAddValue::Data {
            class: ItemClass::generic_password(),
            data: data.clone(),
        });
        options
            .set_service(BUNDLE_SERVICE)
            .set_account_name(BUNDLE_ACCOUNT)
            .set_label("Bottie saved credentials")
            .set_comment(&comment);
        if let Some(keychain) = keychain {
            options.set_location(Location::FileKeychain(keychain.clone()));
        }
        match options.add() {
            Ok(_) => Ok(()),
            Err(error) if error.code() == errSecDuplicateItem => {
                let mut query = ItemSearchOptions::new();
                query
                    .class(ItemClass::generic_password())
                    .service(BUNDLE_SERVICE)
                    .account(BUNDLE_ACCOUNT);
                if let Some(keychain) = keychain {
                    query.keychains(std::slice::from_ref(keychain));
                }
                let mut update = ItemUpdateOptions::new();
                update
                    .set_value(ItemUpdateValue::Data(data))
                    .set_comment(&comment);
                update_item(&query, &update).map_err(|_| bundle_error())
            }
            Err(_) => Err(bundle_error()),
        }
    }

    /// Persists all keys under the single item, retaining the prior cache if writing fails.
    pub(in crate::credentials) fn save(bundle: &CredentialBundle) -> Result<(), ProviderError> {
        let _lock = KEYCHAIN_ACCESS.lock().map_err(|_| bundle_error())?;
        save_locked(bundle)
    }

    /// Removes the old secret on explicit credential removal, without reading it or opening another dialog.
    pub(in crate::credentials) fn delete_legacy(provider_id: &str) -> Result<(), ProviderError> {
        let _lock = KEYCHAIN_ACCESS.lock().map_err(|_| bundle_error())?;
        let _silent = SilentKeychain::enter()?;
        let mut query = ItemSearchOptions::new();
        query
            .class(ItemClass::generic_password())
            .service(super::super::SERVICE_NAME)
            .account(provider_id);
        match query.delete() {
            Ok(()) => Ok(()),
            Err(error) if error.code() == errSecItemNotFound => Ok(()),
            Err(_) => Err(bundle_error()),
        }
    }

    /// Reads the one current item, then silently copies authorized older entries into it.
    pub(in crate::credentials) fn load() -> Result<CredentialBundle, ProviderError> {
        let _lock = KEYCHAIN_ACCESS.lock().map_err(|_| bundle_error())?;
        let mut bundle = match generic_password(PasswordOptions::new_generic_password(
            BUNDLE_SERVICE,
            BUNDLE_ACCOUNT,
        )) {
            Ok(bytes) => decode_bundle(&bytes)?,
            Err(error) if error.code() == errSecItemNotFound => CredentialBundle::default(),
            Err(_) => return Err(bundle_error()),
        };
        let migrated = {
            let _silent = SilentKeychain::enter()?;
            migrate_legacy(&mut bundle, |id| {
                let options = PasswordOptions::new_generic_password(super::super::SERVICE_NAME, id);
                match generic_password(options) {
                    Ok(bytes) => Ok(Some(String::from_utf8(bytes).map_err(|_| bundle_error())?)),
                    Err(error)
                        if [
                            errSecItemNotFound,
                            ERR_SEC_INTERACTION_NOT_ALLOWED,
                            errSecAuthFailed,
                        ]
                        .contains(&error.code()) =>
                    {
                        Ok(None)
                    }
                    Err(_) => Err(bundle_error()),
                }
            })?
        };
        if migrated {
            save_locked(&bundle)?;
        }
        Ok(bundle)
    }

    #[cfg(test)]
    #[test]
    fn isolated_keychain_updates_both_password_and_public_index_without_dialogs() {
        use security_framework::os::macos::keychain::CreateOptions;

        let _lock = KEYCHAIN_ACCESS.lock().expect("keychain lock");
        let _silent = SilentKeychain::enter().expect("disable fixture dialogs");
        let directory =
            std::env::temp_dir().join(format!("bottie-vault-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).expect("fixture directory");
        let keychain = CreateOptions::new()
            .password("fixture-password")
            .create(directory.join("fixture.keychain"))
            .expect("isolated fixture keychain");
        let mut bundle = CredentialBundle {
            secrets: HashMap::from([("openai".into(), "fixture-secret".into())]),
            retired_legacy_ids: HashSet::from(["openai".into()]),
        };
        save_to_keychain(&bundle, Some(&keychain)).expect("create bundle");
        bundle.secrets.remove("openai");
        bundle
            .secrets
            .insert("localmail".into(), "lmk_fixture-key".into());
        bundle.retired_legacy_ids.insert("localmail".into());
        save_to_keychain(&bundle, Some(&keychain)).expect("update bundle and index atomically");
        let mut query = ItemSearchOptions::new();
        query
            .keychains(std::slice::from_ref(&keychain))
            .class(ItemClass::generic_password())
            .service(BUNDLE_SERVICE)
            .account(BUNDLE_ACCOUNT)
            .load_attributes(true)
            .limit(1);
        let attributes = query.search().expect("metadata without secret read");
        let public = attributes[0].simplify_dict().expect("public attributes");
        let index = decode_index(&public["icmt"]).expect("updated public index");
        assert_eq!(index.configured_ids, HashSet::from(["localmail".into()]));
        assert_eq!(index.retired_legacy_ids, bundle.retired_legacy_ids);
        assert!(
            !public
                .values()
                .any(|value| value.contains("fixture-secret") || value.contains("lmk_fixture-key"))
        );
        let (bytes, _) = keychain
            .find_generic_password(BUNDLE_SERVICE, BUNDLE_ACCOUNT)
            .expect("read one fixture item");
        assert_eq!(
            decode_bundle(&bytes).expect("updated payload").secrets,
            bundle.secrets
        );
        query.delete().expect("remove fixture item");
        drop(bytes);
        unsafe extern "C" {
            fn SecKeychainDelete(keychain: security_framework_sys::base::SecKeychainRef) -> i32;
        }
        // SAFETY: This handle refers exclusively to the fixture keychain created above.
        // Delete its storage and remove any fixture search-list reference before releasing it.
        assert_eq!(
            unsafe { SecKeychainDelete(keychain.as_concrete_TypeRef()) },
            0
        );
        drop(keychain);
        std::fs::remove_dir_all(directory).expect("remove isolated fixture directory");
    }
}

#[cfg(target_os = "macos")]
pub(super) use native::{configured_ids, delete_legacy, empty_bundle, load, save};

#[cfg(test)]
#[path = "credentials_macos_tests.rs"]
mod tests;
