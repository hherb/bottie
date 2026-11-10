//! Off-thread Silero release gate for conversation capture.
//!
//! The energy VAD reacts within a frame or two but releases too eagerly in rooms with noise; Silero is accurate but far
//! too slow for the audio callback. A dedicated worker evaluates a trailing window roughly every 120 ms and publishes
//! the end of the last detected speech. While that end is recent, the capture path keeps classifying frames as speech so
//! the endpointer's hangover starts from the real end of the utterance.

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard,
        mpsc::{Receiver, SyncSender, TrySendError, sync_channel},
    },
    thread::{self, JoinHandle},
};

use super::vad_silero::{SileroVad, SpeechWindow};

/// A verdict older than this says nothing about the current frame.
const GATE_STALENESS_MS: u64 = 600;
/// How long a completed Silero segment keeps the frame classified as speech after its end.
const RELEASE_HOLD_MS: u64 = 300;
const COMMAND_CAPACITY: usize = 2;

/// Capture-relative result of the last Silero evaluation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct VadVerdict {
    /// Capture this verdict belongs to; a new capture starts with the previous id and is ignored.
    pub(super) capture_id: u64,
    /// Capture-relative end of the evaluated window.
    pub(super) evaluated_at_ms: u64,
    /// Capture-relative end of the last speech segment Silero found, if any.
    pub(super) last_speech_end_ms: Option<u64>,
}

/// One trailing window for the VAD worker; samples are already 16 kHz mono.
pub(super) struct GateWindow {
    pub(super) capture_id: u64,
    pub(super) samples_16k: Vec<f32>,
    pub(super) window_end_ms: u64,
}

enum GateCommand {
    Window(GateWindow),
    Shutdown,
}

/// Cloneable sender used by the capture loop; windows are dropped rather than queued without bound.
#[derive(Clone)]
pub(super) struct VadGateSender {
    commands: SyncSender<GateCommand>,
}

impl VadGateSender {
    /// Offers one window to the worker; the newest verdict is always the one that matters.
    pub(super) fn try_window(&self, window: GateWindow) {
        match self.commands.try_send(GateCommand::Window(window)) {
            Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
        }
    }
}

/// Owns the VAD worker and its latest verdict for one microphone controller.
pub(super) struct VadGate {
    commands: SyncSender<GateCommand>,
    verdict: Arc<Mutex<VadVerdict>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl VadGate {
    /// Starts the worker immediately; the model loads and downloads inside it so capture is never blocked.
    pub(super) fn start(cache_dir: PathBuf) -> Self {
        let (commands, receiver) = sync_channel(COMMAND_CAPACITY);
        let verdict = Arc::new(Mutex::new(VadVerdict::default()));
        let shared = verdict.clone();
        let thread = thread::Builder::new()
            .name("bottie-silero-gate".to_owned())
            .spawn(move || run_worker(receiver, shared, cache_dir))
            .expect("the Silero release-gate worker should start");
        Self {
            commands,
            verdict,
            thread: Mutex::new(Some(thread)),
        }
    }

    /// Returns the current verdict for the capture path to consult.
    pub(super) fn verdict(&self) -> Arc<Mutex<VadVerdict>> {
        self.verdict.clone()
    }

    /// Returns a cloneable sender for the capture loop.
    pub(super) fn sender(&self) -> VadGateSender {
        VadGateSender {
            commands: self.commands.clone(),
        }
    }
}

impl Drop for VadGate {
    fn drop(&mut self) {
        let _ = self.commands.try_send(GateCommand::Shutdown);
        if let Some(thread) = lock(&self.thread).take() {
            let _ = thread.join();
        }
    }
}

/// Whether a recent verdict still hears speech near the current frame.
pub(super) fn releases_speech(verdict: &VadVerdict, capture_id: u64, frame_ms: u64) -> bool {
    if verdict.capture_id != capture_id {
        return false;
    }
    if verdict.evaluated_at_ms.saturating_add(GATE_STALENESS_MS) < frame_ms {
        return false;
    }
    match verdict.last_speech_end_ms {
        Some(end) if end <= frame_ms && frame_ms.saturating_sub(end) < RELEASE_HOLD_MS => true,
        _ => false,
    }
}

fn run_worker(
    receiver: Receiver<GateCommand>,
    verdict: Arc<Mutex<VadVerdict>>,
    cache_dir: PathBuf,
) {
    // Loading may download the pinned asset on first use; until then the gate simply reports nothing.
    let mut vad = SileroVad::load(&cache_dir).ok();
    while let Ok(command) = receiver.recv() {
        match command {
            GateCommand::Shutdown => break,
            GateCommand::Window(window) => {
                let Some(vad) = vad.as_mut() else {
                    continue;
                };
                let speech = vad.speech_window(&window.samples_16k).ok().flatten();
                let window_ms = sample_window_ms(window.samples_16k.len());
                let window_start_ms = window.window_end_ms.saturating_sub(window_ms);
                let last_speech_end_ms = speech
                    .map(|SpeechWindow { end_ms, .. }| window_start_ms.saturating_add(end_ms));
                let mut verdict = lock(&verdict);
                verdict.capture_id = window.capture_id;
                verdict.evaluated_at_ms = window.window_end_ms;
                verdict.last_speech_end_ms = last_speech_end_ms;
            }
        }
    }
}

fn sample_window_ms(samples_at_16k: usize) -> u64 {
    (samples_at_16k as u64).saturating_mul(1_000) / 16_000
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(
        capture_id: u64,
        evaluated_at_ms: u64,
        last_speech_end_ms: Option<u64>,
    ) -> VadVerdict {
        VadVerdict {
            capture_id,
            evaluated_at_ms,
            last_speech_end_ms,
        }
    }

    #[test]
    fn a_recent_speech_end_keeps_the_frame_speaking_during_the_hold() {
        let recent = verdict(7, 1_000, Some(950));
        assert!(releases_speech(&recent, 7, 1_000));
        assert!(releases_speech(&recent, 7, 1_240));
        // The hold expires 300 ms after the segment end.
        assert!(!releases_speech(&recent, 7, 1_250));
    }

    #[test]
    fn a_stale_or_foreign_verdict_says_nothing() {
        let stale = verdict(7, 500, Some(480));
        assert!(!releases_speech(&stale, 7, 1_200));
        let foreign = verdict(8, 1_000, Some(950));
        assert!(!releases_speech(&foreign, 7, 1_000));
        let silent = verdict(7, 1_000, None);
        assert!(!releases_speech(&silent, 7, 1_000));
    }

    #[test]
    fn a_speech_end_in_the_future_is_ignored() {
        let verdict = verdict(7, 1_000, Some(1_100));
        assert!(!releases_speech(&verdict, 7, 1_000));
    }

    #[test]
    fn window_duration_follows_sixteen_kilohertz() {
        assert_eq!(sample_window_ms(16_000), 1_000);
        assert_eq!(sample_window_ms(24_000), 1_500);
        assert_eq!(sample_window_ms(0), 0);
    }
}
