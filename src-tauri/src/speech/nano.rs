//! Optional offline MOSS-TTS-Nano inference and native PCM playback.

mod buffer;
mod playback;
mod process;
mod protocol;
mod runtime;
mod service;

use super::{NativeSpeechVoice, SpeechBackend, SpeechErrorCode};
use runtime::NanoRuntime;
use service::{NanoService, Progress};
use std::path::Path;

const VOICE_PREFIX: &str = "bottie-moss-nano-v1:";
const VOICES: [&str; 2] = ["Ava", "Adam"];

/// Optional native speech backend; discovery does not start a model or output device.
pub(super) struct NanoBackend {
    runtime: NanoRuntime,
    voice: String,
    service: Option<NanoService>,
}

impl NanoBackend {
    /// Discovers only a deliberately provisioned native runtime at Bottie's private app-data location.
    pub(super) fn discover(app_data: &Path) -> Option<Self> {
        Some(Self {
            runtime: NanoRuntime::discover(app_data)?,
            voice: VOICES[0].to_owned(),
            service: None,
        })
    }
    /// Recognizes only this backend's closed preset identities, never an arbitrary model or audio path.
    pub(super) fn owns(id: &str) -> bool {
        id.strip_prefix(VOICE_PREFIX)
            .is_some_and(|voice| VOICES.contains(&voice))
    }
}

impl SpeechBackend for NanoBackend {
    fn voices(&mut self) -> Result<Vec<NativeSpeechVoice>, SpeechErrorCode> {
        Ok(VOICES
            .iter()
            .map(|voice| NativeSpeechVoice {
                id: format!("{VOICE_PREFIX}{voice}"),
                name: format!("MOSS Nano · {voice}"),
                language: "mul".to_owned(),
            })
            .collect())
    }
    fn set_voice(&mut self, id: &str) -> Result<(), SpeechErrorCode> {
        if !Self::owns(id) {
            return Err(SpeechErrorCode::Unavailable);
        }
        self.voice = id[VOICE_PREFIX.len()..].to_owned();
        Ok(())
    }
    fn speak(&mut self, text: &str) -> Result<(), SpeechErrorCode> {
        if self
            .service
            .as_ref()
            .is_some_and(|service| service.progress() != Progress::Idle)
        {
            self.stop()?;
        }
        if self.service.is_none() {
            self.service = Some(NanoService::new(self.runtime.clone())?);
        }
        self.service
            .as_mut()
            .ok_or(SpeechErrorCode::Unavailable)?
            .speak(text, &self.voice)
    }
    fn stop(&mut self) -> Result<(), SpeechErrorCode> {
        if let Some(service) = self.service.as_mut() {
            service.stop()?;
        }
        self.service = None;
        Ok(())
    }
    fn is_speaking(&mut self) -> Result<bool, SpeechErrorCode> {
        match self.service.as_ref().map(NanoService::progress) {
            Some(Progress::Failed) => Err(SpeechErrorCode::PlaybackFailed),
            Some(Progress::Active) => Ok(true),
            _ => Ok(false),
        }
    }
}

impl Drop for NanoBackend {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests;
