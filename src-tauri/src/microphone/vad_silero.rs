//! Silero voice activity detection over the pinned whisper.cpp VAD ggml asset.
//!
//! The recognizer uses this module to decide whether a bounded snapshot contains speech at all and which part of it
//! does, so silence and room noise become an honest empty transcript instead of a Whisper hallucination. The model is
//! verified through the same pinned-release gate as the recognition checkpoint.

use std::{
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};

use whisper_rs::{WhisperVadContext, WhisperVadContextParams, WhisperVadParams};

use super::transcription::{TranscriptionErrorCode, prepare_pinned_vad_model};

const VAD_RUNTIME_THREADS: i32 = 1;
const VAD_THRESHOLD: f32 = 0.5;
/// Shortest run the model will call speech; filters clicks and single-frame noise.
const MIN_SPEECH_MS: i32 = 250;
/// Silence this long ends a speech segment inside the window.
const MIN_SILENCE_MS: i32 = 300;
/// Trimming uses the segment edges exactly; the turn detector owns conversational padding.
const SPEECH_PAD_MS: i32 = 0;
/// The model needs more than one frame; shorter snapshots are treated as empty.
const MIN_WINDOW_SAMPLES: usize = 512;

/// The span of a recognizer snapshot that contains speech, in milliseconds from its start.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SpeechWindow {
    pub(super) start_ms: u64,
    pub(super) end_ms: u64,
}

/// One Silero VAD context; inference is serialized by the caller's mutex.
pub(super) struct SileroVad {
    context: WhisperVadContext,
}

impl SileroVad {
    /// Loads the pinned VAD model from Bottie's app-owned cache.
    pub(super) fn load(cache_dir: &PathBuf) -> Result<Self, TranscriptionErrorCode> {
        let model_path = prepare_pinned_vad_model(cache_dir)?;
        let mut params = WhisperVadContextParams::default();
        params.set_n_threads(VAD_RUNTIME_THREADS);
        params.set_use_gpu(false);
        let context = WhisperVadContext::new(model_path.to_string_lossy().as_ref(), params)
            .map_err(|_| TranscriptionErrorCode::RecognitionFailed)?;
        Ok(Self { context })
    }

    /// Returns the speech span of one 16 kHz snapshot, `None` when it contains no speech, or `Err` when inference
    /// failed, which must never be mistaken for silence.
    pub(super) fn speech_window(
        &mut self,
        samples_16k: &[f32],
    ) -> Result<Option<SpeechWindow>, ()> {
        if samples_16k.len() < MIN_WINDOW_SAMPLES {
            return Ok(None);
        }
        let mut params = WhisperVadParams::default();
        params.set_threshold(VAD_THRESHOLD);
        params.set_min_speech_duration(MIN_SPEECH_MS);
        params.set_min_silence_duration(MIN_SILENCE_MS);
        params.set_speech_pad(SPEECH_PAD_MS);
        let segments = self
            .context
            .segments_from_samples(params, samples_16k)
            .map_err(|_| ())?;
        Ok(merge_segments(segments.into_iter().map(|segment| {
            (f64::from(segment.start), f64::from(segment.end))
        })))
    }
}

/// Locks the VAD context, recovering from a poisoned lock instead of losing recognition silently.
pub(super) fn lock_vad(vad: &Mutex<SileroVad>) -> MutexGuard<'_, SileroVad> {
    vad.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Joins every detected segment into the single span a recognizer should transcribe.
fn merge_segments(segments: impl Iterator<Item = (f64, f64)>) -> Option<SpeechWindow> {
    let mut first: Option<u64> = None;
    let mut last = 0_u64;
    for (start, end) in segments {
        if !start.is_finite() || !end.is_finite() || end <= start {
            continue;
        }
        // whisper.cpp reports VAD segment timestamps in centiseconds.
        let start_ms = centiseconds_to_milliseconds(start);
        let end_ms = centiseconds_to_milliseconds(end);
        first = Some(first.map_or(start_ms, |current| current.min(start_ms)));
        last = last.max(end_ms);
    }
    let start_ms = first?;
    (last > start_ms).then_some(SpeechWindow {
        start_ms,
        end_ms: last,
    })
}

/// Converts a sample index range for one millisecond window at an arbitrary rate.
pub(super) fn speech_sample_bounds(
    total_samples: usize,
    sample_rate_hz: u32,
    window: SpeechWindow,
) -> Option<(usize, usize)> {
    let rate = u64::from(sample_rate_hz.max(1));
    let start = window.start_ms.saturating_mul(rate) / 1_000;
    let end = window.end_ms.saturating_mul(rate) / 1_000;
    let start = usize::try_from(start).ok()?.min(total_samples);
    let end = usize::try_from(end).ok()?.min(total_samples);
    (end > start).then_some((start, end))
}

fn centiseconds_to_milliseconds(value: f64) -> u64 {
    (value.max(0.0) * 10.0).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_segments_into_one_contiguous_span() {
        assert_eq!(
            merge_segments([(0.0, 12.0), (12.0, 25.0), (40.0, 50.0)].into_iter()),
            Some(SpeechWindow {
                start_ms: 0,
                end_ms: 500
            })
        );
    }

    #[test]
    fn ignores_empty_or_non_finite_segments() {
        assert_eq!(merge_segments(std::iter::empty()), None);
        assert_eq!(
            merge_segments([(10.0, 10.0), (f64::NAN, 5.0)].into_iter()),
            None
        );
    }

    #[test]
    fn converts_a_window_to_a_clamped_sample_range() {
        let window = SpeechWindow {
            start_ms: 100,
            end_ms: 500,
        };
        assert_eq!(
            speech_sample_bounds(16_000, 16_000, window),
            Some((1_600, 8_000))
        );
        // A window beyond the snapshot clamps to its end and can become empty.
        assert_eq!(
            speech_sample_bounds(4_000, 16_000, window),
            Some((1_600, 4_000))
        );
        assert_eq!(speech_sample_bounds(100, 16_000, window), None);
    }
}
