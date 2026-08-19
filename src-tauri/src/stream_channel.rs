//! Typed Tauri channel sink for normalized provider stream events.

use tauri::ipc::Channel;

use crate::inference::{ProviderError, StreamEvent, StreamSink, Usage};

/// Forwards one native generation's normalized events to its WebView channel.
pub(crate) struct ChannelSink {
    pub(crate) run_id: String,
    pub(crate) channel: Channel<StreamEvent>,
}

impl StreamSink for ChannelSink {
    fn text_delta(&self, delta: String) -> Result<(), ProviderError> {
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
