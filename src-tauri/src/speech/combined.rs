//! Route closed neural preset identities and existing OS voices through one native controller.

use super::{
    MAX_SPEECH_VOICES, NativeSpeechVoice, SpeechBackend, SpeechErrorCode, nano::NanoBackend,
    system::SystemSpeechBackend,
};
use std::path::Path;

/// Keeps optional Nano usable even when the host's system speech engine is unavailable.
pub(super) struct CombinedSpeechBackend {
    system: Option<Box<dyn SpeechBackend>>,
    nano: Option<Box<dyn SpeechBackend>>,
    nano_selected: bool,
}

impl CombinedSpeechBackend {
    /// Discover voices without loading neural weights or opening an output stream.
    pub(super) fn new(app_data: Option<&Path>) -> Result<Self, SpeechErrorCode> {
        let system = SystemSpeechBackend::new()
            .ok()
            .map(|backend| Box::new(backend) as Box<dyn SpeechBackend>);
        let nano = app_data
            .and_then(NanoBackend::discover)
            .map(|backend| Box::new(backend) as Box<dyn SpeechBackend>);
        if system.is_none() && nano.is_none() {
            return Err(SpeechErrorCode::Unavailable);
        }
        Ok(Self {
            system,
            nano,
            nano_selected: false,
        })
    }
    fn selected(&mut self) -> Result<&mut Box<dyn SpeechBackend>, SpeechErrorCode> {
        if self.nano_selected {
            self.nano.as_mut()
        } else {
            self.system.as_mut()
        }
        .ok_or(SpeechErrorCode::Unavailable)
    }
}

impl SpeechBackend for CombinedSpeechBackend {
    fn voices(&mut self) -> Result<Vec<NativeSpeechVoice>, SpeechErrorCode> {
        let neural = self
            .nano
            .as_mut()
            .map(|backend| backend.voices())
            .transpose()?
            .unwrap_or_default();
        let system_voices = self.system.as_mut().map(|backend| backend.voices());
        let mut voices = match system_voices {
            Some(Ok(voices)) => voices,
            Some(Err(_)) => {
                self.system = None;
                Vec::new()
            }
            None => Vec::new(),
        };
        voices.retain(|voice| !NanoBackend::owns(&voice.id));
        let mut voices = super::voices::bounded_voices(voices)
            .into_iter()
            .map(|(voice, id)| NativeSpeechVoice {
                id,
                name: voice.name,
                language: voice.language,
            })
            .collect::<Vec<_>>();
        voices.truncate(MAX_SPEECH_VOICES.saturating_sub(neural.len()));
        voices.extend(neural);
        Ok(voices)
    }
    fn set_voice(&mut self, id: &str) -> Result<(), SpeechErrorCode> {
        self.stop()?;
        let neural = NanoBackend::owns(id);
        if neural {
            self.nano.as_mut()
        } else {
            self.system.as_mut()
        }
        .ok_or(SpeechErrorCode::Unavailable)?
        .set_voice(id)?;
        self.nano_selected = neural;
        Ok(())
    }
    fn speak(&mut self, text: &str) -> Result<(), SpeechErrorCode> {
        self.selected()?.speak(text)
    }
    fn stop(&mut self) -> Result<(), SpeechErrorCode> {
        // Both are attempted even when one reports an error, so unrelated pending speech cannot survive a switch.
        let neural = self.nano.as_mut().map(|backend| backend.stop()).transpose();
        let system = self
            .system
            .as_mut()
            .map(|backend| backend.stop())
            .transpose();
        neural?;
        system?;
        Ok(())
    }
    fn is_speaking(&mut self) -> Result<bool, SpeechErrorCode> {
        self.selected()?.is_speaking()
    }
}

impl Drop for CombinedSpeechBackend {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Record {
        stops: usize,
        spoken: usize,
        fail_stop: bool,
        fail_voices: bool,
    }
    struct Fake {
        state: Arc<Mutex<Record>>,
        id: &'static str,
    }
    impl SpeechBackend for Fake {
        fn voices(&mut self) -> Result<Vec<NativeSpeechVoice>, SpeechErrorCode> {
            if self.state.lock().unwrap().fail_voices {
                return Err(SpeechErrorCode::Unavailable);
            }
            Ok(vec![NativeSpeechVoice {
                id: self.id.into(),
                name: self.id.into(),
                language: "en".into(),
            }])
        }
        fn set_voice(&mut self, _: &str) -> Result<(), SpeechErrorCode> {
            Ok(())
        }
        fn speak(&mut self, _: &str) -> Result<(), SpeechErrorCode> {
            self.state.lock().unwrap().spoken += 1;
            Ok(())
        }
        fn stop(&mut self) -> Result<(), SpeechErrorCode> {
            let mut state = self.state.lock().unwrap();
            state.stops += 1;
            if state.fail_stop {
                Err(SpeechErrorCode::PlaybackFailed)
            } else {
                Ok(())
            }
        }
        fn is_speaking(&mut self) -> Result<bool, SpeechErrorCode> {
            Ok(false)
        }
    }

    #[test]
    fn unavailable_unused_system_engine_cannot_prevent_nano_selection() {
        let state = Arc::new(Mutex::new(Record {
            fail_voices: true,
            fail_stop: true,
            ..Record::default()
        }));
        let neural = Arc::new(Mutex::new(Record::default()));
        let mut backend = CombinedSpeechBackend {
            system: Some(Box::new(Fake {
                state,
                id: "system",
            })),
            nano: Some(Box::new(Fake {
                state: neural.clone(),
                id: "bottie-moss-nano-v1:Ava",
            })),
            nano_selected: false,
        };
        assert_eq!(backend.voices().unwrap().len(), 1);
        backend.set_voice("bottie-moss-nano-v1:Ava").unwrap();
        backend.speak("Hello").unwrap();
        assert_eq!(neural.lock().unwrap().spoken, 1);
    }

    #[test]
    fn engine_switching_stops_both_and_does_not_hide_a_failed_stop() {
        let system = Arc::new(Mutex::new(Record::default()));
        let neural = Arc::new(Mutex::new(Record::default()));
        let mut backend = CombinedSpeechBackend {
            system: Some(Box::new(Fake {
                state: system.clone(),
                id: "system",
            })),
            nano: Some(Box::new(Fake {
                state: neural.clone(),
                id: "bottie-moss-nano-v1:Ava",
            })),
            nano_selected: false,
        };
        backend.set_voice("system").unwrap();
        backend.speak("System reply").unwrap();
        backend.set_voice("bottie-moss-nano-v1:Ava").unwrap();
        backend.speak("Neural reply").unwrap();
        neural.lock().unwrap().fail_stop = true;
        let before = system.lock().unwrap().stops;
        assert!(backend.set_voice("system").is_err());
        assert!(system.lock().unwrap().stops > before);
        assert!(backend.nano_selected);
        neural.lock().unwrap().fail_stop = false;
        backend.set_voice("system").unwrap();
        assert!(!backend.nano_selected);
    }
}
