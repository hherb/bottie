//! Bounded native microphone capture with path-free WebView status.

mod capture;
mod transcription;
mod vad;

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard,
        mpsc::{self, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use cpal::{FromSample, Sample};
use serde::Serialize;

use capture::capture_worker;
use transcription::{
    RawTranscriptSegment, TranscriptSegment, TranscriptionErrorCode, TranscriptionJob,
    TranscriptionPhase, TranscriptionWorker,
};
use vad::{VoiceActivity, VoiceActivityDetector, VoiceSegment};

const MAX_CAPTURE_DURATION: Duration = Duration::from_secs(60);
const MAX_RETAINED_BYTES: usize = 32 * 1024 * 1024;
const CAPTURE_WORKER_POLL: Duration = Duration::from_millis(40);
const FLOAT_SAMPLE_BYTES: usize = size_of::<f32>();

/// User-visible lifecycle for one session-only native voice capture.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MicrophonePhase {
    /// No capture exists and the operating system has not been asked by this session.
    #[default]
    Idle,
    /// The native worker is opening the default input and may show an operating-system prompt.
    Starting,
    /// Native PCM samples are being retained within the fixed in-memory limits.
    Recording,
    /// A stopped capture remains in native memory until discarded or replaced.
    Captured,
    /// Capture could not start or the active input stream stopped unexpectedly.
    Error,
}

/// Path-free operating-system authorization state for the current microphone action.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MicrophonePermission {
    /// The app has not yet observed an operating-system decision.
    #[default]
    Prompt,
    /// The input stream started successfully after the user-requested action.
    Granted,
    /// The operating system refused access.
    Denied,
    /// No usable input device or audio host is available.
    Unavailable,
}

/// Stable capture failures that omit backend messages and device identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MicrophoneErrorCode {
    /// Operating-system privacy settings denied microphone access.
    PermissionDenied,
    /// No usable default input device or audio host exists.
    DeviceUnavailable,
    /// Another process or stream currently owns the input.
    DeviceBusy,
    /// The default input uses a sample format outside Bottie's PCM boundary.
    UnsupportedFormat,
    /// A redacted backend, resource, or stream failure stopped capture.
    CaptureFailed,
}

/// Bounded metadata exposed to the WebView without samples or device identity.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MicrophoneStatus {
    phase: MicrophonePhase,
    permission: MicrophonePermission,
    duration_ms: u64,
    max_duration_ms: u64,
    sample_rate_hz: Option<u32>,
    channels: Option<u16>,
    retained_byte_size: u64,
    input_level: f32,
    voice_activity: Option<VoiceActivity>,
    voice_segments: Vec<VoiceSegment>,
    transcription_phase: TranscriptionPhase,
    transcript_segments: Vec<TranscriptSegment>,
    transcription_error_code: Option<TranscriptionErrorCode>,
    error_code: Option<MicrophoneErrorCode>,
}

/// Thread-safe controller for the one session-only native capture slot.
pub(crate) struct MicrophoneController {
    shared: Arc<Mutex<CaptureState>>,
    worker: Mutex<Worker>,
    transcription: Option<TranscriptionWorker>,
}

#[derive(Default)]
struct Worker {
    commands: Option<Sender<CaptureCommand>>,
    handle: Option<JoinHandle<()>>,
}

enum CaptureCommand {
    Stop,
    Discard,
}

#[derive(Default)]
struct CaptureState {
    phase: MicrophonePhase,
    permission: MicrophonePermission,
    buffer: Option<CaptureBuffer>,
    input_level: f32,
    error_code: Option<MicrophoneErrorCode>,
    capture_id: u64,
    transcription_generation: u64,
    applied_transcription_generation: u64,
    transcription_phase: TranscriptionPhase,
    transcript_segments: Vec<TranscriptSegment>,
    transcription_error_code: Option<TranscriptionErrorCode>,
    pending_transcription: Option<TranscriptionJob>,
    next_transcription_ms: u64,
}

struct CaptureBuffer {
    samples: Vec<f32>,
    sample_rate_hz: u32,
    source_channels: u16,
    max_samples: usize,
    voice_detector: VoiceActivityDetector,
}

