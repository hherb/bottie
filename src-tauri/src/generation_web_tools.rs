//! Generation-only Web tool selection and dispatch behind native trust boundaries.

use std::sync::Arc;

use crate::{
    credentials::CredentialStore,
    inference::ProviderError,
    storage::{ConversationStore, SemanticEmbedder},
    tool_dispatch::{
        MemoryToolExecution, dispatch_memory_tool, dispatch_web_fetch_tool,
        dispatch_web_search_tool, policy_error,
    },
    tool_loop::NativeToolCall,
    tool_policy::{ToolPolicyError, ToolPolicyErrorCode},
    web_fetch::{NativeWebFetch, WEB_FETCH_TOOL_NAME},
    web_search::{
        BRAVE_SEARCH_PROVIDER_ID, BraveSearchProvider, EXA_SEARCH_PROVIDER_ID, ExaSearchProvider,
        WEB_SEARCH_TOOL_NAME,
    },
};

/// Confirms explicit Web intent plus a mapped provider's discovered per-model tool capability.
pub(crate) fn web_tools_enabled(
    web_enabled: bool,
    provider_id: &str,
    model_supports_tools: bool,
) -> bool {
    web_enabled && matches!(provider_id, "ollama" | "openai" | "anthropic") && model_supports_tools
}

/// Confirms that an already-enabled Web request has an explicit provider-native fetch mapping.
pub(crate) fn web_fetch_enabled(web_tools_enabled: bool, provider_id: &str) -> bool {
    web_tools_enabled && matches!(provider_id, "ollama" | "openai" | "anthropic")
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

/// Resolves the selected credential and constructs its fixed native search adapter.
pub(crate) fn configured_web_search(
    provider_id: &str,
    credentials: &dyn CredentialStore,
) -> Result<Arc<dyn NativeWebSearchExecutor>, ProviderError> {
    if !matches!(
        provider_id,
        BRAVE_SEARCH_PROVIDER_ID | EXA_SEARCH_PROVIDER_ID
    ) {
        return Err(ProviderError::invalid_request(
            "Choose a supported web search engine in Settings.",
        ));
    }
    let api_key = credentials
        .get(provider_id)
        .map_err(|_| {
            ProviderError::internal(
                "Bottie could not access the configured web-search credential.",
                None,
            )
        })?
        .ok_or_else(|| ProviderError::invalid_request(missing_credential_message(provider_id)))?;
    let provider = match provider_id {
        BRAVE_SEARCH_PROVIDER_ID => BraveSearchProvider::new(api_key)
            .map(|provider| Arc::new(provider) as Arc<dyn NativeWebSearchExecutor>),
        EXA_SEARCH_PROVIDER_ID => ExaSearchProvider::new(api_key)
            .map(|provider| Arc::new(provider) as Arc<dyn NativeWebSearchExecutor>),
        _ => unreachable!("provider identity was validated above"),
    };
    provider.map_err(|_| {
        ProviderError::internal(
            "Bottie could not initialize the configured web-search provider.",
            None,
        )
    })
}

/// Constructs the credential-free public-network fetcher for an explicitly mapped request.
pub(crate) fn configured_web_fetch() -> Arc<dyn NativeWebFetchExecutor> {
    Arc::new(NativeWebFetch::new())
}

/// Returns the user action for a missing selected search credential.
fn missing_credential_message(provider_id: &str) -> &'static str {
    match provider_id {
        EXA_SEARCH_PROVIDER_ID => "Add an Exa Search API key in Settings before enabling Web.",
        _ => "Add a Brave Search API key in Settings before enabling Web.",
    }
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

impl NativeWebSearchExecutor for ExaSearchProvider {
    fn execute(&self, call: &NativeToolCall) -> MemoryToolExecution {
        tauri::async_runtime::block_on(dispatch_web_search_tool(self, call, None))
    }
}

/// Synchronous generation-loop boundary implemented by the asynchronous native fetcher.
pub(crate) trait NativeWebFetchExecutor: Send + Sync {
    /// Executes one already-correlated raw call through the strict native web-fetch dispatcher.
    fn execute(&self, call: &NativeToolCall) -> MemoryToolExecution;
}

impl NativeWebFetchExecutor for NativeWebFetch {
    fn execute(&self, call: &NativeToolCall) -> MemoryToolExecution {
        tauri::async_runtime::block_on(dispatch_web_fetch_tool(self, call, None))
    }
}

/// Selects the web or memory dispatcher without giving providers a generic native execution path.
pub(crate) fn dispatch_native_tool(
    store: &ConversationStore,
    embedder: &mut impl SemanticEmbedder,
    call: &NativeToolCall,
    memory_enabled: bool,
    web_search: Option<&dyn NativeWebSearchExecutor>,
    web_fetch: Option<&dyn NativeWebFetchExecutor>,
) -> MemoryToolExecution {
    if call.tool_name == WEB_SEARCH_TOOL_NAME {
        if let Some(web_search) = web_search {
            return web_search.execute(call);
        }
        return disabled_tool_error();
    }
    if call.tool_name == WEB_FETCH_TOOL_NAME {
        if let Some(web_fetch) = web_fetch {
            return web_fetch.execute(call);
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

    #[test]
    fn enables_web_fetch_only_for_explicitly_mapped_web_tool_routes() {
        assert!(web_fetch_enabled(true, "ollama"));
        assert!(web_fetch_enabled(true, "openai"));
        assert!(web_fetch_enabled(true, "anthropic"));
        assert!(!web_fetch_enabled(false, "openai"));
        assert!(!web_fetch_enabled(true, "omlx"));
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
    fn requires_the_selected_native_credential_before_advertising_web_search() {
        let error = match configured_web_search(
            BRAVE_SEARCH_PROVIDER_ID,
            &CredentialFixture { fail_read: false },
        ) {
            Ok(_) => panic!("missing credential must not build a web-search executor"),
            Err(error) => error,
        };

        assert_eq!(error.code, ProviderErrorCode::InvalidRequest);
        assert_eq!(
            error.message,
            "Add a Brave Search API key in Settings before enabling Web."
        );
        assert!(error.diagnostic.is_none());

        let error = match configured_web_search(
            EXA_SEARCH_PROVIDER_ID,
            &CredentialFixture { fail_read: false },
        ) {
            Ok(_) => panic!("missing credential must not build a web-search executor"),
            Err(error) => error,
        };
        assert_eq!(
            error.message,
            "Add an Exa Search API key in Settings before enabling Web."
        );
    }

    #[test]
    fn redacts_native_credential_access_failures_before_generation() {
        let error = match configured_web_search(
            EXA_SEARCH_PROVIDER_ID,
            &CredentialFixture { fail_read: true },
        ) {
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

    #[test]
    fn rejects_unknown_search_engines_before_credential_access() {
        let error = match configured_web_search("custom", &CredentialFixture { fail_read: true }) {
            Ok(_) => panic!("unknown provider must not build a web-search executor"),
            Err(error) => error,
        };

        assert_eq!(error.code, ProviderErrorCode::InvalidRequest);
        assert_eq!(
            error.message,
            "Choose a supported web search engine in Settings."
        );
    }
}
