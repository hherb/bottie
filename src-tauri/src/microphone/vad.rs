//! Deterministic bounded voice activity detection over native mono PCM.

use serde::Serialize;

pub(super) const FRAME_DURATION_MS: u32 = 20;
const SPEECH_ONSET_FRAMES: u16 = 3;
const SILENCE_RELEASE_FRAMES: u16 = 10;
/// Quiet-room fallback thresholds. A built-in microphone with normal gain often produces speech RMS far below
/// a fixed 0.02 threshold, so the detector adapts to a tracked noise floor and only falls back to these when it is quiet.
const MIN_SPEECH_RMS: f32 = 0.003;
const MIN_SILENCE_RMS: f32 = 0.0018;
/// Speech has to stand this far above the tracked noise floor; released silence falls this far below it.
const SPEECH_NOISE_RATIO: f32 = 3.0;
const SILENCE_NOISE_RATIO: f32 = 1.8;
/// Slow per-frame adaptation so a burst of speech cannot deafen the detector.
const NOISE_FLOOR_ADAPTATION: f32 = 0.02;
const MIN_NOISE_FLOOR_RMS: f32 = 0.0002;
const MAX_NOISE_FLOOR_RMS: f32 = 0.02;
const MAX_SPEECH_RMS: f32 = 0.2;
pub(super) const MAX_VOICE_SEGMENTS: usize = 512;
/// One completed frame activity per 20 ms; the capture callback drains these immediately.
pub(super) const MAX_PENDING_FRAME_ACTIVITIES: usize = 64;

/// Stable activity classification safe to expose without detector internals.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum VoiceActivity {
    /// The bounded native detector currently classifies the input as silence.
    Silence,
    /// The bounded native detector currently classifies the input as speech.
    Speech,
}

/// One path-free contiguous activity range within the current capture.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VoiceSegment {
    pub(super) activity: VoiceActivity,
    pub(super) start_ms: u64,
    pub(super) end_ms: u64,
}

/// Streaming RMS detector with separate speech-onset and silence-release confirmation.
pub(super) struct VoiceActivityDetector {
    sample_rate_hz: u32,
    frame_samples: usize,
    frame_energy: f64,
    frame_sample_count: usize,
    processed_samples: usize,
    activity: VoiceActivity,
    current_start_sample: usize,
    candidate: Option<ActivityCandidate>,
    segments: Vec<SampleSegment>,
    frame_activities: Vec<VoiceActivity>,
    /// Slowly tracked ambient level used to scale the speech and silence thresholds.
    noise_floor_rms: f32,
}

struct ActivityCandidate {
    activity: VoiceActivity,
    start_sample: usize,
    frame_count: u16,
}

struct SampleSegment {
    activity: VoiceActivity,
    start_sample: usize,
    end_sample: usize,
}

impl VoiceActivityDetector {
    /// Creates one detector whose frame timing follows the native input sample rate.
    pub(super) fn new(sample_rate_hz: u32) -> Self {
        let frame_samples = usize::try_from(
            u64::from(sample_rate_hz)
                .saturating_mul(u64::from(FRAME_DURATION_MS))
                .saturating_div(1_000),
        )
        .unwrap_or(usize::MAX)
        .max(1);
        Self {
            sample_rate_hz,
            frame_samples,
            frame_energy: 0.0,
            frame_sample_count: 0,
            processed_samples: 0,
            activity: VoiceActivity::Silence,
            current_start_sample: 0,
            candidate: None,
            segments: Vec::new(),
            frame_activities: Vec::new(),
            noise_floor_rms: MIN_NOISE_FLOOR_RMS,
        }
    }

    /// Adds one normalized mono sample without retaining another copy of the audio.
    pub(super) fn push(&mut self, sample: f32) {
        let sample = f64::from(sample);
        self.frame_energy += sample * sample;
        self.frame_sample_count += 1;
        self.processed_samples += 1;
        if self.frame_sample_count == self.frame_samples {
            self.classify_frame();
            self.frame_energy = 0.0;
            self.frame_sample_count = 0;
        }
    }

    /// Returns the latest confirmed activity classification.
    pub(super) fn activity(&self) -> VoiceActivity {
        self.activity
    }

    /// Materializes bounded millisecond segments through the current retained sample.
    pub(super) fn segments(&self, retained_samples: usize) -> Vec<VoiceSegment> {
        let mut segments = self
            .segments
            .iter()
            .map(|segment| self.millisecond_segment(segment))
            .collect::<Vec<_>>();
        if retained_samples > self.current_start_sample {
            segments.push(self.millisecond_segment(&SampleSegment {
                activity: self.activity,
                start_sample: self.current_start_sample,
                end_sample: retained_samples,
            }));
        }
        segments
    }

    /// Drains the confirmed classification of every 20 ms frame completed since the last call.
    ///
    /// One entry is produced per completed frame, in order, so a caller can drive end-of-turn logic at the detector's
    /// own cadence instead of at callback cadence. The trace is bounded and drops the newest frames (never queued
    /// unboundedly) if a caller stops draining it.
    pub(super) fn take_frame_activities(&mut self, activities: &mut Vec<VoiceActivity>) {
        activities.append(&mut self.frame_activities);
    }

    fn classify_frame(&mut self) {
        self.classify_frame_inner();
        if self.frame_activities.len() < MAX_PENDING_FRAME_ACTIVITIES {
            self.frame_activities.push(self.activity);
        }
    }

