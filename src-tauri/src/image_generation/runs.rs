//! Process-wide one-active image-run reservation and cancellation ownership.

use std::sync::Arc;

use futures_util::future::{AbortHandle, AbortRegistration};
use tokio::sync::watch;

/// Process-wide one-active-image-run registry.
#[derive(Clone, Default)]
pub(crate) struct ImageGenerationRuns {
    active: Arc<tauri::async_runtime::Mutex<Option<ActiveImageRun>>>,
}

/// Cancellation mechanism selected by the accepted execution backend.
enum ActiveCancellation {
    Abort(AbortHandle),
    Cooperative(watch::Sender<bool>),
}

/// Cancellation handle for the one accepted image generation.
struct ActiveImageRun {
    run_id: String,
    cancellation: ActiveCancellation,
}

impl ImageGenerationRuns {
    /// Reserves one hosted run and returns its abort registration.
    pub(crate) async fn reserve_abortable(&self, run_id: String) -> Option<AbortRegistration> {
        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        self.reserve(run_id, ActiveCancellation::Abort(abort_handle))
            .await
            .then_some(abort_registration)
    }

    /// Reserves one local run and returns its cooperative cancellation receiver.
    pub(crate) async fn reserve_cooperative(
        &self,
        run_id: String,
    ) -> Option<watch::Receiver<bool>> {
        let (sender, receiver) = watch::channel(false);
        self.reserve(run_id, ActiveCancellation::Cooperative(sender))
            .await
            .then_some(receiver)
    }

    /// Clears the slot only when it still belongs to the completing run.
    pub(crate) async fn finish(&self, run_id: &str) {
        let mut active = self.active.lock().await;
        if active.as_ref().is_some_and(|run| run.run_id == run_id) {
            *active = None;
        }
    }

    /// Cancels the exact active run without accepting provider-owned identifiers.
    pub(crate) async fn cancel(&self, run_id: &str) -> bool {
        let active = self.active.lock().await;
        let Some(run) = active.as_ref() else {
            return false;
        };
        if run.run_id != run_id {
            return false;
        }
        cancel(&run.cancellation);
        true
    }

    /// Cancels the active image run before another mutually exclusive interaction begins.
    pub(crate) async fn cancel_active(&self) -> bool {
        let active = self.active.lock().await;
        let Some(run) = active.as_ref() else {
            return false;
        };
        cancel(&run.cancellation);
        true
    }

    /// Returns whether hosted or local image generation currently owns the process-wide slot.
    pub(crate) async fn is_active(&self) -> bool {
        self.active.lock().await.is_some()
    }

    async fn reserve(&self, run_id: String, cancellation: ActiveCancellation) -> bool {
        let mut active = self.active.lock().await;
        if active.is_some() {
            return false;
        }
        *active = Some(ActiveImageRun {
            run_id,
            cancellation,
        });
        true
    }
}

fn cancel(cancellation: &ActiveCancellation) {
    match cancellation {
        ActiveCancellation::Abort(handle) => handle.abort(),
        ActiveCancellation::Cooperative(sender) => {
            let _ = sender.send(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ImageGenerationRuns;

    #[test]
    fn admits_one_backend_at_a_time_and_cancels_only_the_exact_identity() {
        tauri::async_runtime::block_on(async {
            let runs = ImageGenerationRuns::default();
            let first = runs.reserve_abortable("first".into()).await.unwrap();
            assert!(runs.is_active().await);
            assert!(runs.reserve_cooperative("second".into()).await.is_none());
            assert!(!runs.cancel("wrong").await);
            assert!(runs.cancel("first").await);
            assert!(
                futures_util::future::Abortable::new(futures_util::future::pending::<()>(), first,)
                    .await
                    .is_err()
            );
            assert!(runs.reserve_abortable("blocked".into()).await.is_none());
            runs.finish("first").await;
            assert!(!runs.is_active().await);

            let cooperative = runs.reserve_cooperative("third".into()).await.unwrap();
            assert!(runs.cancel_active().await);
            assert!(*cooperative.borrow());
            runs.finish("third").await;
            assert!(!runs.cancel_active().await);
        });
    }
}
