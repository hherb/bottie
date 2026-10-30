//! Shared cancellation registry for active provider and native-tool runs.

use std::{collections::HashMap, sync::Arc};

use futures_util::future::{AbortHandle, AbortRegistration};
use tauri::State;

use crate::{
    AppState, image_generation::ImageGenerationRuns, inference::ProviderError,
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

/// Unregisters one run when its task ends, panics included, so an orphaned entry can never block restore.
pub(crate) struct ActiveRunRegistration {
    runs: ActiveRuns,
    run_id: String,
}

impl ActiveRunRegistration {
    /// Takes ownership of removing `run_id`; create it inside the task that owns the run.
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

/// Returns whether any chat or image run could still write to the live store.
pub(crate) async fn any_generation_active(
    runs: &ActiveRuns,
    image_runs: &ImageGenerationRuns,
) -> bool {
    !runs.lock().await.is_empty() || image_runs.is_active().await
}

#[tauri::command]
/// Cancels one active generation by its opaque run identity.
///
/// The run stays registered until its task records the final cancelled state, so restore keeps refusing to
/// replace the store while that last write is still pending.
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
/// Each run removes itself once its task has written its final state.
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
