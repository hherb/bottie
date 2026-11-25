//! Bounded native microphone capture with path-free WebView status.

mod audio;
mod buffer;
mod capture;
mod capture_state;
mod correction;
mod devices;
pub(crate) mod endpointing;
pub(crate) mod latency;
mod pinned_assets;
mod transcription;
mod vad;
mod vad_gate;
mod vad_silero;

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard,
        mpsc::{self, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use serde::Serialize;

pub(crate) use audio::{CapturedAudio, CapturedAudioError, CapturedAudioFormat};
use buffer::CaptureBuffer;
use capture::capture_worker;
use capture_state::CaptureState;
use correction::MAX_TRANSCRIPT_TURN_BYTES;
pub(crate) use correction::TranscriptCorrectionError;
use devices::MicrophoneDeviceRegistry;
pub(crate) use devices::{MicrophoneDeviceCommandError, MicrophoneInputDeviceList};
use endpointing::EndReason;
use latency::MicrophoneLatency;
use transcription::{
    TranscriptSegment, TranscriptionErrorCode, TranscriptionPhase, TranscriptionWorker,
};
use vad::{VoiceActivity, VoiceActivityDetector, VoiceSegment};
use vad_gate::VadGate;

const MAX_CAPTURE_DURATION: Duration = Duration::from_secs(60);
const MAX_RETAINED_BYTES: usize = 32 * 1024 * 1024;
const CAPTURE_WORKER_POLL: Duration = Duration::from_millis(40);
const FLOAT_SAMPLE_BYTES: usize = size_of::<f32>();

/// How the current native capture decides when a turn ends.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MicrophoneMode {
    /// The user presses Stop; the native endpoint detector stays idle.
    #[default]
    PushToTalk,
    /// The native endpoint detector decides when the turn ended and stops capture.
    Conversation,
}

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
    /// An explicitly selected input is no longer present at capture start.
    SelectedDeviceUnavailable,
    /// Another process or stream currently owns the input.
    DeviceBusy,
    /// The selected input uses a sample format outside Bottie's PCM boundary.
    UnsupportedFormat,
    /// A redacted backend, resource, or stream failure stopped capture.
    CaptureFailed,
}

/// Bounded metadata exposed to the WebView without samples or device identity.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MicrophoneStatus {
    phase: MicrophonePhase,
    mode: MicrophoneMode,
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
    /// Silero failed to load in the recognizer (silence and noise reach Whisper unfiltered) or, in a conversation, in
    /// the release gate (turns end on loudness alone). Recognition still works; the next capture retries the load.
    speech_detection_error_code: Option<TranscriptionErrorCode>,
    error_code: Option<MicrophoneErrorCode>,
    latency: MicrophoneLatency,
    endpoint_reason: Option<EndReason>,
}

/// Thread-safe controller for the one session-only native capture slot.
pub(crate) struct MicrophoneController {
    shared: Arc<Mutex<CaptureState>>,
    worker: Mutex<Worker>,
    devices: Mutex<MicrophoneDeviceRegistry>,
    transcription: Option<TranscriptionWorker>,
    model_cache_path: PathBuf,
    /// Started on the first conversation capture; the worker loads and verifies the VAD asset off the capture path.
    vad_gate: Mutex<Option<VadGate>>,
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

impl Default for MicrophoneController {
    fn default() -> Self {
        Self {
            shared: Arc::new(Mutex::new(CaptureState::default())),
            worker: Mutex::new(Worker::default()),
            devices: Mutex::new(MicrophoneDeviceRegistry::default()),
            transcription: None,
            model_cache_path: PathBuf::default(),
            vad_gate: Mutex::new(None),
        }
    }
}

impl MicrophoneController {
    /// Starts the process-lifetime local speech worker without loading its model before user capture.
    pub(crate) fn new(model_cache_path: PathBuf, remembered_input_token: Option<String>) -> Self {
        let shared = Arc::new(Mutex::new(CaptureState::default()));
        let transcription = TranscriptionWorker::start(model_cache_path.clone(), shared.clone());
        Self {
            shared,
            worker: Mutex::new(Worker::default()),
            devices: Mutex::new(MicrophoneDeviceRegistry::with_remembered_token(
                remembered_input_token,
            )),
            transcription: Some(transcription),
            model_cache_path,
            vad_gate: Mutex::new(None),
        }
    }

