//! Provider-neutral inference orchestration and concrete provider adapters.

mod anthropic;
mod ollama;
mod omlx;
mod openai;
mod provider;
mod settings;
mod sse;
mod types;

pub use anthropic::AnthropicProvider;
pub use ollama::OllamaProvider;
pub use omlx::OmlxProvider;
pub use openai::OpenAiProvider;
pub use provider::{InferenceProvider, StreamSink};
pub use settings::{
    ProviderSettings, load_provider_settings, redact_diagnostic, save_provider_settings,
};
pub use types::{ChatRequest, ChatRun, ModelInfo, ProviderError, StreamEvent, Usage};