impl Default for MicrophoneController {
    fn default() -> Self {
        Self {
            shared: Arc::new(Mutex::new(CaptureState::default())),
            worker: Mutex::new(Worker::default()),
            transcription: None,
        }
    }
}

impl MicrophoneController {
    /// Starts the process-lifetime local speech worker without loading its model before user capture.
    pub(crate) fn new(model_cache_path: PathBuf) -> Self {
        let shared = Arc::new(Mutex::new(CaptureState::default()));
        let transcription = TranscriptionWorker::start(model_cache_path, shared.clone());
        Self {
            shared,
            worker: Mutex::new(Worker::default()),
            transcription: Some(transcription),
        }
    }

    /// Returns the current path-free capture state.
    pub(crate) fn status(&self) -> MicrophoneStatus {
        lock(&self.shared).status()
    }

    /// Starts one default-input capture only after the WebView's explicit user action.
    pub(crate) fn start(&self) -> MicrophoneStatus {
        let mut worker = lock(&self.worker);
        clear_finished_worker(&mut worker);
        if worker.handle.is_some() {
            return self.status();
        }

        lock(&self.shared).begin_starting();
        let (commands, receiver) = mpsc::channel();
        let shared = self.shared.clone();
        let transcription_wake = self
            .transcription
            .as_ref()
            .map(TranscriptionWorker::wake_handle);
        match thread::Builder::new()
            .name("bottie-microphone".into())
            .spawn(move || capture_worker(shared, receiver, transcription_wake))
        {
            Ok(handle) => {
                worker.commands = Some(commands);
                worker.handle = Some(handle);
            }
            Err(_) => lock(&self.shared).fail(MicrophoneErrorCode::CaptureFailed),
        }
        self.status()
    }

    /// Stops an active stream while retaining its bounded samples in native memory.
    pub(crate) fn stop(&self) -> MicrophoneStatus {
        if let Some(commands) = lock(&self.worker).commands.as_ref() {
            let _ = commands.send(CaptureCommand::Stop);
        }
        self.status()
    }

    /// Stops any active stream and irreversibly clears the session-only sample buffer.
    pub(crate) fn discard(&self) -> MicrophoneStatus {
        let worker = lock(&self.worker);
        if let Some(commands) = worker.commands.as_ref() {
            let _ = commands.send(CaptureCommand::Discard);
        }
        lock(&self.shared).reset();
        self.status()
    }
}

impl CaptureState {
    fn begin_starting(&mut self) {
        self.capture_id = self.capture_id.wrapping_add(1);
        self.phase = MicrophonePhase::Starting;
        self.permission = MicrophonePermission::Prompt;
        self.buffer = None;
        self.input_level = 0.0;
        self.error_code = None;
        self.clear_transcription(TranscriptionPhase::Idle);
    }

    fn begin_recording(&mut self, sample_rate_hz: u32, channels: u16) {
        self.phase = MicrophonePhase::Recording;
        self.permission = MicrophonePermission::Granted;
        self.buffer = Some(CaptureBuffer::new(sample_rate_hz, channels));
        self.input_level = 0.0;
        self.error_code = None;
        self.clear_transcription(TranscriptionPhase::Listening);
    }

    fn finish(&mut self) {
        if self.phase != MicrophonePhase::Error && self.phase != MicrophonePhase::Idle {
            self.phase = MicrophonePhase::Captured;
        }
        self.input_level = 0.0;
    }

    fn fail(&mut self, code: MicrophoneErrorCode) {
        self.phase = MicrophonePhase::Error;
        self.permission = permission_for_error(code);
        self.input_level = 0.0;
        self.error_code = Some(code);
    }

    fn reset(&mut self) {
        let next_capture_id = self.capture_id.wrapping_add(1);
        *self = Self {
            capture_id: next_capture_id,
            ..Self::default()
        };
    }

    fn limit_reached(&self) -> bool {
        self.buffer
            .as_ref()
            .is_some_and(CaptureBuffer::limit_reached)
    }

