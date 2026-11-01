//! Phase 0.3 bridge between one voice-conversation turn and streaming speech.
//!
//! The conversation controller (`phase 0.4`) will begin a turn, the generation path asks this coordinator for the
//! session that speaks the matching run, and `ChannelSink` feeds that session's reducer from the token stream. Keeping
//! the lifecycle here means the generation path stays unaware of voices: it only asks "is this run spoken?".
//!
//! `VoiceSpeech` is the phase-0.3 half of that bridge and `VoiceConversation` is the phase-0.4 state machine the Tauri
//! commands drive. The conversation phase lives in its own module; this module owns the speech session lifecycle.

#![cfg_attr(not(test), allow(dead_code))]

mod conversation;

pub(crate) use conversation::{VoiceCommandError, VoiceConversation, VoiceConversationStatus};

use std::sync::{Arc, Mutex, MutexGuard};

use crate::speech::{
    SpeechController,
    streaming::{SessionProgress, StreamingSpeechSession},
};

/// Owns at most one streamed reply's speech session at a time.
#[derive(Default)]
pub(crate) struct VoiceSpeech {
    active: Mutex<Option<ActiveTurn>>,
}

/// One run whose reply is being spoken.
struct ActiveTurn {
    run_id: String,
    session: Arc<StreamingSpeechSession>,
}

impl VoiceSpeech {
    /// Creates an idle coordinator.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Starts speaking one run's reply through the local speech controller.
    ///
    /// A second begin is rejected until the active turn is finished or interrupted, so two replies can never share one
    /// audio device.
    pub(crate) fn begin(&self, run_id: &str, speech: &Arc<SpeechController>) -> Result<(), ()> {
        let mut active = lock(&self.active);
        if active.is_some() {
            return Err(());
        }
        let session = Arc::new(StreamingSpeechSession::start(speech.clone())?);
        *active = Some(ActiveTurn {
            run_id: run_id.to_owned(),
            session,
        });
        Ok(())
    }

    /// Returns the session that speaks this run, or `None` when the run is not the active voice turn.
    pub(crate) fn session_for(&self, run_id: &str) -> Option<Arc<StreamingSpeechSession>> {
        lock(&self.active)
            .as_ref()
            .filter(|turn| turn.run_id == run_id)
            .map(|turn| Arc::clone(&turn.session))
    }

    /// Ends the run's session, flushing remaining text and letting playback drain.
    ///
    /// Runs for other ids are left untouched; the generation path calls this unconditionally at the end of every run.
    pub(crate) fn finish_run(&self, run_id: &str) {
        let turn = {
            let mut active = lock(&self.active);
            match active.as_ref() {
                Some(current) if current.run_id == run_id => active.take(),
                _ => None,
            }
        };
        if let Some(turn) = turn {
            let _ = turn.session.end();
        }
    }

    /// Interrupts the active voice turn, if any, and drops queued speech.
    pub(crate) fn interrupt(&self) {
        let turn = lock(&self.active).take();
        if let Some(turn) = turn {
            let _ = turn.session.interrupt();
        }
    }

    /// Reports the active session's progress without waiting for playback.
    pub(crate) fn progress(&self) -> Option<SessionProgress> {
        lock(&self.active)
            .as_ref()
            .map(|turn| turn.session.progress())
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
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use crate::speech::{NativeSpeechVoice, SpeechBackend, SpeechErrorCode};

    /// Minimum controller backend: one voice, controllable speaking state, recorded utterances and stops.
    #[derive(Default)]
    struct FakeBackend {
        spoken: Arc<Mutex<Vec<String>>>,
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

        fn speak(&mut self, text: &str) -> Result<(), SpeechErrorCode> {
            self.spoken.lock().unwrap().push(text.to_owned());
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

    fn controller() -> (Arc<SpeechController>, Arc<Mutex<Vec<String>>>) {
        let backend = FakeBackend::default();
        let spoken = backend.spoken.clone();
        (
            Arc::new(SpeechController::with_backend(Box::new(backend))),
            spoken,
        )
    }

    #[test]
    fn a_session_is_attached_only_to_its_own_run() {
        let (speech, _spoken) = controller();
        let coordinator = VoiceSpeech::new();
        coordinator.begin("run-1", &speech).unwrap();

        assert!(coordinator.session_for("run-1").is_some());
        assert!(coordinator.session_for("run-2").is_none());
        assert!(coordinator.progress().is_some());

        coordinator.finish_run("run-1");
        assert!(coordinator.session_for("run-1").is_none());
        assert!(coordinator.progress().is_none());
    }

    #[test]
    fn a_second_turn_is_rejected_while_one_is_active() {
        let (speech, _spoken) = controller();
        let coordinator = VoiceSpeech::new();
        coordinator.begin("run-1", &speech).unwrap();
        assert_eq!(coordinator.begin("run-2", &speech), Err(()));
        coordinator.finish_run("run-1");
        assert_eq!(coordinator.begin("run-2", &speech), Ok(()));
        coordinator.finish_run("run-2");
    }

    #[test]
    fn feeding_the_session_speaks_the_reduced_text_through_the_controller() {
        let (speech, spoken) = controller();
        let coordinator = VoiceSpeech::new();
        coordinator.begin("run-1", &speech).unwrap();
        let session = coordinator.session_for("run-1").unwrap();
        session
            .push_delta("Hello **there**, [docs](https://example.com). ")
            .unwrap();
        session.end().unwrap();

        // The session worker plays the reduced chunk through the controller's backend.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline && spoken.lock().unwrap().is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let spoken = spoken.lock().unwrap().clone();
        assert_eq!(spoken.join(" "), "Hello there, docs.");
        coordinator.finish_run("run-1");
    }

    #[test]
    fn interrupt_detaches_the_turn_and_stops_playback() {
        let (speech, _spoken) = controller();
        let coordinator = VoiceSpeech::new();
        coordinator.begin("run-1", &speech).unwrap();
        coordinator.interrupt();
        assert!(coordinator.session_for("run-1").is_none());
        assert!(coordinator.progress().is_none());
    }
}
