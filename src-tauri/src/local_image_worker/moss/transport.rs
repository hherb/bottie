//! One request per child: bounded framing, independent cancellation, and confirmed process reaping.
use super::{
    MossError, MossInstallation,
    protocol::{self, MAX_EVENT_BYTES},
};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::{Child, Command},
    sync::watch,
    time::timeout,
};
const STARTUP_TIMEOUT: Duration = Duration::from_secs(300);
const GENERATION_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const READ_BYTES: usize = 8192;
const MAX_PROGRESS_EVENTS: usize = 64;
/// Keep the external CUDA interpreter from opening an unrelated console window in the desktop app.
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

impl MossInstallation {
    /// Verify exact assets off-thread, then stream a single local request without retaining a warm GPU process.
    pub(crate) async fn generate(
        &self,
        prompt: &str,
        size: (u32, u32),
        seed: u64,
        mut cancel: watch::Receiver<bool>,
        on_progress: impl FnMut() + Send,
    ) -> Result<Vec<u8>, MossError> {
        if prompt.trim().is_empty()
            || prompt.len() > super::MAX_PROMPT_BYTES
            || !super::valid_dimensions(size)
        {
            return Err(MossError::InvalidOutput);
        }
        let installation = self.clone();
        let verification_cancel = cancel.clone();
        let mut verification = tauri::async_runtime::spawn_blocking(move || {
            installation.verify_assets(verification_cancel)
        });
        tokio::select! { biased;
            _=cancelled(&mut cancel)=>{let _=verification.await;return Err(MossError::Cancelled);},
            result=&mut verification=>result.map_err(|_|MossError::Unavailable)??,
        }
        if *cancel.borrow() {
            return Err(MossError::Cancelled);
        }
        run(
            self.command()?,
            prompt,
            size,
            seed,
            self.model_id(),
            self.revision(),
            self.runtime(),
            cancel,
            on_progress,
        )
        .await
    }
}

/// Keep ownership outside all cancelable futures; every terminal result kills/reaps its exact child.
#[allow(clippy::too_many_arguments)]
async fn run(
    mut command: Command,
    prompt: &str,
    size: (u32, u32),
    seed: u64,
    model: &str,
    revision: &str,
    runtime: &str,
    mut cancel: watch::Receiver<bool>,
    mut progress: impl FnMut() + Send,
) -> Result<Vec<u8>, MossError> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);
    let mut child = command.spawn().map_err(|_| MossError::Unavailable)?;
    let result = tokio::select! { biased;
        _=cancelled(&mut cancel)=>Err(MossError::Cancelled),
        result=timeout(GENERATION_TIMEOUT,exchange(&mut child,prompt,size,seed,model,revision,runtime,&mut progress))=>
            result.map_err(|_|MossError::Unavailable).and_then(|r|r),
    };
    reap(&mut child).await;
    if *cancel.borrow() {
        Err(MossError::Cancelled)
    } else {
        result
    }
}

/// Require readiness, a matching completion, successful exit, and EOF before accepting any PNG bytes.
#[allow(clippy::too_many_arguments)]
async fn exchange(
    child: &mut Child,
    prompt: &str,
    size: (u32, u32),
    seed: u64,
    model: &str,
    revision: &str,
    runtime: &str,
    progress: &mut (impl FnMut() + Send),
) -> Result<Vec<u8>, MossError> {
    let mut output = child.stdout.take().ok_or(MossError::Unavailable)?;
    let mut input = child.stdin.take().ok_or(MossError::Unavailable)?;
    let mut buffer = Vec::new();
    let ready = timeout(STARTUP_TIMEOUT, line(&mut output, &mut buffer, 4096))
        .await
        .map_err(|_| MossError::Unavailable)??;
    if ready["event"] != "ready"
        || ready["protocolVersion"] != 1
        || ready["modelId"] != model
        || ready["revision"] != revision
        || ready["runtime"] != runtime
    {
        return Err(MossError::InvalidOutput);
    }
    let mut request = serde_json::to_vec(
        &serde_json::json!({"prompt":prompt,"width":size.0,"height":size.1,
        "seed":seed,"steps":50}),
    )
    .map_err(|_| MossError::InvalidOutput)?;
    request.push(b'\n');
    timeout(WRITE_TIMEOUT, input.write_all(&request))
        .await
        .map_err(|_| MossError::Unavailable)?
        .map_err(|_| MossError::Unavailable)?;
    drop(input);
    progress();
    let mut progress_count = 0;
    let png = loop {
        let event = line(&mut output, &mut buffer, MAX_EVENT_BYTES).await?;
        match event["event"].as_str() {
            Some("progress") if progress_count < MAX_PROGRESS_EVENTS => {
                progress_count += 1;
                progress();
            }
            Some("complete") => {
                break protocol::decode_complete(&event, model, revision, size, seed)?;
            }
            _ => return Err(MossError::InvalidOutput),
        }
    };
    if !buffer.is_empty() {
        return Err(MossError::InvalidOutput);
    }
    let mut trailing = [0; 1];
    if timeout(EXIT_TIMEOUT, output.read(&mut trailing))
        .await
        .map_err(|_| MossError::Unavailable)?
        .map_err(|_| MossError::Unavailable)?
        != 0
    {
        return Err(MossError::InvalidOutput);
    }
    if !timeout(EXIT_TIMEOUT, child.wait())
        .await
        .map_err(|_| MossError::Unavailable)?
        .map_err(|_| MossError::Unavailable)?
        .success()
    {
        return Err(MossError::Unavailable);
    }
    Ok(png)
}

