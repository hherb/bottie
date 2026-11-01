//! Phase 0.4 conversation state machine: one explicit hands-free session over native capture and speech.

use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;

use super::VoiceSpeech;
use crate::{
    microphone::endpointing::EndReason,
    speech::{SpeechCommandError, SpeechController, streaming::SessionProgress},
};

/// User-visible phase of the one hands-free conversation session.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum VoicePhase {
    /// No conversation session is active; push-to-talk and typed chat behave as before.
    #[default]
    Idle,
    /// Native capture is open in conversation mode, waiting for the native end-of-turn decision.
    Listening,
    /// The turn ended and the final transcript is ready for the frontend to submit.
    Finalizing,
    /// A reply was requested and generation has not started streaming speech yet.
    Thinking,
    /// The current reply is being spoken (or its first chunks are buffered).
    Speaking,
}

/// Path-free conversation status exposed to the WebView.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VoiceConversationStatus {
    pub(crate) active: bool,
    pub(crate) phase: VoicePhase,
    pub(crate) pending_reply: bool,
    pub(crate) speech_progress: Option<SessionProgress>,
    pub(crate) endpoint_reason: Option<EndReason>,
}

/// Closed errors for conversation commands; never carries device or engine detail.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VoiceCommandError {
    /// The requested action only applies while a conversation session is active.
    NotActive,
    /// The turn's capture is still open, so the reply cannot be armed yet.
    CaptureActive,
    /// No local speech engine or voice is available for this session.
    SpeechUnavailable,
    /// The local speech engine failed without exposing backend detail.
    SpeechPlaybackFailed,
}

impl From<SpeechCommandError> for VoiceCommandError {
    fn from(error: SpeechCommandError) -> Self {
        match error {
            SpeechCommandError::Unavailable => Self::SpeechUnavailable,
            _ => Self::SpeechPlaybackFailed,
        }
    }
}

/// Owns the one conversation session's phase and its pending "speak the next reply" intent.
pub(crate) struct VoiceConversation {
    state: Mutex<ConversationState>,
    speech: Arc<VoiceSpeech>,
}

#[derive(Default)]
struct ConversationState {
    phase: VoicePhase,
    pending_reply: bool,
}

impl VoiceConversation {
    /// Creates an idle conversation controller over a shared speech coordinator.
    pub(crate) fn new(speech: Arc<VoiceSpeech>) -> Self {
        Self {
            state: Mutex::new(ConversationState::default()),
            speech,
        }
    }

    /// Reports whether a conversation session owns voice input.
    pub(crate) fn is_active(&self) -> bool {
        lock(&self.state).phase != VoicePhase::Idle
    }

    /// Returns the current path-free status, including the microphone's latched endpoint reason.
    pub(crate) fn status(&self, endpoint_reason: Option<EndReason>) -> VoiceConversationStatus {
        let state = lock(&self.state);
        VoiceConversationStatus {
            active: state.phase != VoicePhase::Idle,
            phase: state.phase,
            pending_reply: state.pending_reply,
            speech_progress: self.speech.progress(),
            endpoint_reason,
        }
    }

    /// Enters listening after a successful capture start.
    pub(crate) fn started(&self) {
        let mut state = lock(&self.state);
        state.phase = VoicePhase::Listening;
        state.pending_reply = false;
    }

    /// Re-arms listening after a reply drained, or after an explicit interrupt.
    pub(crate) fn listening(&self) {
        let mut state = lock(&self.state);
        if state.phase != VoicePhase::Idle {
            state.phase = VoicePhase::Listening;
            // A stale "speak the next reply" intent must not survive a re-arm.
            state.pending_reply = false;
        }
    }

    /// Restores listening after a submit failed before a reply was ever dispatched.
    ///
    /// Generation consumes the pending intent at entry; an early error (routing, validation, a store failure) would
    /// otherwise leave the session stuck in `thinking`.
    pub(crate) fn reply_not_started(&self) {
        let mut state = lock(&self.state);
        state.pending_reply = false;
        if state.phase == VoicePhase::Thinking {
            state.phase = VoicePhase::Listening;
        }
    }

    /// Ends the session and stops any active speech.
    pub(crate) fn ended(&self) {
        self.speech.interrupt();
        let mut state = lock(&self.state);
        state.phase = VoicePhase::Idle;
        state.pending_reply = false;
    }

    /// Marks the next submitted reply as spoken by this session.
    pub(crate) fn arm_reply(&self) -> Result<(), VoiceCommandError> {
        let mut state = lock(&self.state);
        if state.phase == VoicePhase::Idle {
            return Err(VoiceCommandError::NotActive);
        }
        state.pending_reply = true;
        state.phase = VoicePhase::Thinking;
        Ok(())
    }

    /// Consumes the pending intent exactly once; generation calls this when it builds its sink.
    pub(crate) fn take_pending_reply(&self) -> bool {
        let mut state = lock(&self.state);
        if state.phase == VoicePhase::Idle || !state.pending_reply {
            return false;
        }
        state.pending_reply = false;
        true
    }

