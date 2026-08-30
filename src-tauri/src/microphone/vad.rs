//! Deterministic bounded voice activity detection over native mono PCM.

use serde::Serialize;

const FRAME_DURATION_MS: u32 = 20;
const SPEECH_ONSET_FRAMES: u16 = 3;
const SILENCE_RELEASE_FRAMES: u16 = 10;
const SPEECH_RMS_THRESHOLD: f32 = 0.02;
const SILENCE_RMS_THRESHOLD: f32 = 0.012;
pub(super) const MAX_VOICE_SEGMENTS: usize = 512;

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

    fn classify_frame(&mut self) {
        let rms = (self.frame_energy / self.frame_samples as f64).sqrt() as f32;
        let observed = match self.activity {
            VoiceActivity::Silence if rms >= SPEECH_RMS_THRESHOLD => VoiceActivity::Speech,
            VoiceActivity::Speech if rms <= SILENCE_RMS_THRESHOLD => VoiceActivity::Silence,
            activity => activity,
        };
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
