//! Bounded mono PCM accumulation for one native microphone capture.

use std::time::Duration;

use cpal::{FromSample, Sample};

use super::{FLOAT_SAMPLE_BYTES, MAX_CAPTURE_DURATION, MAX_RETAINED_BYTES, VoiceActivityDetector};

/// Native-only mono samples and their streaming voice-activity detector.
pub(super) struct CaptureBuffer {
    pub(super) samples: Vec<f32>,
    pub(super) sample_rate_hz: u32,
    pub(super) source_channels: u16,
    pub(super) max_samples: usize,
    pub(super) voice_detector: VoiceActivityDetector,
}

impl CaptureBuffer {
    /// Creates one buffer under the product capture-duration and memory ceilings.
    pub(super) fn new(sample_rate_hz: u32, source_channels: u16) -> Self {
        Self::with_limits(
            sample_rate_hz,
            source_channels,
            MAX_CAPTURE_DURATION,
            MAX_RETAINED_BYTES,
        )
    }

    /// Creates a buffer with explicit limits for focused boundary tests.
    pub(super) fn with_limits(
        sample_rate_hz: u32,
        source_channels: u16,
        duration: Duration,
        max_bytes: usize,
    ) -> Self {
        let duration_samples = u64::from(sample_rate_hz)
            .saturating_mul(duration.as_millis() as u64)
            .saturating_div(1_000);
        let max_samples = usize::try_from(duration_samples)
            .unwrap_or(usize::MAX)
            .min(max_bytes / FLOAT_SAMPLE_BYTES);
        Self {
            samples: Vec::with_capacity(max_samples),
            sample_rate_hz,
            source_channels,
            max_samples,
            voice_detector: VoiceActivityDetector::new(sample_rate_hz),
        }
    }

    /// Downmixes complete interleaved frames, normalizes invalid samples, and returns the peak level.
    pub(super) fn append<T>(&mut self, input: &[T]) -> f32
    where
        T: Copy + Sample,
        f32: FromSample<T>,
    {
        let channels = usize::from(self.source_channels.max(1));
        let remaining = self.max_samples.saturating_sub(self.samples.len());
        let mut peak = 0.0_f32;
        for frame in input.chunks_exact(channels).take(remaining) {
            let mono = frame
                .iter()
                .map(|sample| f32::from_sample(*sample))
                .sum::<f32>()
                / channels as f32;
            let mono = if mono.is_finite() {
                mono.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            peak = peak.max(mono.abs());
            self.samples.push(mono);
            self.voice_detector.push(mono);
        }
        peak
    }

    /// Reports whether either configured storage limit has stopped capture.
    pub(super) fn limit_reached(&self) -> bool {
        self.samples.len() >= self.max_samples
    }

    /// Converts retained sample count into path-free capture duration.
    pub(super) fn duration_ms(&self) -> u64 {
        (self.samples.len() as u64)
            .saturating_mul(1_000)
            .saturating_div(u64::from(self.sample_rate_hz.max(1)))
    }
}
