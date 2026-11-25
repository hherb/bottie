//! Shared capture state: the one record the capture thread, the transcription worker, the Silero gate and the
//! controller share.
//!
//! The capture thread appends samples and feeds the endpointer, the transcription worker applies results, the Silero
//! gate worker reports load failures, and `MicrophoneController` starts captures and reads a path-free
//! `MicrophoneStatus` snapshot. All of them hold the same mutex.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use cpal::{FromSample, Sample};

use super::{
    MAX_CAPTURE_DURATION, MicrophoneErrorCode, MicrophoneMode, MicrophonePermission,
    MicrophonePhase, MicrophoneStatus,
    buffer::CaptureBuffer,
    endpointing::{EndPointer, EndReason},
    latency::MicrophoneLatencyTracker,
    lock,
    transcription::{
        self, RawTranscriptSegment, TARGET_SAMPLE_RATE_HZ, TranscriptSegment,
        TranscriptionErrorCode, TranscriptionJob, TranscriptionPhase,
    },
    vad::{self, VoiceActivity},
    vad_gate::{VadGateSender, VadVerdict, releases_speech},
};

/// One capture slot: phase, retained samples, endpointer, transcription progress and latency, behind one mutex.
#[derive(Default)]
pub(super) struct CaptureState {
    pub(super) phase: MicrophonePhase,
    pub(super) mode: MicrophoneMode,
    pub(super) endpointer: EndPointer,
    pub(super) capture_started_at: Option<Instant>,
    pub(super) first_speech_ms: Option<u64>,
    pub(super) vad_frames_processed: u64,
    /// Latest off-thread Silero verdict for this capture, when conversation mode has a gate worker.
    pub(super) vad_verdict: Option<Arc<Mutex<VadVerdict>>>,
    /// Window sender for the Silero gate worker.
    pub(super) vad_gate: Option<VadGateSender>,
    pub(super) permission: MicrophonePermission,
    pub(super) buffer: Option<CaptureBuffer>,
    pub(super) input_level: f32,
    pub(super) error_code: Option<MicrophoneErrorCode>,
    pub(super) capture_id: u64,
    pub(super) transcription_generation: u64,
    pub(super) applied_transcription_generation: u64,
    pub(super) transcription_phase: TranscriptionPhase,
    pub(super) transcript_segments: Vec<TranscriptSegment>,
    pub(super) transcription_error_code: Option<TranscriptionErrorCode>,
    /// Last Silero load failure in the recognizer and in the release gate. Each worker keeps its own, so one worker's
    /// success cannot hide the other's failure; both outlive a capture until a later load succeeds.
    pub(super) recognizer_vad_error: Option<TranscriptionErrorCode>,
    pub(super) gate_vad_error: Option<TranscriptionErrorCode>,
    pub(super) pending_transcription: Option<TranscriptionJob>,
    pub(super) next_transcription_ms: u64,
    pub(super) latency: MicrophoneLatencyTracker,
}

impl CaptureState {
    pub(super) fn begin_starting(&mut self) {
        self.begin_starting_at(Instant::now());
    }

    pub(super) fn begin_starting_at(&mut self, now: Instant) {
        self.capture_id = self.capture_id.wrapping_add(1);
        self.phase = MicrophonePhase::Starting;
        self.permission = MicrophonePermission::Prompt;
        self.buffer = None;
        self.input_level = 0.0;
        self.error_code = None;
        self.capture_started_at = None;
        self.first_speech_ms = None;
        self.vad_frames_processed = 0;
        self.endpointer.reset();
        self.clear_transcription(TranscriptionPhase::Idle);
        self.latency.begin_capture(now);
    }

    pub(super) fn begin_recording(&mut self, sample_rate_hz: u32, channels: u16) {
        let now = Instant::now();
        self.capture_started_at = Some(now);
        self.first_speech_ms = None;
        self.vad_frames_processed = 0;
        if self.mode == MicrophoneMode::Conversation {
            self.endpointer.arm();
        } else {
            self.endpointer.reset();
        }
        self.phase = MicrophonePhase::Recording;
        self.permission = MicrophonePermission::Granted;
        self.buffer = Some(CaptureBuffer::new(sample_rate_hz, channels));
        self.input_level = 0.0;
        self.error_code = None;
        self.clear_transcription(TranscriptionPhase::Listening);
    }

