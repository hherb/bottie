//! Process-wide one-active image-run reservation and cancellation ownership.

use std::sync::Arc;

use futures_util::future::{AbortHandle, AbortRegistration};
use tokio::sync::{MutexGuard, watch};

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
        clear_if_owned(&mut *self.active.lock().await, run_id);
    }

    /// Returns a guard that clears the slot when the owning task ends, panics included.
    pub(crate) fn finish_on_drop(&self, run_id: String) -> ImageRunCompletion {
        ImageRunCompletion {
            runs: self.clone(),
            run_id,
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

    /// Holds the empty slot so no image run can reserve until the guard drops; `None` while a run owns it.
    pub(crate) async fn lock_idle(&self) -> Option<IdleImageRuns<'_>> {
        let active = self.active.lock().await;
        active.is_none().then_some(IdleImageRuns { _slot: active })
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

/// Proof that the image slot is empty, held to keep it so.
pub(crate) struct IdleImageRuns<'a> {
    _slot: MutexGuard<'a, Option<ActiveImageRun>>,
}

/// Clears one image run's slot on drop so a panicked task cannot hold it, and block restore, forever.
pub(crate) struct ImageRunCompletion {
    runs: ImageGenerationRuns,
    run_id: String,
}

impl Drop for ImageRunCompletion {
    fn drop(&mut self) {
        let run_id = std::mem::take(&mut self.run_id);
        if let Ok(mut active) = self.runs.active.try_lock() {
            clear_if_owned(&mut active, &run_id);
            return;
        }
        let runs = self.runs.clone();
        tauri::async_runtime::spawn(async move { runs.finish(&run_id).await });
    }
}

fn clear_if_owned(active: &mut Option<ActiveImageRun>, run_id: &str) {
    if active.as_ref().is_some_and(|run| run.run_id == run_id) {
        *active = None;
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

    #[test]
    fn a_panicking_image_task_still_releases_the_slot() {
        tauri::async_runtime::block_on(async {
            let runs = ImageGenerationRuns::default();
            let _registration = runs.reserve_abortable("first".into()).await.unwrap();
            let completion = runs.finish_on_drop("first".into());

            let task = tauri::async_runtime::spawn(async move {
                let _completion = completion;
                panic!("image task panicked");
            });

            assert!(task.await.is_err());
            assert!(!runs.is_active().await);
        });
    }
}
