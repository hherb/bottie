//! Operating-system speech backend with native-only voice identities.

use super::{NativeSpeechVoice, SpeechBackend, SpeechErrorCode};

/// Operating-system speech engine retained alongside optional neural voices.
pub(super) struct SystemSpeechBackend {
    engine: tts::Tts,
    native_voices: Vec<tts::Voice>,
}

impl SystemSpeechBackend {
    /// Opens the operating-system engine without speaking or touching model assets.
    pub(super) fn new() -> Result<Self, SpeechErrorCode> {
        let engine = tts::Tts::default().map_err(|_| SpeechErrorCode::Unavailable)?;
        Ok(Self {
            engine,
            native_voices: Vec::new(),
        })
    }
}

impl SpeechBackend for SystemSpeechBackend {
    fn voices(&mut self) -> Result<Vec<NativeSpeechVoice>, SpeechErrorCode> {
        self.native_voices = self
            .engine
            .voices()
            .map_err(|_| SpeechErrorCode::Unavailable)?;
        Ok(self
            .native_voices
            .iter()
            .map(|voice| NativeSpeechVoice {
                id: voice.id(),
                name: voice.name(),
                language: voice.language().to_string(),
            })
            .collect())
    }

    fn set_voice(&mut self, voice_id: &str) -> Result<(), SpeechErrorCode> {
        let voice = self
            .native_voices
            .iter()
            .find(|voice| voice.id() == voice_id)
            .ok_or(SpeechErrorCode::Unavailable)?;
        self.engine
            .set_voice(voice)
            .map_err(|_| SpeechErrorCode::PlaybackFailed)
    }

    fn speak(&mut self, text: &str) -> Result<(), SpeechErrorCode> {
        self.engine
            .speak(text, true)
            .map(|_| ())
            .map_err(|_| SpeechErrorCode::PlaybackFailed)
    }

    fn stop(&mut self) -> Result<(), SpeechErrorCode> {
        self.engine
            .stop()
            .map(|_| ())
            .map_err(|_| SpeechErrorCode::PlaybackFailed)
    }

    fn is_speaking(&mut self) -> Result<bool, SpeechErrorCode> {
        self.engine
            .is_speaking()
            .map_err(|_| SpeechErrorCode::PlaybackFailed)
    }
}
