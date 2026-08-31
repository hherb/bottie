//! Bounded Rust-owned local text-to-speech orchestration.

use std::sync::{Mutex, MutexGuard};

use serde::Serialize;

#[cfg(test)]
mod tests;

/// Maximum UTF-8 payload accepted for one explicit local playback action.
pub(crate) const MAX_SPEECH_TEXT_BYTES: usize = 32 * 1_024;
const MAX_SPEECH_VOICES: usize = 128;
const MAX_VOICE_FIELD_BYTES: usize = 160;
const MAX_NATIVE_VOICE_ID_BYTES: usize = 1_024;

/// Path-free local speech lifecycle exposed to the WebView.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SpeechPhase {
    /// No speech is currently playing.
    #[default]
    Idle,
    /// An explicitly requested utterance is playing.
    Speaking,
    /// The local engine failed and has no native detail to expose.
    Error,
}

/// Stable path-free engine failure categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SpeechErrorCode {
    /// No supported local speech engine or voice is available.
    Unavailable,
    /// The local engine failed to select, play, stop, or inspect speech.
    PlaybackFailed,
}

/// Closed validation and engine errors returned by speech commands.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SpeechCommandError {
    /// The requested text is empty after whitespace normalization.
    InvalidText,
    /// The requested text exceeds Bottie's bounded UTF-8 payload.
    TextTooLong,
    /// The requested voice was not returned by the current local engine.
    VoiceNotFound,
    /// Native microphone capture is active, so playback is kept separate.
    MicrophoneActive,
    /// No supported local speech engine or voice is available.
    Unavailable,
    /// The local engine failed without exposing backend detail.
    PlaybackFailed,
}

/// Bounded local voice metadata without device or filesystem identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpeechVoice {
    /// Opaque process-local token accepted back only for exact selection.
    id: String,
    /// Human-readable local voice name.
    name: String,
    /// Best-effort RFC 5646 language tag supplied by the local engine.
    language: String,
}

/// Current path-free local speech state without utterance text.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpeechStatus {
    phase: SpeechPhase,
    selected_voice_id: Option<String>,
    error_code: Option<SpeechErrorCode>,
}

/// Narrow engine surface kept behind Bottie's controller for deterministic tests.
trait SpeechBackend: Send {
    fn voices(&mut self) -> Result<Vec<NativeSpeechVoice>, SpeechErrorCode>;
    fn set_voice(&mut self, voice_id: &str) -> Result<(), SpeechErrorCode>;
    fn speak(&mut self, text: &str) -> Result<(), SpeechErrorCode>;
    fn stop(&mut self) -> Result<(), SpeechErrorCode>;
    fn is_speaking(&mut self) -> Result<bool, SpeechErrorCode>;
}

/// Process-lifetime owner for one local speech engine and session-only selection.
pub(crate) struct SpeechController {
    shared: Mutex<SpeechState>,
}

struct SpeechState {
    backend: Option<Box<dyn SpeechBackend>>,
    voices: Vec<SpeechVoice>,
    voice_selections: Vec<(String, String)>,
    selected_voice_id: Option<String>,
    playback_may_be_active: bool,
    phase: SpeechPhase,
    error_code: Option<SpeechErrorCode>,
}

impl Default for SpeechController {
    fn default() -> Self {
        Self {
            shared: Mutex::new(SpeechState::default()),
        }
    }
}

impl Default for SpeechState {
    fn default() -> Self {
        Self {
            backend: None,
            voices: Vec::new(),
            voice_selections: Vec::new(),
            selected_voice_id: None,
            playback_may_be_active: false,
            phase: SpeechPhase::Idle,
            error_code: None,
        }
    }
}

impl SpeechController {
    #[cfg(test)]
    /// Reports whether capture must remain blocked after an uncertain playback outcome.
    pub(crate) fn blocks_microphone_capture(&self) -> bool {
        let mut state = lock(&self.shared);
        state.refresh_playback();
        state.playback_may_be_active
    }

    /// Returns the current state and refreshes only the engine's speaking flag.
    pub(crate) fn status(&self) -> SpeechStatus {
        let mut state = lock(&self.shared);
        state.refresh_playback();
        state.status()
    }

