//! `@kastellan:` requests: hands a task to a Kastellan agent daemon and saves its reply into the conversation.
//!
//! A request becomes an ordinary provider run (`provider_id = "kastellan"`, `model_id` = the host), so the reply lands
//! as the durable assistant response to the exact user message that asked for it. The run is accepted immediately and
//! completes in the background, possibly minutes later; completion is announced with an application-wide event so the
//! WebView can reload the conversation. Bottie's one-running-response-per-conversation rule still applies, so the
//! conversation is busy until Kastellan answers while every other conversation stays usable.
//!
//! The transport is Kastellan's operator CLI, `kastellan-cli ask`, run directly for a loopback host or over `ssh`;
//! see [`transport`]. The host is chosen in Settings and saved as `kastellan.json`; see [`config`].

pub(crate) mod config;
pub(crate) mod settings;
pub(crate) mod transport;

use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::{
    AppState,
    diagnostics::record_diagnostic,
    inference::ProviderError,
    storage::{
        ConversationStore, NewProviderRun, ProviderRunContext, ProviderRunState, RunBlockKind,
        StorageError, StoredReasoningEffort,
    },
};
use config::load_config;
use settings::{config_path, home_directory};
use transport::{Action, TaskOutcome, execute, instruction_from_prompt, plan_invocation};

/// Stable provider identity recorded on Kastellan runs.
pub(crate) const KASTELLAN_PROVIDER_ID: &str = "kastellan";
/// Application event emitted after a Kastellan run reaches a terminal durable state.
pub(crate) const KASTELLAN_TASK_EVENT: &str = "kastellan-task-finished";
/// Upper bound on one task, after which the CLI is killed. The Kastellan task itself keeps running.
const TASK_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// Durable error code for a run whose outcome could not be saved.
const SAVE_FAILED_CODE: &str = "internal";
/// Host recorded on a run whose settings could not be read; the saved response explains why.
const UNCONFIGURED_HOST: &str = "unconfigured";

/// Identity returned when a request is accepted, including where it was sent.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KastellanRun {
    pub(crate) run_id: String,
    pub(crate) host: String,
}

/// Event payload telling the WebView which conversation changed.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KastellanTaskFinished {
    pub(crate) conversation_id: String,
    pub(crate) run_id: String,
}

/// Plans the CLI invocation for one prompt; every error here becomes the saved response.
pub(crate) fn prepare(
    config: &config::KastellanConfig,
    prompt: &str,
    home: Option<&std::path::Path>,
) -> Result<transport::Invocation, ProviderError> {
    plan_invocation(config, Action::Ask(instruction_from_prompt(prompt)?), home)
}

/// Formats the one-line provenance saved with every reply.
pub(crate) fn provenance(task_id: Option<&str>, host: &str, elapsed: Duration) -> String {
    format!(
        "Kastellan task {} on {host} · {}s",
        task_id.unwrap_or("(id unknown)"),
        elapsed.as_secs()
    )
}

/// Writes the outcome into the run's response and closes the run.
///
/// The provenance line is stored in the response's reasoning block: it is durable, it never enters model context
/// (only answer text does), and presentation shows it as the response footer. If saving fails part-way, the run is
/// still closed as failed, because a run left running holds its conversation busy until Bottie restarts.
pub(crate) fn persist_outcome(
    store: &impl RunStore,
    run_id: &str,
    outcome: &TaskOutcome,
    provenance: &str,
) -> Result<(), StorageError> {
    let saved = save_outcome(store, run_id, outcome, provenance);
    if saved.is_err() {
        let _ = store.finish(run_id, ProviderRunState::Failed, Some(SAVE_FAILED_CODE));
    }
    saved
}