    fn append<T>(&mut self, input: &[T])
    where
        T: Copy + Sample,
        f32: FromSample<T>,
    {
        let Some(buffer) = self.buffer.as_mut() else {
            return;
        };
        self.input_level = buffer.append(input);
    }

    fn schedule_transcription(&mut self, is_final: bool) -> bool {
        let Some(buffer) = self.buffer.as_ref() else {
            return false;
        };
        let duration_ms = buffer.duration_ms();
        if !is_final && duration_ms < self.next_transcription_ms {
            return false;
        }
        self.next_transcription_ms =
            duration_ms.saturating_add(transcription::TRANSCRIPTION_INTERVAL_MS);
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
            }
            return false;
        };
        self.transcription_generation = job.generation();
        self.pending_transcription = Some(job);
        self.transcription_phase = TranscriptionPhase::PreparingModel;
        self.transcription_error_code = None;
        true
    }

    fn take_pending_transcription(&mut self) -> Option<TranscriptionJob> {
        self.pending_transcription.take()
    }

    fn mark_transcribing(&mut self, capture_id: u64, generation: u64) {
        if self.capture_id == capture_id && generation >= self.applied_transcription_generation {
            self.transcription_phase = TranscriptionPhase::Transcribing;
        }
    }

    fn apply_transcription(
        &mut self,
        capture_id: u64,
        generation: u64,
        is_final: bool,
        result: Result<Vec<RawTranscriptSegment>, TranscriptionErrorCode>,
    ) {
        if self.capture_id != capture_id || generation < self.applied_transcription_generation {
            return;
        }
        self.applied_transcription_generation = generation;
        match result {
            Ok(segments) => {
                self.transcript_segments = transcription::bounded_segments(segments, is_final);
                self.transcription_phase = if is_final {
                    TranscriptionPhase::Ready
                } else {
                    TranscriptionPhase::Transcribing
                };
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
        self.next_transcription_ms = transcription::TRANSCRIPTION_INTERVAL_MS;
    }

    fn status(&self) -> MicrophoneStatus {
        let duration_ms = self.buffer.as_ref().map_or(0, CaptureBuffer::duration_ms);
        let max_duration_ms = self.buffer.as_ref().map_or(
            MAX_CAPTURE_DURATION.as_millis() as u64,
            CaptureBuffer::max_duration_ms,
        );
        MicrophoneStatus {
            phase: self.phase,
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
            error_code: self.error_code,
        }
    }
}

impl CaptureBuffer {
    fn new(sample_rate_hz: u32, source_channels: u16) -> Self {
        Self::with_limits(
            sample_rate_hz,
            source_channels,
            MAX_CAPTURE_DURATION,
            MAX_RETAINED_BYTES,
        )
    }

    fn with_limits(
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

    fn append<T>(&mut self, input: &[T]) -> f32
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

    fn limit_reached(&self) -> bool {
        self.samples.len() >= self.max_samples
    }

    fn duration_ms(&self) -> u64 {
        (self.samples.len() as u64)
            .saturating_mul(1_000)
            .saturating_div(u64::from(self.sample_rate_hz.max(1)))
    }

    fn max_duration_ms(&self) -> u64 {
        (self.max_samples as u64)
            .saturating_mul(1_000)
            .saturating_div(u64::from(self.sample_rate_hz.max(1)))
    }

    fn retained_byte_size(&self) -> u64 {
        self.samples.len().saturating_mul(FLOAT_SAMPLE_BYTES) as u64
    }

    fn voice_segments(&self) -> Vec<VoiceSegment> {
        self.voice_detector.segments(self.samples.len())
    }
}

fn permission_for_error(code: MicrophoneErrorCode) -> MicrophonePermission {
    match code {
        MicrophoneErrorCode::PermissionDenied => MicrophonePermission::Denied,
        MicrophoneErrorCode::DeviceUnavailable => MicrophonePermission::Unavailable,
        _ => MicrophonePermission::Prompt,
    }
}

fn clear_finished_worker(worker: &mut Worker) {
    let finished = worker.handle.as_ref().is_some_and(JoinHandle::is_finished);
    if !finished {
        return;
    }
    worker.commands = None;
    if let Some(handle) = worker.handle.take() {
        let _ = handle.join();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests;
