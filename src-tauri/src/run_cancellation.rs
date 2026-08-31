//! Shared cancellation registry for active provider and native-tool runs.

use std::{collections::HashMap, sync::Arc};

use futures_util::future::AbortHandle;
use tauri::State;

use crate::{AppState, inference::ProviderError, tool_loop::ToolLoopCancellation};

#[cfg(test)]
mod tests;

/// Cancellation handles shared by provider I/O and native tool work for one accepted generation.
pub(crate) struct ActiveRun {
    pub(crate) abort_handle: AbortHandle,
    pub(crate) tool_cancellation: ToolLoopCancellation,
}

/// Process-wide registry of provider generations that may be cancelled by opaque identity.
pub(crate) type ActiveRuns = Arc<tauri::async_runtime::Mutex<HashMap<String, ActiveRun>>>;

#[tauri::command]
/// Cancels one active generation by its opaque run identity.
pub(crate) async fn cancel_chat(
    run_id: String,
    state: State<'_, AppState>,
) -> Result<bool, ProviderError> {
    if let Some(run) = state.runs.lock().await.remove(&run_id) {
        cancel_run(run);
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Cancels every registered provider and native-tool run before voice capture begins.
pub(crate) async fn cancel_all_chats(runs: &ActiveRuns) -> usize {
    let active_runs = {
        let mut active_runs = runs.lock().await;
        active_runs.drain().map(|(_, run)| run).collect::<Vec<_>>()
    };
    let cancelled_count = active_runs.len();
    for run in active_runs {
        cancel_run(run);
    }
    cancelled_count
}

fn cancel_run(run: ActiveRun) {
    run.tool_cancellation.cancel();
    run.abort_handle.abort();
}