    fn classify_frame_inner(&mut self) {
        let rms = (self.frame_energy / self.frame_samples as f64).sqrt() as f32;
        let noise_floor = self
            .noise_floor_rms
            .clamp(MIN_NOISE_FLOOR_RMS, MAX_NOISE_FLOOR_RMS);
        let speech_threshold = (noise_floor * SPEECH_NOISE_RATIO)
            .max(MIN_SPEECH_RMS)
            .min(MAX_SPEECH_RMS);
        let silence_threshold = (noise_floor * SILENCE_NOISE_RATIO)
            .max(MIN_SILENCE_RMS)
            .min(speech_threshold);
        let observed = match self.activity {
            VoiceActivity::Silence if rms >= speech_threshold => VoiceActivity::Speech,
            VoiceActivity::Speech if rms <= silence_threshold => VoiceActivity::Silence,
            activity => activity,
        };
        if observed == VoiceActivity::Silence {
            self.noise_floor_rms = (self.noise_floor_rms
                + (rms - self.noise_floor_rms) * NOISE_FLOOR_ADAPTATION)
                .clamp(MIN_NOISE_FLOOR_RMS, MAX_NOISE_FLOOR_RMS);
        }
        if observed == self.activity {
            self.candidate = None;
            return;
        }

        let frame_start = self.processed_samples.saturating_sub(self.frame_samples);
        match self.candidate.as_mut() {
            Some(candidate) if candidate.activity == observed => candidate.frame_count += 1,
            _ => {
                self.candidate = Some(ActivityCandidate {
                    activity: observed,
                    start_sample: frame_start,
                    frame_count: 1,
                });
            }
        }
        let required_frames = match observed {
            VoiceActivity::Speech => SPEECH_ONSET_FRAMES,
            VoiceActivity::Silence => SILENCE_RELEASE_FRAMES,
        };
        if self
            .candidate
            .as_ref()
            .is_some_and(|candidate| candidate.frame_count >= required_frames)
        {
            self.confirm_candidate();
        }
    }

    fn confirm_candidate(&mut self) {
        let Some(candidate) = self.candidate.take() else {
            return;
        };
        if self.segments.len() >= MAX_VOICE_SEGMENTS.saturating_sub(1) {
            return;
        }
        if candidate.start_sample > self.current_start_sample {
            self.segments.push(SampleSegment {
                activity: self.activity,
                start_sample: self.current_start_sample,
                end_sample: candidate.start_sample,
            });
        }
        self.activity = candidate.activity;
        self.current_start_sample = candidate.start_sample;
    }

    fn millisecond_segment(&self, segment: &SampleSegment) -> VoiceSegment {
        VoiceSegment {
            activity: segment.activity,
            start_ms: samples_to_milliseconds(segment.start_sample, self.sample_rate_hz),
            end_ms: samples_to_milliseconds(segment.end_sample, self.sample_rate_hz),
        }
    }
}

fn samples_to_milliseconds(samples: usize, sample_rate_hz: u32) -> u64 {
    (samples as u64)
        .saturating_mul(1_000)
        .saturating_div(u64::from(sample_rate_hz.max(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn frame_sample_count() -> usize {
        (RATE as usize * FRAME_DURATION_MS as usize) / 1_000
    }

    fn feed(detector: &mut VoiceActivityDetector, amplitude: f32, frames: usize) {
        for _ in 0..frames {
            for _ in 0..frame_sample_count() {
                detector.push(amplitude);
            }
        }
    }

    #[test]
    fn constant_silence_stays_silent() {
        let mut detector = VoiceActivityDetector::new(RATE);
        feed(&mut detector, 0.0, 50);
        assert_eq!(detector.activity(), VoiceActivity::Silence);
    }

    #[test]
    fn quiet_room_noise_is_not_speech() {
        let mut detector = VoiceActivityDetector::new(RATE);
        feed(&mut detector, 0.001, 30);
        assert_eq!(detector.activity(), VoiceActivity::Silence);
    }

    #[test]
    fn ordinary_speech_on_a_quiet_mic_confirms_after_the_onset_window() {
        let mut detector = VoiceActivityDetector::new(RATE);
        feed(&mut detector, 0.001, 20);
        assert_eq!(detector.activity(), VoiceActivity::Silence);
        feed(&mut detector, 0.01, 4);
        assert_eq!(detector.activity(), VoiceActivity::Speech);
    }

    /// A fixed 0.02 RMS threshold never fires on a built-in mic at normal gain; the adaptive floor must.
    #[test]
    fn quiet_speech_above_the_tracked_noise_floor_is_detected() {
        let mut detector = VoiceActivityDetector::new(RATE);
        feed(&mut detector, 0.0008, 30);
        assert_eq!(detector.activity(), VoiceActivity::Silence);
        feed(&mut detector, 0.004, 4);
        assert_eq!(detector.activity(), VoiceActivity::Speech);
    }

    #[test]
    fn release_still_requires_the_full_silence_window() {
        let mut detector = VoiceActivityDetector::new(RATE);
        feed(&mut detector, 0.01, 4);
        assert_eq!(detector.activity(), VoiceActivity::Speech);
        feed(&mut detector, 0.0, 9);
        assert_eq!(detector.activity(), VoiceActivity::Speech);
        feed(&mut detector, 0.0, 1);
        assert_eq!(detector.activity(), VoiceActivity::Silence);
    }

    #[test]
    fn the_noise_floor_is_bounded_so_a_loud_room_cannot_deafen_the_detector() {
        let mut detector = VoiceActivityDetector::new(RATE);
        feed(&mut detector, 0.03, 50);
        let floor = detector.noise_floor_rms;
        assert!((MIN_NOISE_FLOOR_RMS..=MAX_NOISE_FLOOR_RMS).contains(&floor));
    }
}
