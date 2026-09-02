//! Bounded opaque metadata for native speech-engine voices.

use crate::local_audio_preferences::stable_choice_token;

use super::{
    MAX_NATIVE_VOICE_ID_BYTES, MAX_SPEECH_VOICES, MAX_VOICE_FIELD_BYTES, NativeSpeechVoice,
    SpeechVoice, normalize_speech_text,
};

pub(super) fn bounded_voices(voices: Vec<NativeSpeechVoice>) -> Vec<(SpeechVoice, String)> {
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

pub(super) fn stable_voice_preference(native_id: &str) -> String {
    stable_choice_token("local-voice-", native_id)
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