    /// Returns the current path-free capture state.
    pub(crate) fn status(&self) -> MicrophoneStatus {
        lock(&self.shared).status()
    }

    /// Reports the latched end-of-turn reason for conversation capture, if the turn has finished.
    pub(crate) fn endpoint_reason(&self) -> Option<EndReason> {
        lock(&self.shared).endpoint_finished()
    }

    /// Reports whether the last capture attempt failed and no input is open.
    pub(crate) fn failed(&self) -> bool {
        lock(&self.shared).phase == MicrophonePhase::Error
    }

    /// Reports whether native input is starting or actively recording.
    pub(crate) fn is_capturing(&self) -> bool {
        matches!(
            lock(&self.shared).phase,
            MicrophonePhase::Starting | MicrophonePhase::Recording
        )
    }

    /// Starts one selected-input capture only after the WebView's explicit user action.
    pub(crate) fn start(&self) -> MicrophoneStatus {
        self.start_with_mode(MicrophoneMode::PushToTalk)
    }

    /// Starts a conversation capture whose turn ends in the native endpoint detector.
    pub(crate) fn start_conversation(&self) -> MicrophoneStatus {
        self.start_with_mode(MicrophoneMode::Conversation)
    }

    fn start_with_mode(&self, mode: MicrophoneMode) -> MicrophoneStatus {
        let mut worker = lock(&self.worker);
        let phase = lock(&self.shared).phase;
        clear_worker_for_restart(&mut worker, phase);
        if worker.handle.is_some() {
            return self.status();
        }

        let devices = lock(&self.devices);
        let input_selection = devices.capture_selection();
        let gate_handles = if mode == MicrophoneMode::Conversation {
            let mut gate = lock(&self.vad_gate);
            let gate = gate.get_or_insert_with(|| {
                let shared = self.shared.clone();
                VadGate::start(self.model_cache_path.clone(), move |load| {
                    lock(&shared).gate_vad_error = load.err();
                })
            });
            Some((gate.verdict(), gate.sender()))
        } else {
            None
        };
        {
            let mut state = lock(&self.shared);
            state.mode = mode;
            state.vad_verdict = gate_handles.as_ref().map(|(verdict, _)| verdict.clone());
            state.vad_gate = gate_handles.map(|(_, sender)| sender);
            state.begin_starting();
        }
        drop(devices);
        let (commands, receiver) = mpsc::channel();
        let shared = self.shared.clone();
        let transcription_wake = self
            .transcription
            .as_ref()
            .map(TranscriptionWorker::wake_handle);
        match thread::Builder::new()
            .name("bottie-microphone".into())
            .spawn(move || capture_worker(shared, receiver, transcription_wake, input_selection))
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
            let mut state = lock(&self.shared);
            let requested_at = Instant::now();
            if state.phase == MicrophonePhase::Recording
                && commands.send(CaptureCommand::Stop).is_ok()
            {
                state.latency.begin_finalization(requested_at);
            }
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

/// Joins a worker that has finished or whose capture state has already become inactive.
fn clear_worker_for_restart(worker: &mut Worker, phase: MicrophonePhase) {
    let finished = worker.handle.as_ref().is_some_and(JoinHandle::is_finished);
    if !worker_can_be_replaced(phase, finished) {
        return;
    }
    worker.commands = None;
    if let Some(handle) = worker.handle.take() {
        let _ = handle.join();
    }
}

/// Reports whether a replacement may safely join the old capture owner before starting.
fn worker_can_be_replaced(phase: MicrophonePhase, finished: bool) -> bool {
    finished
        || !matches!(
            phase,
            MicrophonePhase::Starting | MicrophonePhase::Recording
        )
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod worker_tests;
