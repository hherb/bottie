//! One persistent speech actor owns Python, CPAL, cancellation, and child teardown on a dedicated thread.

use super::{
    playback::NativePlayback,
    process::NanoProcess,
    protocol::{Decoder, request_chunks},
    runtime::NanoRuntime,
};
use crate::speech::{SpeechErrorCode, lock};
use std::{
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::Duration,
};
use tokio::{
    sync::{mpsc as async_mpsc, watch},
    time::{sleep, timeout},
};

const STOP_TIMEOUT: Duration = Duration::from_secs(5);
const UTTERANCE_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const OUTPUT_POLL: Duration = Duration::from_millis(10);
const OUTPUT_STALL_TIMEOUT: Duration = Duration::from_secs(5);

/// Native-only activity; failed work remains an error until explicitly stopped or retried.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Progress {
    Idle,
    Active,
    Failed,
}
struct Request {
    text: String,
    voice: String,
}

/// A bounded command channel and a separate cancel channel prevent queued speech from delaying Stop.
pub(super) struct NanoService {
    commands: async_mpsc::Sender<Request>,
    cancel: watch::Sender<bool>,
    progress: Arc<Mutex<Progress>>,
    done: mpsc::Receiver<()>,
    thread: Option<JoinHandle<()>>,
}

impl NanoService {
    /// Starts a dedicated actor without loading weights until the first explicit speech request.
    pub(super) fn new(runtime: NanoRuntime) -> Result<Self, SpeechErrorCode> {
        let (commands, receiver) = async_mpsc::channel(1);
        let (cancel, cancellation) = watch::channel(false);
        let progress = Arc::new(Mutex::new(Progress::Idle));
        let shared = progress.clone();
        let (done_tx, done) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("bottie-nano-speech".to_owned())
            .spawn(move || {
                let executor = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                match executor {
                    Ok(executor) => {
                        executor.block_on(actor(runtime, receiver, cancellation, shared.clone()))
                    }
                    Err(_) => *lock(&shared) = Progress::Failed,
                }
                let _ = done_tx.send(());
            })
            .map_err(|_| SpeechErrorCode::Unavailable)?;
        Ok(Self {
            commands,
            cancel,
            progress,
            done,
            thread: Some(thread),
        })
    }
    /// Atomically registers one utterance before the controller can admit microphone capture.
    pub(super) fn speak(&mut self, text: &str, voice: &str) -> Result<(), SpeechErrorCode> {
        let mut progress = lock(&self.progress);
        if *progress != Progress::Idle {
            return Err(SpeechErrorCode::PlaybackFailed);
        }
        *progress = Progress::Active;
        self.commands
            .try_send(Request {
                text: text.to_owned(),
                voice: voice.to_owned(),
            })
            .map_err(|_| {
                *progress = Progress::Failed;
                SpeechErrorCode::PlaybackFailed
            })
    }
    /// Reads only bounded actor state, never waiting for inference or the output device.
    pub(super) fn progress(&self) -> Progress {
        *lock(&self.progress)
    }
    /// Confirms that playback is dropped and Python is reaped before returning success.
    pub(super) fn stop(&mut self) -> Result<(), SpeechErrorCode> {
        if self.thread.is_none() {
            return Ok(());
        }
        let _ = self.cancel.send(true);
        self.done
            .recv_timeout(STOP_TIMEOUT)
            .map_err(|_| SpeechErrorCode::PlaybackFailed)?;
        self.thread
            .take()
            .ok_or(SpeechErrorCode::PlaybackFailed)?
            .join()
            .map_err(|_| SpeechErrorCode::PlaybackFailed)?;
        Ok(())
    }
}

impl Drop for NanoService {
    fn drop(&mut self) {
        let _ = self.cancel.send(true);
    }
}

/// Keep the child outside cancelable futures so stopping a partial handshake or pipe read still reaps it.
async fn actor(
    runtime: NanoRuntime,
    mut commands: async_mpsc::Receiver<Request>,
    mut cancel: watch::Receiver<bool>,
    progress: Arc<Mutex<Progress>>,
) {
    let mut process: Option<NanoProcess> = None;
    loop {
        let request = tokio::select! { biased;
            _ = cancel.changed() => break,
            request = commands.recv() => match request { Some(request) => request, None => break },
        };
        if process.is_none() {
            match NanoProcess::spawn(&runtime) {
                Ok(child) => process = Some(child),
                Err(_) => {
                    *lock(&progress) = Progress::Failed;
                    break;
                }
            }
            let ready = tokio::select! { biased;
                _ = cancel.changed() => break,
                ready = process.as_mut().unwrap().ready() => ready,
            };
            if ready.is_err() {
                *lock(&progress) = Progress::Failed;
                break;
            }
        }
        let result = tokio::select! { biased;
            _ = cancel.changed() => break,
            result = timeout(UTTERANCE_TIMEOUT, play(process.as_mut().unwrap(), request)) => result,
        };
        if !matches!(result, Ok(Ok(()))) {
            *lock(&progress) = Progress::Failed;
            break;
        }
        *lock(&progress) = Progress::Idle;
    }
    if let Some(process) = process.as_mut() {
        process.terminate().await;
    }
}

/// Stream backpressure keeps Python, private pipes, and the two-second native queue bounded.
async fn play(process: &mut NanoProcess, request: Request) -> Result<(), SpeechErrorCode> {
    let output = NativePlayback::new()?;
    for text in request_chunks(&request.text) {
        process.speak(&text, &request.voice).await?;
        let mut decoder = Decoder::default();
        loop {
            let event = process.read().await?;
            match event["event"].as_str() {
                Some("audio") => {
                    let frames = decoder.audio(event)?;
                    wait_for_output(|| output.try_push(&frames), OUTPUT_STALL_TIMEOUT).await?;
                }
                Some("complete") => {
                    decoder.complete(&event)?;
                    break;
                }
                _ => return Err(SpeechErrorCode::PlaybackFailed),
            }
        }
    }
    wait_for_output(|| output.drained(), OUTPUT_STALL_TIMEOUT).await
}

/// A stalled or disconnected output device must fail without defeating Stop or keeping capture blocked indefinitely.
pub(super) async fn wait_for_output(
    mut ready: impl FnMut() -> Result<bool, SpeechErrorCode>,
    deadline: Duration,
) -> Result<(), SpeechErrorCode> {
    timeout(deadline, async {
        while !ready()? {
            sleep(OUTPUT_POLL).await;
        }
        Ok(())
    })
    .await
    .map_err(|_| SpeechErrorCode::PlaybackFailed)?
}
