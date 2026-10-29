//! Cancel-safe private-pipe worker ownership with bounded framing and complete child reaping.

use super::{
    protocol::{MAX_EVENT_BYTES, validate_ready},
    runtime::NanoRuntime,
};
use crate::speech::SpeechErrorCode;
use serde_json::Value;
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::timeout,
};

const READ_BYTES: usize = 8 * 1024;
const START_TIMEOUT: Duration = Duration::from_secs(60);
const EVENT_TIMEOUT: Duration = Duration::from_secs(45);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const REAP_RETRY: Duration = Duration::from_secs(1);

/// Owns the process independently of cancelable startup/read futures so teardown can always reach it.
pub(super) struct NanoProcess {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
    buffer: Vec<u8>,
}

impl NanoProcess {
    /// Starts only the configured Python and this binary's exact embedded worker, without a shell.
    pub(super) fn spawn(runtime: &NanoRuntime) -> Result<Self, SpeechErrorCode> {
        let script = runtime.prepare_worker()?;
        let mut command = Command::new(&runtime.python);
        let mut child = crate::worker_env::apply_minimal_environment(&mut command)
            .args(["-I", "-u"])
            .arg(script)
            .arg("--directory")
            .arg(&runtime.assets)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| SpeechErrorCode::Unavailable)?;
        let input = child.stdin.take().ok_or(SpeechErrorCode::Unavailable)?;
        let output = child.stdout.take().ok_or(SpeechErrorCode::Unavailable)?;
        Ok(Self {
            child,
            input,
            output,
            buffer: Vec::new(),
        })
    }

    /// Requires exact ready identities before any text is sent or PCM is consumed.
    pub(super) async fn ready(&mut self) -> Result<(), SpeechErrorCode> {
        let event = timeout(START_TIMEOUT, self.read_line())
            .await
            .map_err(|_| SpeechErrorCode::Unavailable)??;
        validate_ready(&event)
    }

    /// Sends a serial bounded request using private stdin and a fixed write deadline.
    pub(super) async fn speak(&mut self, text: &str, voice: &str) -> Result<(), SpeechErrorCode> {
        let mut bytes =
            serde_json::to_vec(&serde_json::json!({"text":text,"voice":voice,"seed":42}))
                .map_err(|_| SpeechErrorCode::PlaybackFailed)?;
        bytes.push(b'\n');
        timeout(WRITE_TIMEOUT, self.input.write_all(&bytes))
            .await
            .map_err(|_| SpeechErrorCode::PlaybackFailed)?
            .map_err(|_| SpeechErrorCode::PlaybackFailed)
    }

    /// Applies one absolute deadline to the complete event, including partial lines.
    pub(super) async fn read(&mut self) -> Result<Value, SpeechErrorCode> {
        timeout(EVENT_TIMEOUT, self.read_line())
            .await
            .map_err(|_| SpeechErrorCode::PlaybackFailed)?
    }

    async fn read_line(&mut self) -> Result<Value, SpeechErrorCode> {
        read_json_line(&mut self.output, &mut self.buffer).await
    }

    /// Retry kill/wait until the exact child is confirmed reaped; callers remain fail-closed meanwhile.
    pub(super) async fn terminate(&mut self) {
        loop {
            let _ = self.child.start_kill();
            if matches!(timeout(REAP_RETRY, self.child.wait()).await, Ok(Ok(_))) {
                break;
            }
            tokio::time::sleep(REAP_RETRY).await;
        }
        self.buffer.clear();
    }
}

/// Preserve partial bytes across cancellation and reject oversized or incomplete JSON events.
pub(super) async fn read_json_line<R: tokio::io::AsyncRead + Unpin>(
    output: &mut R,
    buffer: &mut Vec<u8>,
) -> Result<Value, SpeechErrorCode> {
    loop {
        if let Some(end) = buffer.iter().position(|byte| *byte == b'\n') {
            if end > MAX_EVENT_BYTES {
                return Err(SpeechErrorCode::PlaybackFailed);
            }
            let event = serde_json::from_slice(&buffer[..end])
                .map_err(|_| SpeechErrorCode::PlaybackFailed)?;
            buffer.drain(..=end);
            return Ok(event);
        }
        if buffer.len() > MAX_EVENT_BYTES {
            return Err(SpeechErrorCode::PlaybackFailed);
        }
        let mut bytes = [0; READ_BYTES];
        let count = output
            .read(&mut bytes)
            .await
            .map_err(|_| SpeechErrorCode::PlaybackFailed)?;
        if count == 0 {
            return Err(SpeechErrorCode::PlaybackFailed);
        }
        buffer.extend_from_slice(&bytes[..count]);
    }
}