    pub(super) fn mark_input_ready(&mut self) {
        self.mark_input_ready_at(Instant::now());
    }

    pub(super) fn mark_input_ready_at(&mut self, now: Instant) {
        self.latency.mark_input_ready(now);
    }

    pub(super) fn finish(&mut self) {
        if self.phase != MicrophonePhase::Error && self.phase != MicrophonePhase::Idle {
            self.phase = MicrophonePhase::Captured;
        }
        self.input_level = 0.0;
    }

    pub(super) fn fail(&mut self, code: MicrophoneErrorCode) {
        self.phase = MicrophonePhase::Error;
        self.permission = permission_for_error(code);
        self.input_level = 0.0;
        self.error_code = Some(code);
    }

    pub(super) fn reset(&mut self) {
        let next_capture_id = self.capture_id.wrapping_add(1);
        *self = Self {
            capture_id: next_capture_id,
            ..Self::default()
        };
    }

    pub(super) fn limit_reached(&self) -> bool {
        self.buffer
            .as_ref()
            .is_some_and(CaptureBuffer::limit_reached)
    }

    pub(super) fn append<T>(&mut self, input: &[T])
    where
        T: Copy + Sample,
        f32: FromSample<T>,
    {
        let Some(buffer) = self.buffer.as_mut() else {
            return;
        };
        self.input_level = buffer.append(input);
        if self.mode != MicrophoneMode::Conversation {
            let mut drained = Vec::new();
            if let Some(buffer) = self.buffer.as_mut() {
                let _ = buffer.voice_detector.take_frame_activities(&mut drained);
            }
            return;
        }
        let Some(started_at) = self.capture_started_at else {
            return;
        };
        let mut frames = Vec::new();
        let dropped = self.buffer.as_mut().map_or(0, |buffer| {
            buffer.voice_detector.take_frame_activities(&mut frames)
        });
        for activity in frames {
            self.vad_frames_processed = self.vad_frames_processed.saturating_add(1);
            let at = self.frame_clock(started_at);
            let frame_ms = self
                .vad_frames_processed
                .saturating_mul(u64::from(vad::FRAME_DURATION_MS));
            let activity = self.gated_activity(activity, frame_ms);
            let _ = self.endpointer.on_activity(at, activity);
            self.anchor_transcription_to_onset(started_at);
        }
        // Dropped frames are audio that happened; skipping them would make every endpointer silence timer run long.
        self.vad_frames_processed = self.vad_frames_processed.saturating_add(dropped);
    }

    /// The audio timeline: capture start plus every completed VAD frame, including ones dropped from the trace.
    ///
    /// The endpointer must see one clock. Wall time runs ahead of this timeline by device startup and callback
    /// buffering, and that lag would otherwise count as silence and end a turn while the user is still speaking.
    fn frame_clock(&self, started_at: Instant) -> Instant {
        started_at
            + Duration::from_millis(
                self.vad_frames_processed
                    .saturating_mul(u64::from(vad::FRAME_DURATION_MS)),
            )
    }

    /// Delays release while the off-thread Silero gate still reports speech near this frame.
    pub(super) fn gated_activity(&self, activity: VoiceActivity, frame_ms: u64) -> VoiceActivity {
        if self.mode != MicrophoneMode::Conversation || activity == VoiceActivity::Speech {
            return activity;
        }
        let Some(verdict) = self.vad_verdict.as_ref() else {
            return activity;
        };
        if releases_speech(&lock(verdict), self.capture_id, frame_ms) {
            VoiceActivity::Speech
        } else {
            VoiceActivity::Silence
        }
    }

