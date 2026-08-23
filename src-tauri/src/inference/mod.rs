//! Provider-neutral inference orchestration and concrete provider adapters.

mod anthropic;
mod multimodal;
mod ollama;
mod omlx;
mod openai;
mod provider;
mod settings;
mod sse;
mod types;

pub use anthropic::AnthropicProvider;
pub(crate) use anthropic::{AnthropicToolCall, AnthropicToolResult, AnthropicToolSession};
pub use ollama::OllamaProvider;
pub(crate) use ollama::{OllamaToolCall, OllamaToolResult, OllamaToolSession};
pub use omlx::OmlxProvider;
pub use openai::OpenAiProvider;
pub(crate) use openai::{OpenAiToolCall, OpenAiToolResult, OpenAiToolSession};
pub use provider::{InferenceProvider, StreamSink};
pub use settings::{
    ProviderSettings, load_provider_settings, persist_completed_first_run_setup, redact_diagnostic,
    save_provider_settings,
};
#[cfg(test)]
pub(crate) use types::ChatSettings;
pub use types::{
    ChatRequest, ChatRole, ChatRun, ChatTurn, ContentBlock, ImageMediaType, ModelInfo,
    ProviderError, ProviderErrorCode, ReasoningEffort, StreamEvent, Usage,
};
