//! Provider-neutral inference orchestration and concrete provider adapters.

mod ollama;
mod omlx;
mod provider;
mod settings;
mod types;

pub use ollama::OllamaProvider;
pub use omlx::OmlxProvider;
pub use provider::{InferenceProvider, StreamSink};
pub use settings::{
    ProviderSettings, load_provider_settings, redact_diagnostic, save_provider_settings,
};
pub use types::{ChatRequest, ChatRun, ModelInfo, ProviderError, StreamEvent, Usage};
