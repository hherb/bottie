//! Bounded conversion of one stopped native capture into provider-ready mono WAV bytes.

use super::{
    CaptureBuffer, CaptureState, MicrophoneController, MicrophonePhase, VoiceSegment, lock,
};

const WAV_HEADER_BYTES: usize = 44;
const PCM_BITS_PER_SAMPLE: u16 = 16;
const PCM_BYTES_PER_SAMPLE: u16 = PCM_BITS_PER_SAMPLE / 8;
const MONO_CHANNELS: u16 = 1;

/// Native audio encoding accepted by Bottie's provider-neutral content block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CapturedAudioFormat {
    /// RIFF/WAVE containing little-endian signed 16-bit mono PCM.
    Wav,
}

/// One bounded capture snapshot whose bytes never cross the WebView boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CapturedAudio {
    /// Encoding forwarded only through a compatible native provider adapter.
    pub(crate) format: CapturedAudioFormat,
    /// Complete bounded encoded bytes owned by Rust.
    pub(crate) bytes: Vec<u8>,
    /// Original capture duration exposed only as path-free metadata when needed.
    pub(crate) duration_ms: u64,
    /// Mono PCM sample rate retained in the WAV header.
    pub(crate) sample_rate_hz: u32,
}

/// Fixed preparation failure that does not expose samples or device details.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CapturedAudioError {
    /// No non-empty stopped capture is available for explicit delivery.
    Unavailable,
    /// The bounded capture could not be represented in the WAV container.
    EncodingFailed,
}

impl CaptureState {
    /// Encodes exactly one stopped non-empty capture without mutating session state.
    pub(super) fn captured_audio(&self) -> Result<CapturedAudio, CapturedAudioError> {
        if self.phase != MicrophonePhase::Captured {
            return Err(CapturedAudioError::Unavailable);
        }
        let buffer = self
            .buffer
            .as_ref()
            .filter(|buffer| !buffer.samples.is_empty())
            .ok_or(CapturedAudioError::Unavailable)?;
        let bytes = encode_wav(&buffer.samples, buffer.sample_rate_hz)?;
        Ok(CapturedAudio {
            format: CapturedAudioFormat::Wav,
            bytes,
            duration_ms: buffer.duration_ms(),
            sample_rate_hz: buffer.sample_rate_hz,
        })
    }
}

impl MicrophoneController {
    /// Copies one stopped capture into a bounded native WAV content block.
    pub(crate) fn captured_audio(&self) -> Result<CapturedAudio, CapturedAudioError> {
        lock(&self.shared).captured_audio()
    }
}

impl CaptureBuffer {
    /// Returns the effective duration ceiling after memory and time bounds are combined.
    pub(super) fn max_duration_ms(&self) -> u64 {
        (self.max_samples as u64)
            .saturating_mul(1_000)
            .saturating_div(u64::from(self.sample_rate_hz.max(1)))
    }

    /// Returns the exact in-memory sample footprint without serializing sample values.
    pub(super) fn retained_byte_size(&self) -> u64 {
        self.samples.len().saturating_mul(size_of::<f32>()) as u64
    }

    /// Returns bounded path-free voice activity timing for the retained sample range.
    pub(super) fn voice_segments(&self) -> Vec<VoiceSegment> {
        self.voice_detector.segments(self.samples.len())
    }
}

/// Encodes normalized mono floats into a canonical PCM16 WAV container.
fn encode_wav(samples: &[f32], sample_rate_hz: u32) -> Result<Vec<u8>, CapturedAudioError> {
    let data_bytes = samples
        .len()
        .checked_mul(usize::from(PCM_BYTES_PER_SAMPLE))
        .and_then(|bytes| u32::try_from(bytes).ok())
        .ok_or(CapturedAudioError::EncodingFailed)?;
    let riff_bytes = data_bytes
        .checked_add((WAV_HEADER_BYTES - 8) as u32)
        .ok_or(CapturedAudioError::EncodingFailed)?;
    let byte_rate = sample_rate_hz
        .checked_mul(u32::from(PCM_BYTES_PER_SAMPLE))
        .ok_or(CapturedAudioError::EncodingFailed)?;
    let capacity = WAV_HEADER_BYTES
        .checked_add(data_bytes as usize)
        .ok_or(CapturedAudioError::EncodingFailed)?;
    let mut wav = Vec::with_capacity(capacity);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_bytes.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&MONO_CHANNELS.to_le_bytes());
    wav.extend_from_slice(&sample_rate_hz.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&PCM_BYTES_PER_SAMPLE.to_le_bytes());
    wav.extend_from_slice(&PCM_BITS_PER_SAMPLE.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());
    for sample in samples {
        let normalized = if sample.is_finite() {
            sample.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        let pcm = (normalized * f32::from(i16::MAX)).round() as i16;
        wav.extend_from_slice(&pcm.to_le_bytes());
    }
    Ok(wav)
}
