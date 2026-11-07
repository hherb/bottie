//! Typed Tauri channel sink for normalized provider stream events.

use std::sync::Arc;

use tauri::ipc::Channel;

use crate::{
    inference::{ProviderError, StreamEvent, StreamSink, Usage},
    speech::streaming::StreamingSpeechSession,
    storage::{ConversationStore, RunBlockKind, StorageError, StoredUsage},
};

/// Forwards one native generation's normalized events to its WebView channel.
#[derive(Clone)]
pub(crate) struct ChannelSink {
    pub(crate) run_id: String,
    pub(crate) channel: Channel<StreamEvent>,
    pub(crate) conversations: ConversationStore,
    /// Present only when this run is the active voice turn whose reply is being spoken.
    pub(crate) speech: Option<Arc<StreamingSpeechSession>>,
}

impl StreamSink for ChannelSink {
    fn text_delta(&self, delta: String) -> Result<(), ProviderError> {
        self.conversations
            .checkpoint_provider_delta(&self.run_id, RunBlockKind::Text, &delta)
            .map_err(checkpoint_error)?;
        feed_speech(&self.speech, &delta);
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

/// Feeds one text delta to the active voice turn without ever failing the provider stream.
///
/// The session only refuses text past its cap, and it reports itself failed in that case, so the result is not needed.
fn feed_speech(speech: &Option<Arc<StreamingSpeechSession>>, delta: &str) {
    if let Some(session) = speech {
        let _ = session.push_delta(delta);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };

    use super::*;
    use crate::speech::streaming::{SpeechSink, StreamingSpeechSession};

    #[derive(Default)]
    struct RecordingSink {
        spoken: Mutex<Vec<String>>,
    }

    impl SpeechSink for RecordingSink {
        fn is_idle(&self) -> Result<bool, ()> {
            Ok(true)
        }

        fn speak(&self, text: &str) -> Result<(), ()> {
            self.spoken.lock().unwrap().push(text.to_owned());
            Ok(())
        }

        fn stop(&self) -> Result<(), ()> {
            Ok(())
        }
    }

    #[test]
    fn a_text_delta_reaches_the_speech_session_when_one_is_attached() {
        let sink = Arc::new(RecordingSink::default());
        let session = Arc::new(StreamingSpeechSession::start(sink.clone()).unwrap());
        feed_speech(&Some(session.clone()), "Hello there. ");
        session.end().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && sink.spoken.lock().unwrap().is_empty() {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(sink.spoken.lock().unwrap().as_slice(), ["Hello there."]);
    }

    #[test]
    fn a_text_delta_without_a_speech_session_is_ignored() {
        feed_speech(&None, "Only on screen. ");
    }
}
