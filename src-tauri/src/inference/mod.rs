//! Provider-neutral inference orchestration and concrete provider adapters.

mod omlx;
mod provider;
mod types;

pub use omlx::OmlxProvider;
pub use provider::{InferenceProvider, StreamSink};
pub use types::{ChatRequest, ChatRun, ModelInfo, ProviderError, StreamEvent, Usage};
