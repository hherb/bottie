//! Shared cancellation registry for active provider and native-tool runs.

use std::{collections::HashMap, sync::Arc};

use futures_util::future::{AbortHandle, AbortRegistration};
use tauri::State;
use tokio::sync::MutexGuard;

use crate::{
    AppState,
    image_generation::{IdleImageRuns, ImageGenerationRuns},
    inference::ProviderError,
    tool_loop::ToolLoopCancellation,
};

#[cfg(test)]
mod tests;

/// Cancellation handles shared by provider I/O and native tool work for one accepted generation.
pub(crate) struct ActiveRun {
    pub(crate) abort_handle: AbortHandle,
    pub(crate) tool_cancellation: ToolLoopCancellation,
}

/// Process-wide registry of provider generations that may be cancelled by opaque identity.
pub(crate) type ActiveRuns = Arc<tauri::async_runtime::Mutex<HashMap<String, ActiveRun>>>;

/// Unregisters one run when dropped, whether `start_chat` returns early or the run's task ends (panics included),
/// so an orphaned entry can never block restore.
pub(crate) struct ActiveRunRegistration {
    runs: ActiveRuns,
    run_id: String,
}

impl ActiveRunRegistration {
    /// Takes ownership of removing `run_id`; the run stays registered for as long as this is held.
    pub(crate) fn new(runs: ActiveRuns, run_id: String) -> Self {
        Self { runs, run_id }
    }
}

impl Drop for ActiveRunRegistration {
    fn drop(&mut self) {
        let run_id = std::mem::take(&mut self.run_id);
        if let Ok(mut runs) = self.runs.try_lock() {
            runs.remove(&run_id);
            return;
        }
        let runs = self.runs.clone();
        tauri::async_runtime::spawn(async move {
            runs.lock().await.remove(&run_id);
        });
    }
}

/// Registers `run_id` before its first durable write, so restore refuses to replace the store from then on.
/// A restore already past its last check holds the registry, so this waits until that swap has finished.
///
/// Returns the guard that unregisters the run when dropped (every early return releases it) and the abort and
/// tool-cancellation handles the generation task needs. Cancelling before the task starts is fine: the task's
/// `Abortable` then resolves as aborted at once.
pub(crate) async fn reserve_run(
    runs: &ActiveRuns,
    run_id: String,
) -> (
    ActiveRunRegistration,
    AbortRegistration,
    ToolLoopCancellation,
) {
    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    let tool_cancellation = ToolLoopCancellation::default();
    runs.lock().await.insert(
        run_id.clone(),
        ActiveRun {
            abort_handle,
            tool_cancellation: tool_cancellation.clone(),
        },
    );
    (
        ActiveRunRegistration::new(runs.clone(), run_id),
        abort_registration,
        tool_cancellation,
    )
}

/// Reserves `run_id`, then performs the run's first durable write, so that write can never happen while restore
/// could still swap the store underneath it (#216). If the write fails, the returned registration is dropped and
/// the run unregistered.
pub(crate) async fn reserve_then_write<T, E>(
    runs: &ActiveRuns,
    run_id: String,
    first_write: impl FnOnce() -> Result<T, E>,
) -> Result<
    (
        ActiveRunRegistration,
        AbortRegistration,
        ToolLoopCancellation,
        T,
    ),
    E,
> {
    let (registration, abort_registration, tool_cancellation) = reserve_run(runs, run_id).await;
    let written = first_write()?;
    Ok((registration, abort_registration, tool_cancellation, written))
}

/// Proof that no chat or image run is registered; while held, new runs wait in `reserve_run` or `lock_idle`.
pub(crate) struct IdleGeneration<'a> {
    _runs: MutexGuard<'a, HashMap<String, ActiveRun>>,
    _image_runs: IdleImageRuns<'a>,
}

/// Returns a guard that keeps every run out until dropped, or `None` while any chat or image run could still write.
pub(crate) async fn lock_idle_generation<'a>(
    runs: &'a ActiveRuns,
    image_runs: &'a ImageGenerationRuns,
) -> Option<IdleGeneration<'a>> {
    let chat_runs = runs.lock().await;
    if !chat_runs.is_empty() {
        return None;
    }
    Some(IdleGeneration {
        _runs: chat_runs,
        _image_runs: image_runs.lock_idle().await?,
    })
}

#[tauri::command]
/// Cancels one active generation by its opaque run identity.
///
/// The run stays registered until its final state is recorded, so restore keeps refusing to replace the store
/// while that last write is still pending.
pub(crate) async fn cancel_chat(
    run_id: String,
    state: State<'_, AppState>,
) -> Result<bool, ProviderError> {
    if let Some(run) = state.runs.lock().await.get(&run_id) {
        cancel_run(run);
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Signals every registered provider and native-tool run before voice capture begins.
///
/// Each run removes itself once its final state is written, by its task or by `start_chat` refusing it.
pub(crate) async fn cancel_all_chats(runs: &ActiveRuns) -> usize {
    let active_runs = runs.lock().await;
    for run in active_runs.values() {
        cancel_run(run);
    }
    active_runs.len()
}

fn cancel_run(run: &ActiveRun) {
    run.tool_cancellation.cancel();
    run.abort_handle.abort();
}
