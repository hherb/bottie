//! Validate ordered, bounded Nano PCM and exact model identity before native playback.

use crate::speech::SpeechErrorCode;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::Value;

pub(super) const SOURCE_RATE: u32 = 48_000;
pub(super) const MAX_EVENT_BYTES: usize = 600 * 1024;
const MAX_PCM_BYTES: usize = SOURCE_RATE as usize * 2 * size_of::<f32>();
const MAX_REQUEST_TEXT_BYTES: usize = 2_000;

/// Counts only successfully validated audio within one worker request.
#[derive(Default)]
pub(super) struct Decoder {
    sequence: u64,
    frames: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AudioEvent {
    event: String,
    sequence: u64,
    sample_rate: u32,
    channels: u16,
    samples_per_channel: u64,
    pcm: String,
}

impl Decoder {
    /// Rejects invalid format, ordering, alignment, counts, and nonfinite float samples before buffering.
    pub(super) fn audio(&mut self, event: Value) -> Result<Vec<[f32; 2]>, SpeechErrorCode> {
        let event: AudioEvent =
            serde_json::from_value(event).map_err(|_| SpeechErrorCode::PlaybackFailed)?;
        if event.event != "audio"
            || event.sequence != self.sequence
            || event.sample_rate != SOURCE_RATE
            || event.channels != 2
            || event.pcm.len() > MAX_PCM_BYTES * 4 / 3 + 4
        {
            return Err(SpeechErrorCode::PlaybackFailed);
        }
        let bytes = STANDARD
            .decode(event.pcm)
            .map_err(|_| SpeechErrorCode::PlaybackFailed)?;
        if bytes.is_empty()
            || bytes.len() > MAX_PCM_BYTES
            || bytes.len() % 8 != 0
            || bytes.len() as u64 / 8 != event.samples_per_channel
        {
            return Err(SpeechErrorCode::PlaybackFailed);
        }
        let samples = bytes
            .chunks_exact(8)
            .map(|frame| {
                let left = f32::from_le_bytes(frame[..4].try_into().unwrap());
                let right = f32::from_le_bytes(frame[4..].try_into().unwrap());
                if !left.is_finite() || !right.is_finite() {
                    return Err(SpeechErrorCode::PlaybackFailed);
                }
                Ok([left.clamp(-1.0, 1.0), right.clamp(-1.0, 1.0)])
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.sequence += 1;
        self.frames += event.samples_per_channel;
        Ok(samples)
    }

    /// EOF, an empty utterance, or mismatched completion counts cannot be treated as success.
    pub(super) fn complete(&self, event: &Value) -> Result<(), SpeechErrorCode> {
        let duration = event["audioSeconds"]
            .as_f64()
            .ok_or(SpeechErrorCode::PlaybackFailed)?;
        if self.frames == 0
            || event["audioChunks"].as_u64() != Some(self.sequence)
            || (duration - self.frames as f64 / f64::from(SOURCE_RATE)).abs() > 1e-6
        {
            return Err(SpeechErrorCode::PlaybackFailed);
        }
        Ok(())
    }
}

/// Bind CPU readiness to the exact model/codec/code exercised in the accepted trial.
pub(super) fn validate_ready(event: &Value) -> Result<(), SpeechErrorCode> {
    let lock: Value = serde_json::from_str(include_str!(
        "../../../../scripts/moss-nano-spike/assets.lock.json"
    ))
    .map_err(|_| SpeechErrorCode::Unavailable)?;
    if event["event"] != "ready"
        || event["protocolVersion"] != 1
        || event["torchImported"] != false
        || event["telemetryDisabled"] != true
        || event["executionProviders"] != serde_json::json!(["CPUExecutionProvider"])
        || ["sourceRevision", "modelRevision", "codecRevision"]
            .iter()
            .any(|key| event[key] != lock[key])
    {
        return Err(SpeechErrorCode::Unavailable);
    }
    let voices = event["voices"]
        .as_array()
        .ok_or(SpeechErrorCode::Unavailable)?;
    if !["Ava", "Adam"]
        .iter()
        .all(|voice| voices.iter().any(|value| value == voice))
    {
        return Err(SpeechErrorCode::Unavailable);
    }
    Ok(())
}

/// Split a controller-bounded utterance into worker requests without breaking UTF-8 characters.
pub(super) fn request_chunks(text: &str) -> Vec<String> {
    let mut rest = text.trim();
    let mut chunks = Vec::new();
    while !rest.is_empty() {
        let mut end = rest.len().min(MAX_REQUEST_TEXT_BYTES);
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        if end < rest.len() {
            end = rest[..end]
                .rfind(char::is_whitespace)
                .filter(|value| *value > 0)
                .unwrap_or(end);
        }
        chunks.push(rest[..end].trim().to_owned());
        rest = rest[end..].trim_start();
    }
    chunks
}
