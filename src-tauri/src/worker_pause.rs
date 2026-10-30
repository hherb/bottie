//! Shared pause handshake for Bottie's background store workers (attachment processing, semantic indexing).
//!
//! Restore pauses both workers, and reindex the semantic worker, before touching the store. A worker answers on a
//! one-shot channel once its current item or batch is done. If the worker thread has died, nothing will ever answer:
//! that still counts as paused (a dead thread cannot write to the store), but the user must hear that background
//! work has stopped, so [`WorkerHealth`] records it as a diagnostic, once.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError},
    },
    time::Duration,
};

use crate::{
    diagnostics::{Diagnostics, record_diagnostic},
    storage::StorageError,
};

/// Maximum time restore or reindex waits for a background worker to reach a pause point, e.g. behind a
/// first-run embedding-model download, before returning a "busy" error that asks the user to retry.
pub(crate) const PAUSE_ACKNOWLEDGEMENT_WAIT: Duration = Duration::from_secs(20);

/// What a pause wait observed.
#[derive(Debug, PartialEq, Eq)]
enum PauseAcknowledgement {
    /// The worker confirmed it has paused.
    Paused,
    /// The worker thread is gone: it cannot touch the store, and it will not process work again either.
    ///
    /// Only a dead thread drops the acknowledgement unsent: every worker's `Pause` handler always sends it.
    WorkerGone,
}

/// Waits up to `timeout` for a worker's pause acknowledgement; a timeout is a busy error the user can retry.
fn await_pause_acknowledgement(
    acknowledged: &Receiver<()>,
    timeout: Duration,
) -> Result<PauseAcknowledgement, StorageError> {
    match acknowledged.recv_timeout(timeout) {
        Ok(()) => Ok(PauseAcknowledgement::Paused),
        Err(RecvTimeoutError::Disconnected) => Ok(PauseAcknowledgement::WorkerGone),
        Err(RecvTimeoutError::Timeout) => Err(StorageError::workers_busy()),
    }
}

/// Whether one background worker's thread has stopped, shared by every clone of that worker's handle.
#[derive(Clone)]
pub(crate) struct WorkerHealth {
    diagnostics: Diagnostics,
    stopped_event: &'static str,
    stopped: Arc<AtomicBool>,
}

impl WorkerHealth {
    /// Tracks a running worker whose stop is reported as `stopped_event`.
    pub(crate) fn new(diagnostics: Diagnostics, stopped_event: &'static str) -> Self {
        Self {
            diagnostics,
            stopped_event,
            stopped: Arc::default(),
        }
    }

    /// Whether the worker thread was seen to stop; it will not process work again this session.
    pub(crate) fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }

    /// Marks the worker stopped, recording the diagnostic only the first time so repeated wakes cannot flood the log.
    pub(crate) fn mark_stopped(&self) {
        if self.stopped.swap(true, Ordering::AcqRel) {
            return;
        }
        let (diagnostics, event) = (self.diagnostics.clone(), self.stopped_event);
        tauri::async_runtime::spawn(async move {
            record_diagnostic(
                &diagnostics,
                "error",
                event,
                None,
                Some("The background worker is no longer running; restart Bottie to resume."),
            )
            .await;
        });
    }

    #[cfg(test)]
    pub(crate) fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }
}

/// Confirms a pause request: `sent` is whether the pause command reached the worker's queue at all.
///
/// Returns `Ok` when the worker paused or is gone (marking `health` stopped in the latter case so the stop is not
/// silent), and a busy error when it is alive but did not pause in time.
pub(crate) fn confirm_pause(
    sent: bool,
    acknowledged: &Receiver<()>,
    timeout: Duration,
    health: &WorkerHealth,
) -> Result<(), StorageError> {
    if !sent
        || await_pause_acknowledgement(acknowledged, timeout)? == PauseAcknowledgement::WorkerGone
    {
        health.mark_stopped();
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::mpsc::channel;

    use super::*;

    /// Waits (bounded) for the asynchronously recorded diagnostic and returns its event names.
    pub(crate) fn recorded_events(diagnostics: &Diagnostics) -> Vec<String> {
        tauri::async_runtime::block_on(async {
            for _ in 0..200 {
                let entries = diagnostics.lock().await;
                if !entries.is_empty() {
                    return entries.iter().map(|entry| entry.event.clone()).collect();
                }
                drop(entries);
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            Vec::new()
        })
    }

    #[test]
    fn acknowledgement_disconnect_and_timeout_are_told_apart() {
        let (acknowledge, acknowledged) = channel();
        acknowledge.send(()).unwrap();
        assert_eq!(
            await_pause_acknowledgement(&acknowledged, Duration::ZERO),
            Ok(PauseAcknowledgement::Paused)
        );

        let (_alive, acknowledged) = channel::<()>();
        let busy =
            await_pause_acknowledgement(&acknowledged, Duration::from_millis(5)).unwrap_err();
        assert_eq!(busy.code, "invalid_request");

        let (dropped, acknowledged) = channel::<()>();
        drop(dropped);
        assert_eq!(
            await_pause_acknowledgement(&acknowledged, Duration::ZERO),
            Ok(PauseAcknowledgement::WorkerGone)
        );
    }

    #[test]
    fn an_acknowledged_or_timed_out_pause_marks_nothing() {
        let health = WorkerHealth::new(Diagnostics::default(), "stopped");
        let (acknowledge, acknowledged) = channel();
        acknowledge.send(()).unwrap();
        assert!(confirm_pause(true, &acknowledged, Duration::ZERO, &health).is_ok());

        // A busy worker is retried, not reported as dead.
        let (_alive, acknowledged) = channel::<()>();
        assert!(confirm_pause(true, &acknowledged, Duration::from_millis(5), &health).is_err());

        assert!(!health.is_stopped());
    }

    #[test]
    fn an_unsent_pause_counts_as_paused_and_is_reported_once() {
        let health = WorkerHealth::new(Diagnostics::default(), "stopped");
        let (_acknowledge, acknowledged) = channel();

        assert!(confirm_pause(false, &acknowledged, Duration::ZERO, &health).is_ok());
        health.clone().mark_stopped();

        assert!(health.is_stopped());
        assert_eq!(recorded_events(health.diagnostics()), ["stopped"]);
    }
}
