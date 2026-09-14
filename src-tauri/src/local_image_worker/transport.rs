//! Bounded private-pipe ownership for one long-lived local image worker.

use std::{
    collections::VecDeque,
    ffi::OsString,
    path::{Component, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::mpsc,
    task::JoinHandle,
    time::{Instant as TokioInstant, sleep_until, timeout},
};

use super::{
    manager::{CANCELLATION_GRACE, ManagerError, ManagerEvent, WorkerManager, WorkerReadiness},
    protocol::{
        FrameDecoder, HostMessage, ModelLocation, WorkerMessage, decode_worker_payload,
        encode_host_frame,
    },
};

#[path = "transport/stderr.rs"]
mod stderr;

use stderr::{StderrSignal, drain_stderr};

/// Bounded read buffer used without retaining worker output beyond protocol limits.
const PIPE_READ_BYTES: usize = 8 * 1_024;
/// Default deadline for version and capability negotiation.
const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Default deadline for one complete worker event.
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(45);
/// Default deadline for one complete host frame write.
const DEFAULT_WRITE_TIMEOUT: Duration = Duration::from_secs(5);
/// Default deadline for clean shutdown or forced process reaping.
const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Stable path-free process transport failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TransportError {
    /// The executable path was not an exact absolute native path.
    InvalidExecutable,
    /// One or more configured process-I/O deadlines were zero.
    InvalidTimeout,
    /// The trusted worker process could not be spawned.
    SpawnFailed,
    /// A required private pipe was not available after spawn.
    PipeUnavailable,
    /// The handshake did not finish within its fixed deadline.
    HandshakeTimeout,
    /// A host frame could not be written before its deadline.
    WriteTimeout,
    /// A private stdin write failed.
    WriteFailed,
    /// A complete worker event did not arrive before its deadline.
    ReadTimeout,
    /// A private stdout read failed.
    ReadFailed,
    /// The worker closed stdout between operations or inside a frame.
    UnexpectedEof,
    /// A worker frame or message failed protocol validation.
    Protocol,
    /// Lifecycle or request correlation validation failed.
    Manager(ManagerError),
    /// The worker exceeded the discard-only stderr ceiling.
    StderrLimitExceeded,
    /// Cooperative cancellation exceeded its three-second grace period.
    CancellationTimeout,
    /// Clean shutdown exceeded its fixed deadline.
    ShutdownTimeout,
    /// The worker exited unsuccessfully instead of completing clean shutdown.
    WorkerExited,
    /// Forced termination could not be confirmed and reaped within the deadline.
    TeardownFailed,
}

/// Exact executable and arguments selected by Rust without shell interpretation.
#[derive(Clone, Debug)]
pub(crate) struct WorkerProcessSpec {
    executable: PathBuf,
    arguments: Vec<OsString>,
}

impl WorkerProcessSpec {
    /// Creates one native process specification; validation occurs before spawn.
    pub(crate) fn new(executable: PathBuf, arguments: Vec<OsString>) -> Self {
        Self {
            executable,
            arguments,
        }
    }

    fn validate(&self) -> Result<(), TransportError> {
        let path = self.executable.as_path();
        if !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            Err(TransportError::InvalidExecutable)
        } else {
            Ok(())
        }
    }
}

/// Process-I/O deadlines kept separate from the fixed cancellation policy.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TransportTimeouts {
    handshake: Duration,
    read: Duration,
    write: Duration,
    shutdown: Duration,
}

impl Default for TransportTimeouts {
    fn default() -> Self {
        Self::new(
            DEFAULT_HANDSHAKE_TIMEOUT,
            DEFAULT_READ_TIMEOUT,
            DEFAULT_WRITE_TIMEOUT,
            DEFAULT_SHUTDOWN_TIMEOUT,
        )
    }
}

impl TransportTimeouts {
    /// Creates explicit positive deadlines for deterministic transport tests.
    pub(crate) fn new(
        handshake: Duration,
        read: Duration,
        write: Duration,
        shutdown: Duration,
    ) -> Self {
        Self {
            handshake,
            read,
            write,
            shutdown,
        }
    }