    /// Copies the trailing window at 16 kHz for the VAD worker, with its capture-relative end.
    pub(super) fn vad_window(&self, window_ms: u64) -> Option<(Vec<f32>, u64)> {
        let buffer = self.buffer.as_ref()?;
        let minimum = usize::try_from(u64::from(buffer.sample_rate_hz) / 4).ok()?;
        if buffer.samples.len() < minimum.max(1) {
            return None;
        }
        let capacity =
            usize::try_from(u64::from(buffer.sample_rate_hz).saturating_mul(window_ms) / 1_000)
                .ok()?;
        let start = buffer.samples.len().saturating_sub(capacity);
        let samples = transcription::resample_linear(
            &buffer.samples[start..],
            buffer.sample_rate_hz,
            TARGET_SAMPLE_RATE_HZ,
        );
        Some((samples, buffer.duration_ms()))
    }

    /// Starts conversation partials a full interval after the first confirmed speech, not after capture start.
    fn anchor_transcription_to_onset(&mut self, started_at: Instant) {
        if self.mode != MicrophoneMode::Conversation || self.first_speech_ms.is_some() {
            return;
        }
        let Some(onset) = self.endpointer.first_speech_at() else {
            return;
        };
        let onset_ms = u64::try_from(onset.saturating_duration_since(started_at).as_millis())
            .unwrap_or(u64::MAX);
        self.first_speech_ms = Some(onset_ms);
        self.next_transcription_ms = onset_ms.saturating_add(self.transcription_interval_ms());
    }

    /// Reports the latched end-of-turn reason for conversation capture.
    pub(super) fn endpoint_finished(&self) -> Option<EndReason> {
        self.endpointer.finished()
    }