/// Checkpoints provenance and text, then closes the run with the outcome's state.
fn save_outcome(
    store: &impl RunStore,
    run_id: &str,
    outcome: &TaskOutcome,
    provenance: &str,
) -> Result<(), StorageError> {
    store.checkpoint(run_id, RunBlockKind::Reasoning, provenance)?;
    let (text, state, code) = match outcome {
        TaskOutcome::Reply(text) => (text, ProviderRunState::Completed, None),
        TaskOutcome::Failed { text, code } => (text, ProviderRunState::Failed, Some(*code)),
    };
    store.checkpoint(run_id, RunBlockKind::Text, text)?;
    store.finish(run_id, state, code)
}

/// The two run operations an outcome needs, so failure recovery can be tested without breaking SQLite.
pub(crate) trait RunStore {
    /// Appends one block to the run's response.
    fn checkpoint(&self, run_id: &str, kind: RunBlockKind, delta: &str)
    -> Result<(), StorageError>;
    /// Closes the run with a terminal state.
    fn finish(
        &self,
        run_id: &str,
        state: ProviderRunState,
        code: Option<&str>,
    ) -> Result<(), StorageError>;
}

impl RunStore for ConversationStore {
    fn checkpoint(
        &self,
        run_id: &str,
        kind: RunBlockKind,
        delta: &str,
    ) -> Result<(), StorageError> {
        self.checkpoint_provider_delta(run_id, kind, delta)
    }

    fn finish(
        &self,
        run_id: &str,
        state: ProviderRunState,
        code: Option<&str>,
    ) -> Result<(), StorageError> {
        self.finish_provider_run(run_id, state, code, None)
    }
}

#[tauri::command]
/// Accepts one `@kastellan:` request as a provider run and completes it in the background.
pub(crate) async fn start_kastellan_task(
    app: AppHandle,
    state: State<'_, AppState>,
    context: ProviderRunContext,
    prompt: String,
) -> Result<KastellanRun, ProviderError> {
    let config = load_config(&config_path(&app)?);
    let host = config.as_ref().map_or_else(
        |_| UNCONFIGURED_HOST.to_owned(),
        |config| config.host().to_owned(),
    );
    let run_id = uuid::Uuid::new_v4().to_string();
    state
        .conversations
        .start_provider_run(NewProviderRun {
            id: run_id.clone(),
            conversation_id: context.conversation_id.clone(),
            request_message_id: context.request_message_id.clone(),
            provider_id: KASTELLAN_PROVIDER_ID.into(),
            model_id: host.clone(),
            reasoning_effort: StoredReasoningEffort::Off,
            temperature: None,
            max_output_tokens: None,
        })
        .map_err(|error| ProviderError::invalid_request(error.message))?;
    // From here on every failure is saved as the response, so a request never loses its answer.
    let prepared = config.and_then(|config| prepare(&config, &prompt, home_directory().as_deref()));

    let conversations = state.conversations.clone();
    let diagnostics = state.diagnostics.clone();
    let task_run_id = run_id.clone();
    let task_host = host.clone();
    tauri::async_runtime::spawn(async move {
        let started = Instant::now();
        let (outcome, task_id) = match prepared {
            Ok(invocation) => execute(&invocation, TASK_TIMEOUT).await,
            Err(error) => (
                TaskOutcome::Failed {
                    text: error.message,
                    code: error.code.as_str(),
                },
                None,
            ),
        };
        let line = provenance(task_id.as_deref(), &task_host, started.elapsed());
        let detail = match persist_outcome(&conversations, &task_run_id, &outcome, &line) {
            Ok(()) => line,
            Err(error) => format!("{line}; not saved: {}", error.message),
        };
        let level = match outcome {
            TaskOutcome::Reply(_) => "info",
            TaskOutcome::Failed { .. } => "error",
        };
        record_diagnostic(
            &diagnostics,
            level,
            "Kastellan task finished",
            Some(KASTELLAN_PROVIDER_ID),
            Some(&detail),
        )
        .await;
        let _ = app.emit(
            KASTELLAN_TASK_EVENT,
            KastellanTaskFinished {
                conversation_id: context.conversation_id,
                run_id: task_run_id,
            },
        );
    });
    Ok(KastellanRun { run_id, host })
}

#[cfg(test)]
mod tests;
