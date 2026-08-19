//! Provider construction and provider-neutral routing.

use crate::{
    credentials::CredentialStore,
    inference::{
        AnthropicProvider, ChatRequest, InferenceProvider, ModelInfo, OllamaProvider, OmlxProvider,
        OpenAiProvider, ProviderError, ProviderSettings, StreamSink, Usage,
    },
};

/// Active local adapters plus secret-free settings shared by every route.
#[derive(Clone)]
pub(crate) struct ProviderSet {
    pub(crate) omlx: OmlxProvider,
    pub(crate) ollama: OllamaProvider,
    pub(crate) settings: ProviderSettings,
}

impl ProviderSet {
    /// Builds a complete local-provider set from validated settings.
    pub(crate) fn from_settings(settings: &ProviderSettings) -> Result<Self, ProviderError> {
        Ok(Self {
            omlx: OmlxProvider::with_base_url(&settings.omlx_base_url)?,
            ollama: OllamaProvider::with_base_url(&settings.ollama_base_url)?,
            settings: settings.clone(),
        })
    }

    /// Returns a snapshot of the active provider settings.
    pub(crate) fn settings(&self) -> ProviderSettings {
        self.settings.clone()
    }

    fn local_provider(&self, provider_id: &str) -> Result<RoutedProvider, ProviderError> {
        match provider_id {
            "omlx" => Ok(RoutedProvider::Omlx(self.omlx.clone())),
            "ollama" => Ok(RoutedProvider::Ollama(self.ollama.clone())),
            _ => Err(ProviderError::invalid_request(
                "Choose a supported local provider.",
            )),
        }
    }
}

/// One concrete provider selected for discovery or chat streaming.
#[derive(Clone)]
pub(crate) enum RoutedProvider {
    /// Local oMLX route.
    Omlx(OmlxProvider),
    /// Local Ollama route.
    Ollama(OllamaProvider),
    /// Remote OpenAI-compatible route.
    OpenAi(OpenAiProvider),
    /// Remote Anthropic-compatible route.
    Anthropic(AnthropicProvider),
}

impl RoutedProvider {
    /// Returns the provider's stable routing identity.
    pub(crate) fn provider_id(&self) -> &'static str {
        match self {
            Self::Omlx(_) => "omlx",
            Self::Ollama(_) => "ollama",
            Self::OpenAi(_) => "openai",
            Self::Anthropic(_) => "anthropic",
        }
    }

    /// Streams a chat through the concrete provider.
    pub(crate) async fn stream_chat(
        &self,
        request: ChatRequest,
        sink: impl StreamSink + Send + Sync,
    ) -> Result<Option<Usage>, ProviderError> {
        match self {
            Self::Omlx(provider) => provider.stream_chat(request, sink).await,
            Self::Ollama(provider) => provider.stream_chat(request, sink).await,
            Self::OpenAi(provider) => provider.stream_chat(request, sink).await,
            Self::Anthropic(provider) => provider.stream_chat(request, sink).await,
        }
    }

    /// Discovers models through the concrete provider.
    pub(crate) async fn discover_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        match self {
            Self::Omlx(provider) => provider.discover_models().await,
            Self::Ollama(provider) => provider.discover_models().await,
            Self::OpenAi(provider) => provider.discover_models().await,
            Self::Anthropic(provider) => provider.discover_models().await,
        }
    }
}

/// Resolves a local adapter or constructs an authenticated remote adapter from vault state.
pub(crate) fn routed_provider(
    provider_id: &str,
    providers: &ProviderSet,
    credentials: &dyn CredentialStore,
) -> Result<RoutedProvider, ProviderError> {
    match provider_id {
        "omlx" | "ollama" => providers.local_provider(provider_id),
        "openai" => {
            let key = credentials.get(provider_id)?.ok_or_else(|| {
                ProviderError::invalid_request("Add an OpenAI-compatible API key in Settings.")
            })?;
            Ok(RoutedProvider::OpenAi(OpenAiProvider::new(
                &providers.settings.openai_base_url,
                key,
            )?))
        }
        "anthropic" => {
            let key = credentials.get(provider_id)?.ok_or_else(|| {
                ProviderError::invalid_request("Add an Anthropic-compatible API key in Settings.")
            })?;
            Ok(RoutedProvider::Anthropic(AnthropicProvider::new(
                &providers.settings.anthropic_base_url,
                key,
            )?))
        }
        _ => Err(ProviderError::invalid_request(
            "Choose a supported provider.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCredentialStore {
        secret: Option<String>,
    }

    impl CredentialStore for TestCredentialStore {
        fn configured(&self, _provider_id: &str) -> Result<bool, ProviderError> {
            Ok(self.secret.is_some())
        }

        fn unlocked(&self, _provider_id: &str) -> Result<bool, ProviderError> {
            Ok(self.secret.is_some())
        }

        fn biometric_protected(&self) -> bool {
            false
        }

        fn get(&self, _provider_id: &str) -> Result<Option<String>, ProviderError> {
            Ok(self.secret.clone())
        }

        fn set(&self, _provider_id: &str, _api_key: &str) -> Result<(), ProviderError> {
            unreachable!("routing never writes credentials")
        }

        fn delete(&self, _provider_id: &str) -> Result<(), ProviderError> {
            unreachable!("routing never deletes credentials")
        }
    }

    #[test]
    fn remote_routes_require_a_vault_credential() {
        let providers = ProviderSet::from_settings(&ProviderSettings::default()).unwrap();
        let credentials = TestCredentialStore { secret: None };

        let error = match routed_provider("openai", &providers, &credentials) {
            Ok(_) => panic!("a remote route must not start without a credential"),
            Err(error) => error,
        };

        assert_eq!(
            error.message,
            "Add an OpenAI-compatible API key in Settings."
        );
    }

    #[test]
    fn remote_routes_use_their_native_adapter() {
        let providers = ProviderSet::from_settings(&ProviderSettings::default()).unwrap();
        let credentials = TestCredentialStore {
            secret: Some("test-secret".into()),
        };

        assert_eq!(
            routed_provider("openai", &providers, &credentials)
                .unwrap()
                .provider_id(),
            "openai"
        );
        assert_eq!(
            routed_provider("anthropic", &providers, &credentials)
                .unwrap()
                .provider_id(),
            "anthropic"
        );
    }
}