    /// Joins the bounded transcript segments into the text an end-of-turn gate evaluates.
    fn transcript_text(&self) -> String {
        self.transcript_segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Conversation capture asks for earlier partials than push-to-talk.
    fn transcription_interval_ms(&self) -> u64 {
        match self.mode {
            MicrophoneMode::Conversation => transcription::CONVERSATION_TRANSCRIPTION_INTERVAL_MS,
            MicrophoneMode::PushToTalk => transcription::TRANSCRIPTION_INTERVAL_MS,
        }
    }

    pub(super) fn schedule_transcription(&mut self, is_final: bool) -> bool {
        self.schedule_transcription_at(is_final, Instant::now())
    }

    pub(super) fn schedule_transcription_at(&mut self, is_final: bool, now: Instant) -> bool {
        let Some(buffer) = self.buffer.as_ref() else {
            return false;
        };
        if is_final {
            self.latency.begin_finalization(now);
        }
        let duration_ms = buffer.duration_ms();
        if !is_final && duration_ms < self.next_transcription_ms {
            return false;
        }
        self.next_transcription_ms = duration_ms.saturating_add(self.transcription_interval_ms());
        let Some(job) = TranscriptionJob::from_capture(
            self.capture_id,
            self.transcription_generation.wrapping_add(1),
            is_final,
            &buffer.samples,
            buffer.sample_rate_hz,
            &buffer.voice_segments(),
        ) else {
            if is_final {
                self.transcription_phase = TranscriptionPhase::Ready;
                self.transcript_segments.clear();
                self.latency.mark_final_transcript(now);
            }
            return false;
        };
        self.transcription_generation = job.generation();
        self.pending_transcription = Some(job);
        self.transcription_phase = TranscriptionPhase::PreparingModel;
        self.transcription_error_code = None;
        true
    }

    pub(super) fn take_pending_transcription(&mut self) -> Option<TranscriptionJob> {
        self.pending_transcription.take()
    }

    pub(super) fn mark_transcribing(&mut self, capture_id: u64, generation: u64) {
        if self.capture_id == capture_id && generation >= self.applied_transcription_generation {
            self.transcription_phase = TranscriptionPhase::Transcribing;
        }
    }

    pub(super) fn apply_transcription(
        &mut self,
        capture_id: u64,
        generation: u64,
        is_final: bool,
        result: Result<Vec<RawTranscriptSegment>, TranscriptionErrorCode>,
    ) {
        self.apply_transcription_at(capture_id, generation, is_final, result, Instant::now());
    }

    pub(super) fn apply_transcription_at(
        &mut self,
        capture_id: u64,
        generation: u64,
        is_final: bool,
        result: Result<Vec<RawTranscriptSegment>, TranscriptionErrorCode>,
        now: Instant,
    ) {
        if self.capture_id != capture_id || generation < self.applied_transcription_generation {
            return;
        }
        self.applied_transcription_generation = generation;
        match result {
            Ok(segments) => {
                self.transcript_segments = transcription::bounded_segments(segments, is_final);
                if !self.transcript_segments.is_empty() {
                    self.latency.mark_first_transcript(now);
                }
                self.transcription_phase = if is_final {
                    TranscriptionPhase::Ready
                } else {
                    TranscriptionPhase::Transcribing
                };
                if is_final {
                    self.latency.mark_final_transcript(now);
                }
                if self.mode == MicrophoneMode::Conversation
                    && !is_final
                    && let Some(started_at) = self.capture_started_at
                {
                    let text = self.transcript_text();
                    let at = self.frame_clock(started_at);
                    let _ = self.endpointer.on_transcript(at, &text);
                }
                self.transcription_error_code = None;
            }
            Err(code) => {
                self.transcription_phase = TranscriptionPhase::Error;
                self.transcription_error_code = Some(code);
            }
        }
    }

    fn clear_transcription(&mut self, phase: TranscriptionPhase) {
        self.transcription_generation = 0;
        self.applied_transcription_generation = 0;
        self.transcription_phase = phase;
        self.transcript_segments.clear();
        self.transcription_error_code = None;
        self.pending_transcription = None;
        self.next_transcription_ms = self.transcription_interval_ms();
    }

    pub(super) fn status(&self) -> MicrophoneStatus {
        let duration_ms = self.buffer.as_ref().map_or(0, CaptureBuffer::duration_ms);
        let max_duration_ms = self.buffer.as_ref().map_or(
            MAX_CAPTURE_DURATION.as_millis() as u64,
            CaptureBuffer::max_duration_ms,
        );
        MicrophoneStatus {
            phase: self.phase,
            mode: self.mode,
            permission: self.permission,
            duration_ms,
            max_duration_ms,
            sample_rate_hz: self.buffer.as_ref().map(|buffer| buffer.sample_rate_hz),
            channels: self.buffer.as_ref().map(|buffer| buffer.source_channels),
            retained_byte_size: self
                .buffer
                .as_ref()
                .map_or(0, CaptureBuffer::retained_byte_size),
            input_level: self.input_level,
            voice_activity: self
                .buffer
                .as_ref()
                .map(|buffer| buffer.voice_detector.activity()),
            voice_segments: self
                .buffer
                .as_ref()
                .map_or_else(Vec::new, CaptureBuffer::voice_segments),
            transcription_phase: self.transcription_phase,
            transcript_segments: self.transcript_segments.clone(),
            transcription_error_code: self.transcription_error_code,
            // The gate only runs, and so only retries, in a conversation; push-to-talk must not show its stale error.
            speech_detection_error_code: match self.mode {
                MicrophoneMode::Conversation => self.gate_vad_error.or(self.recognizer_vad_error),
                MicrophoneMode::PushToTalk => self.recognizer_vad_error,
            },
            error_code: self.error_code,
            latency: self.latency.summary(),
            endpoint_reason: self.endpointer.finished(),
        }
    }
}

fn permission_for_error(code: MicrophoneErrorCode) -> MicrophonePermission {
    match code {
        MicrophoneErrorCode::PermissionDenied => MicrophonePermission::Denied,
        MicrophoneErrorCode::DeviceUnavailable | MicrophoneErrorCode::SelectedDeviceUnavailable => {
            MicrophonePermission::Unavailable
        }
        _ => MicrophonePermission::Prompt,
    }
}
