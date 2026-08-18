use super::{ChatRequest, ModelInfo, ProviderError, Usage};

/// Receives normalized deltas and usage while a provider stream is active.
pub trait StreamSink {
    fn text_delta(&self, delta: String) -> Result<(), ProviderError>;
    fn usage_updated(&self, usage: Usage) -> Result<(), ProviderError>;
}

/// The narrow provider contract needed by the first inference slice.
pub trait InferenceProvider: Clone + Send + Sync + 'static {
    async fn discover_models(&self) -> Result<Vec<ModelInfo>, ProviderError>;

    async fn stream_chat(
        &self,
        request: ChatRequest,
        sink: impl StreamSink + Send + Sync,
    ) -> Result<Option<Usage>, ProviderError>;
}
