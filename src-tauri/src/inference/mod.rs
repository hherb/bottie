//! Provider-neutral inference orchestration and concrete provider adapters.

mod ollama;
mod omlx;
mod provider;
mod types;

pub use ollama::OllamaProvider;
pub use omlx::OmlxProvider;
pub use provider::{InferenceProvider, StreamSink};
pub use types::{ChatRequest, ChatRun, ModelInfo, ProviderError, StreamEvent, Usage};