    /// Lazily initializes the local engine and returns its bounded voice list.
    pub(crate) fn list_voices(&self) -> Result<Vec<SpeechVoice>, SpeechCommandError> {
        let mut state = lock(&self.shared);
        state.ensure_ready()?;
        Ok(state.voices.clone())
    }

    /// Selects one exact voice from the current engine for this process lifetime.
    pub(crate) fn select_voice(&self, voice_id: &str) -> Result<SpeechStatus, SpeechCommandError> {
        let mut state = lock(&self.shared);
        state.ensure_ready()?;
        if !state.voices.iter().any(|voice| voice.id == voice_id) {
            return Err(SpeechCommandError::VoiceNotFound);
        }
        let native_voice_id = state
            .voice_selections
            .iter()
            .find_map(|(public_id, native_id)| (public_id == voice_id).then(|| native_id.clone()))
            .expect("every bounded voice has one native selection");
        state
            .backend
            .as_mut()
            .expect("a ready speech state has a backend")
            .set_voice(&native_voice_id)
            .map_err(|code| state.fail_command(code))?;
        state.selected_voice_id = Some(voice_id.to_owned());
        state.error_code = None;
        Ok(state.status())
    }

    /// Plays one bounded text payload locally, interrupting only earlier Bottie speech.
    pub(crate) fn speak(&self, text: &str) -> Result<SpeechStatus, SpeechCommandError> {
        let text = validated_speech_text(text)?;
        let mut state = lock(&self.shared);
        state.ensure_ready()?;
        state.playback_may_be_active = true;
        state
            .backend
            .as_mut()
            .expect("a ready speech state has a backend")
            .speak(&text)
            .map_err(|code| state.fail_command(code))?;
        state.phase = SpeechPhase::Speaking;
        state.error_code = None;
        Ok(state.status())
    }

    /// Stops current Bottie speech without touching microphone capture or other audio.
    pub(crate) fn stop(&self) -> SpeechStatus {
        let _ = self.stop_before_microphone_capture();
        lock(&self.shared).status()
    }

    /// Stops Bottie's playback and confirms the microphone may start without overlap.
    pub(crate) fn stop_before_microphone_capture(&self) -> Result<(), SpeechCommandError> {
        let mut state = lock(&self.shared);
        if !state.playback_may_be_active {
            state.phase = SpeechPhase::Idle;
            state.error_code = None;
            return Ok(());
        }
        if let Some(backend) = state.backend.as_mut() {
            if let Err(code) = backend.stop() {
                return Err(state.fail_command(code));
            }
        }
        state.playback_may_be_active = false;
        state.phase = SpeechPhase::Idle;
        state.error_code = None;
        Ok(())
    }

    #[cfg(test)]
    fn with_backend(backend: Box<dyn SpeechBackend>) -> Self {
        Self {
            shared: Mutex::new(SpeechState {
                backend: Some(backend),
                ..SpeechState::default()
            }),
        }
    }
}

impl SpeechState {
    fn refresh_playback(&mut self) {
        if !self.playback_may_be_active {
            return;
        }
        let speaking = self.backend.as_mut().map(|backend| backend.is_speaking());
        match speaking {
            Some(Ok(true)) => {
                self.phase = SpeechPhase::Speaking;
                self.error_code = None;
            }
            Some(Ok(false)) => {
                self.playback_may_be_active = false;
                self.phase = SpeechPhase::Idle;
                self.error_code = None;
            }
            Some(Err(code)) => self.fail(code),
            None => self.fail(SpeechErrorCode::Unavailable),
        }
    }

