use super::{ChatRequest, ModelInfo, ProviderError, Usage};

/// Receives normalized deltas and usage while a provider stream is active.
pub trait StreamSink {
    /// Delivers one normalized text fragment.
    fn text_delta(&self, delta: String) -> Result<(), ProviderError>;
    /// Delivers one normalized reasoning fragment separately from answer text.
    fn reasoning_delta(&self, delta: String) -> Result<(), ProviderError>;
    /// Delivers the latest normalized usage totals.
    fn usage_updated(&self, usage: Usage) -> Result<(), ProviderError>;
}

/// The narrow provider contract needed by the first inference slice.
pub trait InferenceProvider: Clone + Send + Sync + 'static {
    /// Discovers provider-qualified models available for inference.
    async fn discover_models(&self) -> Result<Vec<ModelInfo>, ProviderError>;

    /// Streams one provider-neutral chat request into the supplied normalized sink.
    async fn stream_chat(
        &self,
        request: ChatRequest,
        sink: impl StreamSink + Send + Sync,
    ) -> Result<Option<Usage>, ProviderError>;
}
