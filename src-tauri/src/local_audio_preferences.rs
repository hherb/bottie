//! Rust-owned durable choices for local microphone input and speech playback.

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TOKEN_DIGEST_BYTES: usize = 64;
const MICROPHONE_TOKEN_PREFIX: &str = "local-input-";
const SPEECH_TOKEN_PREFIX: &str = "local-voice-";

/// Fixed persistence failure without filesystem detail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LocalAudioPreferenceError;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalAudioPreferences {
    #[serde(default)]
    microphone_input_id: Option<String>,
    #[serde(default)]
    speech_voice_id: Option<String>,
}

/// Native-only owner for durable opaque local-audio choices.
pub(crate) struct LocalAudioPreferenceStore {
    path: PathBuf,
    preferences: LocalAudioPreferences,
}

impl LocalAudioPreferenceStore {
    /// Loads valid opaque choices and safely falls back when the file is absent or malformed.
    pub(crate) fn load(path: PathBuf) -> Self {
        let preferences = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<LocalAudioPreferences>(&bytes).ok())
            .map(LocalAudioPreferences::normalized)
            .unwrap_or_default();
        Self { path, preferences }
    }

    /// Returns the remembered microphone token without exposing native device identity.
    pub(crate) fn microphone_input_id(&self) -> Option<String> {
        self.preferences.microphone_input_id.clone()
    }

    /// Returns the remembered speech token without exposing the engine voice identity.
    pub(crate) fn speech_voice_id(&self) -> Option<String> {
        self.preferences.speech_voice_id.clone()
    }

    /// Durably replaces the remembered microphone choice; `None` means System default.
    pub(crate) fn remember_microphone(
        &mut self,
        token: Option<String>,
    ) -> Result<(), LocalAudioPreferenceError> {
        self.save(LocalAudioPreferences {
            microphone_input_id: normalized_token(token, MICROPHONE_TOKEN_PREFIX),
            ..self.preferences.clone()
        })
    }

    /// Durably replaces the remembered local speech voice.
    pub(crate) fn remember_speech_voice(
        &mut self,
        token: Option<String>,
    ) -> Result<(), LocalAudioPreferenceError> {
        self.save(LocalAudioPreferences {
            speech_voice_id: normalized_token(token, SPEECH_TOKEN_PREFIX),
            ..self.preferences.clone()
        })
    }

    fn save(
        &mut self,
        preferences: LocalAudioPreferences,
    ) -> Result<(), LocalAudioPreferenceError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| LocalAudioPreferenceError)?;
        }
        let bytes =
            serde_json::to_vec_pretty(&preferences).map_err(|_| LocalAudioPreferenceError)?;
        fs::write(&self.path, bytes).map_err(|_| LocalAudioPreferenceError)?;
        self.preferences = preferences;
        Ok(())
    }
}

impl LocalAudioPreferences {
    fn normalized(self) -> Self {
        Self {
            microphone_input_id: normalized_token(
                self.microphone_input_id,
                MICROPHONE_TOKEN_PREFIX,
            ),
            speech_voice_id: normalized_token(self.speech_voice_id, SPEECH_TOKEN_PREFIX),
        }
    }
}

/// Derives a deterministic Rust-only preference key from one native identity.
pub(crate) fn stable_choice_token(prefix: &str, native_id: &str) -> String {
    format!("{prefix}{:x}", Sha256::digest(native_id.as_bytes()))
}

fn normalized_token(token: Option<String>, prefix: &str) -> Option<String> {
    token.filter(|value| {
        value.len() == prefix.len() + TOKEN_DIGEST_BYTES
            && value.starts_with(prefix)
            && value[prefix.len()..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_survive_reopen_and_invalid_tokens_fall_back() {
        let path = std::env::temp_dir().join(format!(
            "bottie-local-audio-preferences-{}.json",
            std::process::id(),
        ));
        let _ = fs::remove_file(&path);
        let microphone = stable_choice_token(MICROPHONE_TOKEN_PREFIX, "native microphone");
        let speech = stable_choice_token(SPEECH_TOKEN_PREFIX, "native voice");

        let mut store = LocalAudioPreferenceStore::load(path.clone());
        store.remember_microphone(Some(microphone.clone())).unwrap();
        store.remember_speech_voice(Some(speech.clone())).unwrap();
        let reopened = LocalAudioPreferenceStore::load(path.clone());
        assert_eq!(reopened.microphone_input_id(), Some(microphone));
        assert_eq!(reopened.speech_voice_id(), Some(speech));

        fs::write(
            &path,
            br#"{"microphoneInputId":"native identity","speechVoiceId":"local-voice-short"}"#,
        )
        .unwrap();
        let invalid = LocalAudioPreferenceStore::load(path.clone());
        assert_eq!(invalid.microphone_input_id(), None);
        assert_eq!(invalid.speech_voice_id(), None);
        fs::remove_file(path).unwrap();
    }
}