    /// Starts speaking the run's reply and moves to the speaking phase.
    pub(crate) fn adopt_reply(&self, run_id: &str, speech: &Arc<SpeechController>) -> bool {
        if !self.is_active() {
            return false;
        }
        if self.speech.begin(run_id, speech).is_err() {
            let mut state = lock(&self.state);
            state.phase = VoicePhase::Finalizing;
            return false;
        }
        let mut state = lock(&self.state);
        state.phase = VoicePhase::Speaking;
        true
    }

    /// Marks the reply finished; the frontend re-arms listening when it is ready.
    pub(crate) fn finish_reply(&self) {
        let mut state = lock(&self.state);
        if state.phase != VoicePhase::Idle {
            state.phase = VoicePhase::Finalizing;
        }
    }

    /// Stops speech, clears any pending reply, and returns to listening.
    pub(crate) fn interrupted(&self) {
        self.speech.interrupt();
        let mut state = lock(&self.state);
        state.pending_reply = false;
        if state.phase != VoicePhase::Idle {
            state.phase = VoicePhase::Listening;
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use crate::speech::{NativeSpeechVoice, SpeechBackend, SpeechErrorCode};

    /// Minimal controller backend: one voice, immediate playback completion, recorded stops.
    #[derive(Default)]
    struct FakeBackend {
        stops: Arc<AtomicUsize>,
    }

    impl SpeechBackend for FakeBackend {
        fn voices(&mut self) -> Result<Vec<NativeSpeechVoice>, SpeechErrorCode> {
            Ok(vec![NativeSpeechVoice {
                id: "voice.one".to_owned(),
                name: "One".to_owned(),
                language: "en".to_owned(),
            }])
        }

        fn set_voice(&mut self, _voice_id: &str) -> Result<(), SpeechErrorCode> {
            Ok(())
        }

        fn speak(&mut self, _text: &str) -> Result<(), SpeechErrorCode> {
            Ok(())
        }

        fn stop(&mut self) -> Result<(), SpeechErrorCode> {
            self.stops.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn is_speaking(&mut self) -> Result<bool, SpeechErrorCode> {
            Ok(false)
        }
    }

    fn speech() -> Arc<SpeechController> {
        Arc::new(SpeechController::with_backend(Box::new(
            FakeBackend::default(),
        )))
    }

    fn conversation() -> (Arc<VoiceConversation>, Arc<SpeechController>) {
        let speech = speech();
        (
            Arc::new(VoiceConversation::new(Arc::new(VoiceSpeech::new()))),
            speech,
        )
    }

    #[test]
    fn starts_listening_and_ends_idle() {
        let (conversation, _speech) = conversation();
        assert_eq!(conversation.status(None).phase, VoicePhase::Idle);
        assert!(!conversation.is_active());

        conversation.started();
        let status = conversation.status(None);
        assert!(status.active);
        assert_eq!(status.phase, VoicePhase::Listening);

        conversation.ended();
        assert!(!conversation.is_active());
        assert_eq!(conversation.status(None).phase, VoicePhase::Idle);
    }

    #[test]
    fn arming_marks_thinking_and_is_single_use() {
        let (conversation, _speech) = conversation();
        conversation.started();
        conversation.arm_reply().unwrap();
        let status = conversation.status(None);
        assert_eq!(status.phase, VoicePhase::Thinking);
        assert!(status.pending_reply);

        assert!(conversation.take_pending_reply());
        assert!(!conversation.take_pending_reply());
        assert!(!conversation.status(None).pending_reply);
    }

    #[test]
    fn adopting_a_reply_starts_speech_and_finishing_returns_to_finalizing() {
        let (conversation, speech) = conversation();
        conversation.started();
        conversation.arm_reply().unwrap();
        assert!(conversation.take_pending_reply());
        assert!(conversation.adopt_reply("run-1", &speech));
        let status = conversation.status(None);
        assert_eq!(status.phase, VoicePhase::Speaking);

        conversation.finish_reply();
        assert_eq!(conversation.status(None).phase, VoicePhase::Finalizing);
    }

    #[test]
    fn interrupt_returns_to_listening_and_clears_the_pending_reply() {
        let (conversation, _speech) = conversation();
        conversation.started();
        conversation.arm_reply().unwrap();
        conversation.interrupted();
        let status = conversation.status(None);
        assert_eq!(status.phase, VoicePhase::Listening);
        assert!(!status.pending_reply);
        assert!(status.active);
    }

    #[test]
    fn a_reply_that_never_dispatches_restores_listening() {
        let (conversation, _speech) = conversation();
        conversation.started();
        conversation.arm_reply().unwrap();
        assert!(conversation.take_pending_reply());
        conversation.reply_not_started();
        let status = conversation.status(None);
        assert_eq!(status.phase, VoicePhase::Listening);
        assert!(!status.pending_reply);
    }

    #[test]
    fn arming_requires_an_active_conversation() {
        let (conversation, _speech) = conversation();
        assert_eq!(conversation.arm_reply(), Err(VoiceCommandError::NotActive));
        assert!(!conversation.take_pending_reply());
    }
}
