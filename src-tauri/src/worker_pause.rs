//! Shared pause handshake for Bottie's background store workers (attachment processing, semantic indexing).
//!
//! Restore and reindex ask each worker to pause before touching the store. A worker answers on a one-shot
//! channel once its current item is done. If the worker thread has died, nothing will ever answer: that still
//! counts as paused (a dead thread cannot write to the store), but the user must hear that background work has
//! stopped, so it is recorded as a diagnostic.

use std::{
    sync::mpsc::{Receiver, RecvTimeoutError},
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

/// Confirms a pause request: `sent` is whether the pause command reached the worker's queue at all.
///
/// Returns `Ok` when the worker paused or is gone (recording `event` in the latter case so the stop is not
/// silent), and a busy error when it is alive but did not pause in time.
pub(crate) fn confirm_pause(
    sent: bool,
    acknowledged: &Receiver<()>,
    timeout: Duration,
    diagnostics: &Diagnostics,
    event: &'static str,
) -> Result<(), StorageError> {
    let worker_gone = !sent
        || await_pause_acknowledgement(acknowledged, timeout)? == PauseAcknowledgement::WorkerGone;
    if worker_gone {
        let diagnostics = diagnostics.clone();
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
    fn an_acknowledged_pause_records_nothing() {
        let diagnostics = Diagnostics::default();
        let (acknowledge, acknowledged) = channel();
        acknowledge.send(()).unwrap();

        assert!(
            confirm_pause(true, &acknowledged, Duration::ZERO, &diagnostics, "stopped").is_ok()
        );
        std::thread::sleep(Duration::from_millis(20));
        assert!(diagnostics.try_lock().unwrap().is_empty());
    }

    #[test]
    fn an_unsent_pause_counts_as_paused_and_is_reported() {
        let diagnostics = Diagnostics::default();
        let (_acknowledge, acknowledged) = channel();

        assert!(
            confirm_pause(
                false,
                &acknowledged,
                Duration::ZERO,
                &diagnostics,
                "stopped"
            )
            .is_ok()
        );
        assert_eq!(recorded_events(&diagnostics), ["stopped"]);
    }
}
