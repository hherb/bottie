//! Typed Tauri channel sink for normalized provider stream events.

use tauri::ipc::Channel;

use crate::{
    inference::{ProviderError, StreamEvent, StreamSink, Usage},
    storage::{ConversationStore, RunBlockKind, StorageError, StoredUsage},
};

/// Forwards one native generation's normalized events to its WebView channel.
#[derive(Clone)]
pub(crate) struct ChannelSink {
    pub(crate) run_id: String,
    pub(crate) channel: Channel<StreamEvent>,
    pub(crate) conversations: ConversationStore,
}

impl StreamSink for ChannelSink {
    fn text_delta(&self, delta: String) -> Result<(), ProviderError> {
        self.conversations
            .checkpoint_provider_delta(&self.run_id, RunBlockKind::Text, &delta)
            .map_err(checkpoint_error)?;
        self.channel
            .send(StreamEvent::TextDelta {
                run_id: self.run_id.clone(),
                delta,
            })
            .map_err(|error| {
                ProviderError::internal(
                    "The inference stream could not reach the interface.",
                    Some(error.to_string()),
                )
            })
    }

    fn reasoning_delta(&self, delta: String) -> Result<(), ProviderError> {
        self.conversations
            .checkpoint_provider_delta(&self.run_id, RunBlockKind::Reasoning, &delta)
            .map_err(checkpoint_error)?;
        self.channel
            .send(StreamEvent::ReasoningDelta {
                run_id: self.run_id.clone(),
                delta,
            })
            .map_err(|error| {
                ProviderError::internal(
                    "The reasoning stream could not reach the interface.",
                    Some(error.to_string()),
                )
            })
    }

    fn usage_updated(&self, usage: Usage) -> Result<(), ProviderError> {
        self.conversations
            .checkpoint_provider_usage(
                &self.run_id,
                StoredUsage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    cost_usd: usage.cost_usd,
                },
            )
            .map_err(checkpoint_error)?;
        self.channel
            .send(StreamEvent::UsageUpdated {
                run_id: self.run_id.clone(),
                usage,
            })
            .map_err(|error| {
                ProviderError::internal(
                    "Usage information could not reach the interface.",
                    Some(error.to_string()),
                )
            })
    }
}

/// Maps a secret-free checkpoint failure into the provider stream's stable error surface.
fn checkpoint_error(error: StorageError) -> ProviderError {
    match error.code {
        "invalid_request" | "not_found" => ProviderError::invalid_request(error.message),
        _ => ProviderError::internal(error.message, None),
    }
}