    fn ensure_ready(&mut self) -> Result<(), SpeechCommandError> {
        if self.backend.is_none() {
            self.backend = Some(Box::new(
                NativeSpeechBackend::new().map_err(|code| self.fail_command(code))?,
            ));
        }
        if self.voices.is_empty() {
            let voices = self
                .backend
                .as_mut()
                .expect("an initialized speech state has a backend")
                .voices()
                .map_err(|code| self.fail_command(code))?;
            let bounded = bounded_voices(voices);
            self.voices = bounded.iter().map(|(voice, _)| voice.clone()).collect();
            self.voice_selections = bounded
                .into_iter()
                .map(|(voice, native_id)| (voice.id, native_id))
                .collect();
            let Some(default_voice_id) = self.voices.first().map(|voice| voice.id.clone()) else {
                return Err(self.fail_command(SpeechErrorCode::Unavailable));
            };
            let default_native_id = self
                .voice_selections
                .first()
                .map(|(_, native_id)| native_id.clone())
                .expect("a default bounded voice has one native selection");
            self.backend
                .as_mut()
                .expect("an initialized speech state has a backend")
                .set_voice(&default_native_id)
                .map_err(|code| self.fail_command(code))?;
            self.selected_voice_id = Some(default_voice_id);
        }
        if !self.playback_may_be_active {
            self.error_code = None;
            if self.phase == SpeechPhase::Error {
                self.phase = SpeechPhase::Idle;
            }
        }
        Ok(())
    }

    fn status(&self) -> SpeechStatus {
        SpeechStatus {
            phase: self.phase,
            selected_voice_id: self.selected_voice_id.clone(),
            error_code: self.error_code,
        }
    }

    fn fail(&mut self, code: SpeechErrorCode) {
        self.phase = SpeechPhase::Error;
        self.error_code = Some(code);
    }

    fn fail_command(&mut self, code: SpeechErrorCode) -> SpeechCommandError {
        self.fail(code);
        match code {
            SpeechErrorCode::Unavailable => SpeechCommandError::Unavailable,
            SpeechErrorCode::PlaybackFailed => SpeechCommandError::PlaybackFailed,
        }
    }
}

struct NativeSpeechBackend {
    engine: tts::Tts,
    native_voices: Vec<tts::Voice>,
}

struct NativeSpeechVoice {
    id: String,
    name: String,
    language: String,
}

impl NativeSpeechBackend {
    fn new() -> Result<Self, SpeechErrorCode> {
        let engine = tts::Tts::default().map_err(|_| SpeechErrorCode::Unavailable)?;
        Ok(Self {
            engine,
            native_voices: Vec::new(),
        })
    }
}

impl SpeechBackend for NativeSpeechBackend {
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

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn normalize_speech_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn validated_speech_text(text: &str) -> Result<String, SpeechCommandError> {
    let normalized = normalize_speech_text(text);
    if normalized.is_empty() {
        return Err(SpeechCommandError::InvalidText);
    }
    if normalized.len() > MAX_SPEECH_TEXT_BYTES {
        return Err(SpeechCommandError::TextTooLong);
    }
    Ok(normalized)
}

fn bounded_voices(voices: Vec<NativeSpeechVoice>) -> Vec<(SpeechVoice, String)> {
    let mut voices = voices
        .into_iter()
        .filter_map(|voice| {
            let native_id = voice.id;
            let name = bounded_voice_field(&voice.name);
            let language = bounded_voice_field(&voice.language);
            (!native_id.is_empty()
                && native_id.len() <= MAX_NATIVE_VOICE_ID_BYTES
                && !name.is_empty())
            .then_some((native_id, name, language))
        })
        .collect::<Vec<_>>();
    voices.sort_by(|left, right| left.0.cmp(&right.0));
    voices.dedup_by(|left, right| left.0 == right.0);
    voices.sort_by(|left, right| (&left.2, &left.1, &left.0).cmp(&(&right.2, &right.1, &right.0)));
    voices.truncate(MAX_SPEECH_VOICES);
    voices
        .into_iter()
        .enumerate()
        .map(|(index, (native_id, name, language))| {
            (
                SpeechVoice {
                    id: format!("local-voice-{number:03}", number = index + 1),
                    name,
                    language,
                },
                native_id,
            )
        })
        .collect()
}

fn bounded_voice_field(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let mut end = normalized.len().min(MAX_VOICE_FIELD_BYTES);
    while !normalized.is_char_boundary(end) {
        end -= 1;
    }
    normalize_speech_text(&normalized[..end])
}
