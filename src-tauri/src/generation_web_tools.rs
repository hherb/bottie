//! Generation-only web-search selection and dispatch behind the native credential boundary.

use std::sync::Arc;

use crate::{
    credentials::CredentialStore,
    inference::ProviderError,
    storage::{ConversationStore, SemanticEmbedder},
    tool_dispatch::{
        MemoryToolExecution, dispatch_memory_tool, dispatch_web_search_tool, policy_error,
    },
    tool_loop::NativeToolCall,
    tool_policy::{ToolPolicyError, ToolPolicyErrorCode},
    web_search::{BRAVE_SEARCH_PROVIDER_ID, BraveSearchProvider, WEB_SEARCH_TOOL_NAME},
};

/// Confirms explicit Web intent plus Ollama's discovered per-model tool capability.
pub(crate) fn web_tools_enabled(
    web_enabled: bool,
    provider_id: &str,
    model_supports_tools: bool,
) -> bool {
    web_enabled && provider_id == "ollama" && model_supports_tools
}

/// Confirms explicit Memory intent plus a mapped provider's discovered per-model tool capability.
pub(crate) fn memory_tools_enabled(
    memory_enabled: bool,
    provider_id: &str,
    model_supports_tools: bool,
) -> bool {
    memory_enabled
        && matches!(provider_id, "ollama" | "openai" | "anthropic")
        && model_supports_tools
}

/// Resolves the configured Brave credential and constructs the fixed native search adapter.
pub(crate) fn configured_web_search(
    credentials: &dyn CredentialStore,
) -> Result<Arc<dyn NativeWebSearchExecutor>, ProviderError> {
    let api_key = credentials
        .get(BRAVE_SEARCH_PROVIDER_ID)
        .map_err(|_| {
            ProviderError::internal(
                "Bottie could not access the configured web-search credential.",
                None,
            )
        })?
        .ok_or_else(|| {
            ProviderError::invalid_request(
                "Add a Brave Search API key in Settings before enabling Web.",
            )
        })?;
    BraveSearchProvider::new(api_key)
        .map(|provider| Arc::new(provider) as Arc<dyn NativeWebSearchExecutor>)
        .map_err(|_| {
            ProviderError::internal(
                "Bottie could not initialize the configured web-search provider.",
                None,
            )
        })
}

/// Synchronous generation-loop boundary implemented by native asynchronous search providers.
pub(crate) trait NativeWebSearchExecutor: Send + Sync {
    /// Executes one already-correlated raw call through the strict native web dispatcher.
    fn execute(&self, call: &NativeToolCall) -> MemoryToolExecution;
}

impl NativeWebSearchExecutor for BraveSearchProvider {
    fn execute(&self, call: &NativeToolCall) -> MemoryToolExecution {
        tauri::async_runtime::block_on(dispatch_web_search_tool(self, call, None))
    }
}

/// Selects the web or memory dispatcher without giving providers a generic native execution path.
pub(crate) fn dispatch_native_tool(
    store: &ConversationStore,
    embedder: &mut impl SemanticEmbedder,
    call: &NativeToolCall,
    memory_enabled: bool,
    web_search: Option<&dyn NativeWebSearchExecutor>,
) -> MemoryToolExecution {
    if call.tool_name == WEB_SEARCH_TOOL_NAME {
        if let Some(web_search) = web_search {
            return web_search.execute(call);
        }
        return disabled_tool_error();
    }
    if !memory_enabled {
        return disabled_tool_error();
    }
    dispatch_memory_tool(store, embedder, call, None)
}

/// Returns the same fixed unsupported envelope for registered but request-disabled tools.
fn disabled_tool_error() -> MemoryToolExecution {
    policy_error(ToolPolicyError {
        code: ToolPolicyErrorCode::UnsupportedTool,
        message: "That native tool is not available for this request.",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::ProviderErrorCode;

    /// Secret-free fixture with no configured native credential.
    struct CredentialFixture {
        fail_read: bool,
    }

    impl CredentialStore for CredentialFixture {
        fn configured(&self, _provider_id: &str) -> Result<bool, ProviderError> {
            Ok(false)
        }

        fn unlocked(&self, _provider_id: &str) -> Result<bool, ProviderError> {
            Ok(false)
        }

        fn biometric_protected(&self) -> bool {
            false
        }

        fn get(&self, _provider_id: &str) -> Result<Option<String>, ProviderError> {
            if self.fail_read {
                Err(ProviderError::internal(
                    "private vault detail",
                    Some("secret implementation diagnostic".into()),
                ))
            } else {
                Ok(None)
            }
        }

        fn set(&self, _provider_id: &str, _api_key: &str) -> Result<(), ProviderError> {
            unreachable!("generation never writes credentials")
        }

        fn delete(&self, _provider_id: &str) -> Result<(), ProviderError> {
            unreachable!("generation never deletes credentials")
        }
    }

    #[test]
    fn requires_a_native_brave_credential_before_advertising_web_search() {
        let error = match configured_web_search(&CredentialFixture { fail_read: false }) {
            Ok(_) => panic!("missing credential must not build a web-search executor"),
            Err(error) => error,
        };

        assert_eq!(error.code, ProviderErrorCode::InvalidRequest);
        assert_eq!(
            error.message,
            "Add a Brave Search API key in Settings before enabling Web."
        );
        assert!(error.diagnostic.is_none());
    }

    #[test]
    fn redacts_native_credential_access_failures_before_generation() {
        let error = match configured_web_search(&CredentialFixture { fail_read: true }) {
            Ok(_) => panic!("credential failure must not build a web-search executor"),
            Err(error) => error,
        };

        assert_eq!(error.code, ProviderErrorCode::Internal);
        assert_eq!(
            error.message,
            "Bottie could not access the configured web-search credential."
        );
        assert!(error.diagnostic.is_none());
        assert!(!error.message.contains("private"));
        assert!(!error.message.contains("secret"));
    }
}