/// Bound the complete line, including fragmented output, without losing bytes across canceled reads.
async fn line(
    output: &mut (impl AsyncRead + Unpin),
    buffer: &mut Vec<u8>,
    limit: usize,
) -> Result<serde_json::Value, MossError> {
    loop {
        if let Some(end) = buffer.iter().position(|b| *b == b'\n') {
            if end > limit {
                return Err(MossError::InvalidOutput);
            }
            let result =
                serde_json::from_slice(&buffer[..end]).map_err(|_| MossError::InvalidOutput)?;
            buffer.drain(..=end);
            return Ok(result);
        }
        if buffer.len() > limit {
            return Err(MossError::InvalidOutput);
        }
        let mut bytes = [0; READ_BYTES];
        let count = output
            .read(&mut bytes)
            .await
            .map_err(|_| MossError::Unavailable)?;
        if count == 0 {
            return Err(MossError::InvalidOutput);
        }
        buffer.extend_from_slice(&bytes[..count]);
    }
}
/// A canceled, failed, timed-out, or successful worker cannot outlive its owned generation run.
async fn reap(child: &mut Child) {
    loop {
        let _ = child.start_kill();
        if matches!(timeout(EXIT_TIMEOUT, child.wait()).await, Ok(Ok(_))) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
/// Dropping a cancellation sender is not itself an instruction to abort valid work.
async fn cancelled(cancel: &mut watch::Receiver<bool>) {
    while !*cancel.borrow_and_update() {
        if cancel.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn fragmented_and_oversized_output_fail_closed() {
        let mut stream = &b"{\"event\":\"ready\"}\n"[..];
        assert_eq!(
            line(&mut stream, &mut Vec::new(), 64).await.unwrap()["event"],
            "ready"
        );
        let mut stream = &b"{\"event\":\"ready\"}\n"[..];
        assert!(line(&mut stream, &mut Vec::new(), 8).await.is_err());
        let mut stream = &b"{"[..];
        assert!(line(&mut stream, &mut Vec::new(), 8).await.is_err());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn completion_requires_successful_exit_and_no_extra_output() {
        let ready = r#"{"event":"ready","protocolVersion":1,"modelId":"model","revision":"rev","runtime":"mlx"}"#;
        let complete = r#"{"event":"complete","modelId":"model","revision":"rev","width":512,"height":512,"seed":42,"png":"iVBORw0KGgo="}"#;
        for (suffix, success) in [
            ("exit 0", true),
            ("exit 7", false),
            ("printf extra; exit 0", false),
        ] {
            let script = format!(
                "printf '%s\\n' '{ready}'; read -r request; printf '%s\\n' '{complete}'; {suffix}"
            );
            let mut command = Command::new("/bin/sh");
            command.args(["-c", &script]);
            let (_keep, receiver) = watch::channel(false);
            assert_eq!(
                run(
                    command,
                    "layout",
                    (512, 512),
                    42,
                    "model",
                    "rev",
                    "mlx",
                    receiver,
                    || {}
                )
                .await
                .is_ok(),
                success
            );
        }
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_reaps_even_a_child_stuck_before_handshake() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "exec sleep 120"]);
        let (sender, receiver) = watch::channel(false);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            sender.send(true).unwrap();
        });
        let start = std::time::Instant::now();
        assert_eq!(
            run(
                command,
                "hello",
                (512, 512),
                42,
                "model",
                "rev",
                "mlx",
                receiver,
                || {}
            )
            .await,
            Err(MossError::Cancelled)
        );
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}
