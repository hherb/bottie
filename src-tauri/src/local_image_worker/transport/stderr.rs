//! Bounded discard-only draining for local image-worker stderr.

use tokio::{io::AsyncReadExt, sync::mpsc};

/// Maximum private stderr bytes tolerated before the worker is terminated.
const MAX_STDERR_BYTES: usize = 64 * 1_024;
/// Fixed buffer used to drain stderr without retaining its contents.
const STDERR_READ_BYTES: usize = 8 * 1_024;

/// Failure notifications from the independent stderr drainer.
#[derive(Clone, Copy, Debug)]
pub(super) enum StderrSignal {
    /// The worker emitted more stderr than the fixed ceiling.
    LimitExceeded,
    /// The private stderr pipe failed while being drained.
    ReadFailed,
}

/// Discards stderr continuously while reporting one bounded failure signal.
pub(super) async fn drain_stderr(
    mut stderr: tokio::process::ChildStderr,
    sender: mpsc::Sender<StderrSignal>,
) {
    let mut total = 0_usize;
    let mut reported_limit = false;
    let mut bytes = [0_u8; STDERR_READ_BYTES];
    loop {
        match stderr.read(&mut bytes).await {
            Ok(0) => return,
            Ok(count) => {
                total = total.saturating_add(count);
                if total > MAX_STDERR_BYTES && !reported_limit {
                    let _ = sender.try_send(StderrSignal::LimitExceeded);
                    reported_limit = true;
                }
            }
            Err(_) => {
                let _ = sender.try_send(StderrSignal::ReadFailed);
                return;
            }
        }
    }
}