    fn valid(self) -> bool {
        [self.handshake, self.read, self.write, self.shutdown]
            .into_iter()
            .all(|duration| !duration.is_zero())
    }
}

/// One Rust-owned worker process with private framed stdin/stdout and bounded stderr.
#[derive(Debug)]
pub(crate) struct WorkerTransport {
    manager: WorkerManager,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<ChildStdout>,
    stderr_task: Option<JoinHandle<()>>,
    stderr_signals: mpsc::Receiver<StderrSignal>,
    stderr_closed: bool,
    decoder: FrameDecoder,
    queued_messages: VecDeque<WorkerMessage>,
    started_at: Instant,
    cancellation_started_at: Option<Instant>,
    timeouts: TransportTimeouts,
}

impl WorkerTransport {
    /// Spawns and negotiates one exact trusted executable using production deadlines.
    pub(crate) async fn spawn(
        spec: WorkerProcessSpec,
        client_version: &str,
    ) -> Result<Self, TransportError> {
        Self::spawn_with_timeouts(spec, client_version, TransportTimeouts::default()).await
    }

    /// Spawns and negotiates one worker with explicit positive process-I/O deadlines.
    pub(crate) async fn spawn_with_timeouts(
        spec: WorkerProcessSpec,
        client_version: &str,
        timeouts: TransportTimeouts,
    ) -> Result<Self, TransportError> {
        spec.validate()?;
        if !timeouts.valid() {
            return Err(TransportError::InvalidTimeout);
        }
        let mut manager = WorkerManager::new();
        let hello = manager
            .begin_handshake(client_version)
            .map_err(TransportError::Manager)?;
        let mut command = Command::new(&spec.executable);
        command
            .args(spec.arguments)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(|_| TransportError::SpawnFailed)?;
        let pipes = (child.stdin.take(), child.stdout.take(), child.stderr.take());
        let (Some(stdin), Some(stdout), Some(stderr)) = pipes else {
            return match timeout(timeouts.shutdown, child.kill()).await {
                Ok(Ok(())) => Err(TransportError::PipeUnavailable),
                _ => Err(TransportError::TeardownFailed),
            };
        };
        let (stderr_sender, stderr_signals) = mpsc::channel(1);
        let stderr_task = tokio::spawn(drain_stderr(stderr, stderr_sender));
        let mut transport = Self {
            manager,
            child: Some(child),
            stdin: Some(stdin),
            stdout: Some(stdout),
            stderr_task: Some(stderr_task),
            stderr_signals,
            stderr_closed: false,
            decoder: FrameDecoder::new(),
            queued_messages: VecDeque::new(),
            started_at: Instant::now(),
            cancellation_started_at: None,
            timeouts,
        };
        if let Err(error) = transport.finish_handshake(hello).await {
            return Err(transport.stop_after(error).await);
        }
        Ok(transport)
    }

    /// Returns coarse path-free readiness from the validated lifecycle manager.
    pub(crate) fn readiness(&self) -> WorkerReadiness {
        self.manager.readiness()
    }

    /// Returns the native process identity only to the explicitly enabled runtime-proof tool.
    #[cfg(feature = "local-image-runtime-proof")]
    pub(crate) fn process_id(&self) -> Option<u32> {
        self.child.as_ref().and_then(Child::id)
    }

    /// Sends one exact verified-model load request.
    pub(crate) async fn begin_load(
        &mut self,
        request_id: impl Into<String>,
        model: ModelLocation,
    ) -> Result<(), TransportError> {
        let message = self
            .manager
            .begin_load(request_id, model)
            .map_err(TransportError::Manager)?;
        self.write_or_stop(message).await
    }

