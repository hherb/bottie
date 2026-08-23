//! Generation-only Localmail selection and execution behind pinned native trust boundaries.

use std::{path::PathBuf, sync::Arc};

use crate::{
    credentials::CredentialStore,
    inference::ProviderError,
    localmail::connection_status,
    tool_dispatch::{
        ConfiguredLocalmailToolExecutor, MemoryToolExecution, dispatch_localmail_tool,
    },
    tool_loop::NativeToolCall,
};

/// Confirms explicit Email intent, configured trust/credential state, and a mapped provider capability.
pub(crate) fn email_tools_enabled(
    email_enabled: bool,
    provider_id: &str,
    model_supports_tools: bool,
    localmail_configured: bool,
) -> bool {
    email_enabled
        && matches!(provider_id, "ollama" | "openai" | "anthropic")
        && model_supports_tools
        && localmail_configured
}

/// Builds one immutable per-generation Localmail executor only when trust and a credential exist.
pub(crate) fn configured_localmail_tools(
    config_path: PathBuf,
    credentials: Arc<dyn CredentialStore>,
) -> Result<Option<Arc<dyn NativeLocalmailToolExecutor>>, ProviderError> {
    let status = connection_status(&config_path, credentials.as_ref())?;
    if status.origin.is_none()
        || status.certificate_sha256.is_none()
        || !status.credential_configured
    {
        return Ok(None);
    }
    Ok(Some(Arc::new(ConfiguredNativeLocalmailTools {
        config_path,
        credentials,
    })))
}

/// Synchronous generation-loop boundary implemented by the asynchronous Localmail dispatcher.
pub(crate) trait NativeLocalmailToolExecutor: Send + Sync {
    /// Executes one correlated call through the strict configured Localmail boundary.
    fn execute(&self, call: &NativeToolCall) -> MemoryToolExecution;
}

/// Production executor retaining configuration paths and credentials only in native memory.
struct ConfiguredNativeLocalmailTools {
    config_path: PathBuf,
    credentials: Arc<dyn CredentialStore>,
}

impl NativeLocalmailToolExecutor for ConfiguredNativeLocalmailTools {
    fn execute(&self, call: &NativeToolCall) -> MemoryToolExecution {
        let executor =
            ConfiguredLocalmailToolExecutor::new(&self.config_path, self.credentials.as_ref());
        tauri::async_runtime::block_on(dispatch_localmail_tool(&executor, call, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vault metadata fixture that rejects any attempt to retrieve credential material.
    struct CredentialFixture {
        configured: bool,
    }

    impl CredentialStore for CredentialFixture {
        fn configured(&self, _provider_id: &str) -> Result<bool, ProviderError> {
            Ok(self.configured)
        }

        fn unlocked(&self, _provider_id: &str) -> Result<bool, ProviderError> {
            Ok(false)
        }

        fn biometric_protected(&self) -> bool {
            true
        }

        fn get(&self, _provider_id: &str) -> Result<Option<String>, ProviderError> {
            panic!("Email definition gating must not retrieve a credential")
        }

        fn set(&self, _provider_id: &str, _api_key: &str) -> Result<(), ProviderError> {
            panic!("test does not mutate credentials")
        }

        fn delete(&self, _provider_id: &str) -> Result<(), ProviderError> {
            panic!("test does not mutate credentials")
        }
    }

    #[test]
    fn withholds_the_executor_until_pinned_trust_and_a_credential_are_configured() {
        let directory = std::env::temp_dir()
            .join("bottie-localmail-generation-tests")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&directory).expect("fixture directory should create");
        let path = directory.join("localmail.json");
        let missing_trust = configured_localmail_tools(
            path.clone(),
            Arc::new(CredentialFixture { configured: true }),
        )
        .expect("missing trust should be a closed disabled state");
        assert!(missing_trust.is_none());

        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "origin": "https://127.0.0.1:3000",
                "certificateSha256": "a".repeat(64)
            }))
            .expect("fixture config should serialize"),
        )
        .expect("fixture config should write");
        let missing_credential = configured_localmail_tools(
            path.clone(),
            Arc::new(CredentialFixture { configured: false }),
        )
        .expect("missing credential should be a closed disabled state");
        assert!(missing_credential.is_none());

        let configured =
            configured_localmail_tools(path, Arc::new(CredentialFixture { configured: true }))
                .expect("configured Localmail should be readable");
        assert!(configured.is_some());
        std::fs::remove_dir_all(directory).expect("fixture directory should clean up");
    }
}
