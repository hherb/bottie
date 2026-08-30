//! Bounded native microphone capture with path-free WebView status.

mod vad;

use std::{
    sync::{
        Arc, Mutex, MutexGuard,
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use cpal::{
    Device, Error, ErrorKind, FromSample, I24, Sample, SampleFormat, SizedSample, Stream,
    StreamConfig, U24,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use serde::Serialize;

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
    error_code: Option<MicrophoneErrorCode>,
}

/// Thread-safe controller for the one session-only native capture slot.
pub(crate) struct MicrophoneController {
    shared: Arc<Mutex<CaptureState>>,
    worker: Mutex<Worker>,
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
        }
    }
}

impl MicrophoneController {
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
        match thread::Builder::new()
            .name("bottie-microphone".into())
            .spawn(move || capture_worker(shared, receiver))
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
        self.phase = MicrophonePhase::Starting;
        self.permission = MicrophonePermission::Prompt;
        self.buffer = None;
        self.input_level = 0.0;
        self.error_code = None;
    }

    fn begin_recording(&mut self, sample_rate_hz: u32, channels: u16) {
        self.phase = MicrophonePhase::Recording;
        self.permission = MicrophonePermission::Granted;
        self.buffer = Some(CaptureBuffer::new(sample_rate_hz, channels));
        self.input_level = 0.0;
        self.error_code = None;
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
        *self = Self::default();
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

fn capture_worker(shared: Arc<Mutex<CaptureState>>, receiver: Receiver<CaptureCommand>) {
    let host = cpal::default_host();
    let Some(device) = host.default_input_device() else {
        lock(&shared).fail(MicrophoneErrorCode::DeviceUnavailable);
        return;
    };
    let config = match device.default_input_config() {
        Ok(config) => config,
        Err(error) => {
            lock(&shared).fail(error_code(error.kind()));
            return;
        }
    };
    let sample_rate_hz = config.sample_rate();
    let channels = config.channels();
    let sample_format = config.sample_format();
    let stream = match build_input_stream(&device, config.config(), sample_format, shared.clone()) {
        Ok(stream) => stream,
        Err(error) => {
            lock(&shared).fail(error);
            return;
        }
    };
    lock(&shared).begin_recording(sample_rate_hz, channels);
    if let Err(error) = stream.play() {
        lock(&shared).fail(error_code(error.kind()));
        return;
    }

    let discard = loop {
        if lock(&shared).phase == MicrophonePhase::Error || lock(&shared).limit_reached() {
            break false;
        }
        match receiver.recv_timeout(CAPTURE_WORKER_POLL) {
            Ok(CaptureCommand::Stop) => break false,
            Ok(CaptureCommand::Discard) | Err(RecvTimeoutError::Disconnected) => break true,
            Err(RecvTimeoutError::Timeout) => {}
        }
    };
    drop(stream);
    let mut state = lock(&shared);
    if discard {
        state.reset();
    } else {
        state.finish();
    }
}

fn build_input_stream(
    device: &Device,
    config: StreamConfig,
    format: SampleFormat,
    shared: Arc<Mutex<CaptureState>>,
) -> Result<Stream, MicrophoneErrorCode> {
    match format {
        SampleFormat::I8 => build_typed_stream::<i8>(device, config, shared),
        SampleFormat::I16 => build_typed_stream::<i16>(device, config, shared),
        SampleFormat::I24 => build_typed_stream::<I24>(device, config, shared),
        SampleFormat::I32 => build_typed_stream::<i32>(device, config, shared),
        SampleFormat::I64 => build_typed_stream::<i64>(device, config, shared),
        SampleFormat::U8 => build_typed_stream::<u8>(device, config, shared),
        SampleFormat::U16 => build_typed_stream::<u16>(device, config, shared),
        SampleFormat::U24 => build_typed_stream::<U24>(device, config, shared),
        SampleFormat::U32 => build_typed_stream::<u32>(device, config, shared),
        SampleFormat::U64 => build_typed_stream::<u64>(device, config, shared),
        SampleFormat::F32 => build_typed_stream::<f32>(device, config, shared),
        SampleFormat::F64 => build_typed_stream::<f64>(device, config, shared),
        _ => Err(MicrophoneErrorCode::UnsupportedFormat),
    }
}

fn build_typed_stream<T>(
    device: &Device,
    config: StreamConfig,
    shared: Arc<Mutex<CaptureState>>,
) -> Result<Stream, MicrophoneErrorCode>
where
    T: Copy + Sample + SizedSample,
    f32: FromSample<T>,
{
    let error_state = shared.clone();
    device
        .build_input_stream::<T, _, _>(
            config,
            move |input, _| lock(&shared).append(input),
            move |error: Error| lock(&error_state).fail(error_code(error.kind())),
            None,
        )
        .map_err(|error| error_code(error.kind()))
}

fn error_code(kind: ErrorKind) -> MicrophoneErrorCode {
    match kind {
        ErrorKind::PermissionDenied => MicrophoneErrorCode::PermissionDenied,
        ErrorKind::DeviceNotAvailable | ErrorKind::HostUnavailable => {
            MicrophoneErrorCode::DeviceUnavailable
        }
        ErrorKind::DeviceBusy => MicrophoneErrorCode::DeviceBusy,
        ErrorKind::UnsupportedConfig
        | ErrorKind::UnsupportedOperation
        | ErrorKind::InvalidInput => MicrophoneErrorCode::UnsupportedFormat,
        _ => MicrophoneErrorCode::CaptureFailed,
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