    /// Sends one bounded generation request against the retained loaded model.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn begin_generation(
        &mut self,
        request_id: impl Into<String>,
        prompt: impl Into<String>,
        width: u32,
        height: u32,
        count: u8,
        seed: Option<u64>,
    ) -> Result<(), TransportError> {
        let message = self
            .manager
            .begin_generation(request_id, prompt, width, height, count, seed)
            .map_err(TransportError::Manager)?;
        self.write_or_stop(message).await
    }

    /// Sends cooperative cancellation and starts the manager's fixed grace period.
    pub(crate) async fn cancel_active(&mut self) -> Result<(), TransportError> {
        let now = Instant::now();
        let message = self
            .manager
            .cancel_active(now.saturating_duration_since(self.started_at))
            .map_err(TransportError::Manager)?;
        self.write_or_stop(message).await?;
        self.cancellation_started_at = Some(now);
        Ok(())
    }

    /// Reads, validates, and correlates the next complete worker event.
    pub(crate) async fn next_event(&mut self) -> Result<ManagerEvent, TransportError> {
        let deadline = self.event_deadline();
        let message = match self.read_message_until(deadline).await {
            Ok(message) => message,
            Err(TransportError::ReadTimeout) if self.cancellation_started_at.is_some() => {
                let forced_at = self
                    .cancellation_started_at
                    .expect("cancellation start checked")
                    + CANCELLATION_GRACE;
                if self
                    .manager
                    .requires_forced_teardown(forced_at.saturating_duration_since(self.started_at))
                {
                    return Err(self.stop_after(TransportError::CancellationTimeout).await);
                }
                return Err(self.stop_after(TransportError::ReadTimeout).await);
            }
            Err(error) => return Err(self.stop_after(error).await),
        };
        let event = match self.manager.accept(message) {
            Ok(event) => event,
            Err(error) => return Err(self.stop_after(TransportError::Manager(error)).await),
        };
        if matches!(
            event,
            ManagerEvent::ModelLoaded
                | ManagerEvent::Generated(_)
                | ManagerEvent::Cancelled
                | ManagerEvent::Failed
        ) {
            self.cancellation_started_at = None;
        }
        Ok(event)
    }

    /// Requests clean worker exit, enforcing a bounded wait and confirmed reap.
    pub(crate) async fn shutdown(mut self) -> Result<(), TransportError> {
        let message = match self.manager.begin_shutdown() {
            Ok(message) => message,
            Err(error) => return Err(self.stop_after(TransportError::Manager(error)).await),
        };
        if let Err(error) = self.write_message(&message).await {
            return Err(self.stop_after(error).await);
        }
        self.stdin.take();
        let status = match self.child.as_mut() {
            Some(child) => match timeout(self.timeouts.shutdown, child.wait()).await {
                Ok(Ok(status)) => status,
                Ok(Err(_)) => return Err(self.stop_after(TransportError::WorkerExited).await),
                Err(_) => return Err(self.stop_after(TransportError::ShutdownTimeout).await),
            },
            None => return Err(TransportError::WorkerExited),
        };
        self.child.take();
        self.stdout.take();
        if !status.success() {
            let stderr_result = self.finish_stderr().await;
            self.manager.mark_stopped();
            return Err(stderr_result.err().unwrap_or(TransportError::WorkerExited));
        }
        if let Err(error) = self.finish_stderr().await {
            self.manager.mark_stopped();
            return Err(error);
        }
        self.manager.mark_stopped();
        Ok(())
    }

    async fn finish_handshake(&mut self, hello: HostMessage) -> Result<(), TransportError> {
        self.write_message(&hello).await?;
        let deadline = TokioInstant::now() + self.timeouts.handshake;
        for expected in [ManagerEvent::Hello, ManagerEvent::Ready] {
            let message = self
                .read_message_until(deadline)
                .await
                .map_err(|error| match error {
                    TransportError::ReadTimeout => TransportError::HandshakeTimeout,
                    other => other,
                })?;
            let event = self
                .manager
                .accept(message)
                .map_err(TransportError::Manager)?;
            if event != expected {
                return Err(TransportError::Protocol);
            }
        }
        Ok(())
    }

    fn event_deadline(&self) -> TokioInstant {
        let duration = self
            .cancellation_started_at
            .map(|started| CANCELLATION_GRACE.saturating_sub(started.elapsed()))
            .unwrap_or(self.timeouts.read);
        TokioInstant::now() + duration
    }

    async fn write_or_stop(&mut self, message: HostMessage) -> Result<(), TransportError> {
        match self.write_message(&message).await {
            Ok(()) => Ok(()),
            Err(error) => Err(self.stop_after(error).await),
        }
    }

    async fn write_message(&mut self, message: &HostMessage) -> Result<(), TransportError> {
        let frame = encode_host_frame(message).map_err(|_| TransportError::Protocol)?;
        let stdin = self.stdin.as_mut().ok_or(TransportError::WriteFailed)?;
        match timeout(self.timeouts.write, async {
            stdin.write_all(&frame).await?;
            stdin.flush().await
        })
        .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(TransportError::WriteFailed),
            Err(_) => Err(TransportError::WriteTimeout),
        }
    }

    async fn read_message_until(
        &mut self,
        deadline: TokioInstant,
    ) -> Result<WorkerMessage, TransportError> {
        if let Some(message) = self.queued_messages.pop_front() {
            return Ok(message);
        }
        let mut bytes = [0_u8; PIPE_READ_BYTES];
        let deadline_sleep = sleep_until(deadline);
        tokio::pin!(deadline_sleep);
        loop {
            let read = self
                .stdout
                .as_mut()
                .ok_or(TransportError::ReadFailed)?
                .read(&mut bytes);
            tokio::select! {
                _ = &mut deadline_sleep => return Err(TransportError::ReadTimeout),
                signal = self.stderr_signals.recv(), if !self.stderr_closed => {
                    match signal {
                        Some(StderrSignal::LimitExceeded) => {
                            return Err(TransportError::StderrLimitExceeded);
                        }
                        Some(StderrSignal::ReadFailed) => return Err(TransportError::ReadFailed),
                        None => self.stderr_closed = true,
                    }
                }
                result = read => {
                    let count = result.map_err(|_| TransportError::ReadFailed)?;
                    if count == 0 {
                        let decoder = std::mem::take(&mut self.decoder);
                        let _ = decoder.finish();
                        return Err(TransportError::UnexpectedEof);
                    }
                    let payloads = self
                        .decoder
                        .push(&bytes[..count])
                        .map_err(|_| TransportError::Protocol)?;
                    for payload in payloads {
                        self.queued_messages.push_back(
                            decode_worker_payload(&payload).map_err(|_| TransportError::Protocol)?,
                        );
                    }
                    if let Some(message) = self.queued_messages.pop_front() {
                        return Ok(message);
                    }
                }
            }
        }
    }

    async fn stop_after(&mut self, original: TransportError) -> TransportError {
        match self.force_teardown().await {
            Ok(()) => original,
            Err(error) => error,
        }
    }

    async fn force_teardown(&mut self) -> Result<(), TransportError> {
        self.stdin.take();
        self.stdout.take();
        let process_result = if let Some(mut child) = self.child.take() {
            match child.try_wait() {
                Ok(Some(_)) => Ok(()),
                Ok(None) => match timeout(self.timeouts.shutdown, child.kill()).await {
                    Ok(Ok(())) => Ok(()),
                    _ => Err(TransportError::TeardownFailed),
                },
                Err(_) => Err(TransportError::TeardownFailed),
            }
        } else {
            Ok(())
        };
        let stderr_result = self.finish_stderr().await;
        self.manager.mark_stopped();
        self.cancellation_started_at = None;
        process_result.and(stderr_result)
    }

    async fn finish_stderr(&mut self) -> Result<(), TransportError> {
        if let Some(task) = self.stderr_task.take() {
            match timeout(self.timeouts.shutdown, task).await {
                Ok(Ok(())) => {}
                _ => return Err(TransportError::TeardownFailed),
            }
        }
        match self.stderr_signals.try_recv() {
            Ok(StderrSignal::LimitExceeded) => Err(TransportError::StderrLimitExceeded),
            Ok(StderrSignal::ReadFailed) => Err(TransportError::ReadFailed),
            Err(_) => Ok(()),
        }
    }
}
