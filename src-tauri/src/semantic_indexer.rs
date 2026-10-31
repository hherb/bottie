//! Process-lifetime scheduling for resumable native semantic indexing.

use std::{
    path::{Path, PathBuf},
    sync::mpsc::{Receiver, Sender, channel},
    thread,
    time::Duration,
};

use crate::{
    diagnostics::{Diagnostics, record_diagnostic},
    semantic_model::FastEmbedder,
    storage::{
        ConversationStore, DEFAULT_SEMANTIC_BATCH_SIZE, SemanticEmbedder, SemanticIndexState,
        StorageError,
    },
    worker_pause::{PAUSE_ACKNOWLEDGEMENT_WAIT, WorkerHealth, confirm_pause},
};

const WORKER_THREAD_NAME: &str = "bottie-semantic-indexing";
/// Maximum time a foreground memory lookup waits behind semantic indexing or model preparation.
const QUERY_EMBEDDING_WAIT: Duration = Duration::from_secs(5);

/// Cheap wake handle for Bottie's single semantic-index worker.
#[derive(Clone)]
pub(crate) struct SemanticIndexer {
    sender: Sender<WorkerCommand>,
    health: WorkerHealth,
}

/// Synchronous embedding proxy serviced by Bottie's single model-owning worker thread.
#[derive(Clone)]
pub(crate) struct SemanticQueryEmbedder {
    sender: Sender<WorkerCommand>,
}

/// Commands serialized by the single native worker thread.
enum WorkerCommand {
    Wake,
    Pause(Sender<()>),
    Resume,
    Embed {
        texts: Vec<String>,
        response: Sender<Result<Vec<Vec<f32>>, String>>,
    },
}

/// Scope guard that resumes semantic indexing even when restore returns an error.
pub(crate) struct SemanticIndexingPause {
    sender: Sender<WorkerCommand>,
}

impl Drop for SemanticIndexingPause {
    fn drop(&mut self) {
        let _ = self.sender.send(WorkerCommand::Resume);
    }
}

impl SemanticEmbedder for SemanticQueryEmbedder {
    /// Sends one bounded query batch to the process-lifetime model owner and waits for its result.
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let (response, result) = channel();
        self.sender
            .send(WorkerCommand::Embed {
                texts: texts.to_vec(),
                response,
            })
            .map_err(|_| "embedding_worker".to_owned())?;
        receive_embedding_response(result, QUERY_EMBEDDING_WAIT)
    }
}

/// Receives one worker-owned query embedding without allowing foreground retrieval to hang.
fn receive_embedding_response(
    result: Receiver<Result<Vec<Vec<f32>>, String>>,
    timeout: Duration,
) -> Result<Vec<Vec<f32>>, String> {
    result
        .recv_timeout(timeout)
        .map_err(|_| "embedding_worker".to_owned())?
}

impl SemanticIndexer {
    /// Starts one process-lifetime worker without loading or downloading the model inline.
    pub(crate) fn start(
        cache_dir: PathBuf,
        conversations: ConversationStore,
        diagnostics: Diagnostics,
    ) -> Self {
        let (sender, receiver) = channel();
        thread::Builder::new()
            .name(WORKER_THREAD_NAME.into())
            .spawn({
                let diagnostics = diagnostics.clone();
                move || run_worker(receiver, cache_dir, conversations, diagnostics)
            })
            .expect("the semantic indexing worker must start");
        Self::with_sender(sender, diagnostics)
    }

    /// Wraps the worker's command queue; a stop is reported under this worker's own event name.
    fn with_sender(sender: Sender<WorkerCommand>, diagnostics: Diagnostics) -> Self {
        Self {
            sender,
            health: WorkerHealth::new(diagnostics, "Semantic indexing stopped"),
        }
    }

    /// Wakes the worker after startup, source changes, or a completed store restore.
    pub(crate) fn wake(&self) {
        if self.sender.send(WorkerCommand::Wake).is_err() {
            self.health.mark_stopped();
        }
    }

    /// Whether the worker thread was seen to stop, so derived vectors cleared now would never be rebuilt.
    pub(crate) fn is_stopped(&self) -> bool {
        self.health.is_stopped()
    }

    /// Returns a cheap proxy for query embeddings through the existing model-owning worker.
    pub(crate) fn query_embedder(&self) -> SemanticQueryEmbedder {
        SemanticQueryEmbedder {
            sender: self.sender.clone(),
        }
    }

    /// Waits for any current batch to finish, then pauses work for one store replacement.
    ///
    /// Returns a busy error asking the user to retry when the worker does not pause within [`PAUSE_ACKNOWLEDGEMENT_WAIT`].
    pub(crate) fn pause(&self) -> Result<SemanticIndexingPause, StorageError> {
        self.pause_within(PAUSE_ACKNOWLEDGEMENT_WAIT)
    }

    fn pause_within(&self, timeout: Duration) -> Result<SemanticIndexingPause, StorageError> {
        let (acknowledge, acknowledged) = channel();
        let sent = self.sender.send(WorkerCommand::Pause(acknowledge)).is_ok();
        // Created before waiting so a timeout drops it, queueing Resume behind the late Pause.
        let pause = SemanticIndexingPause {
            sender: self.sender.clone(),
        };
        confirm_pause(sent, &acknowledged, timeout, &self.health)?;
        Ok(pause)
    }
}

/// Drains requested work in bounded durable batches so restore pauses remain prompt.
fn run_worker(
    receiver: Receiver<WorkerCommand>,
    cache_dir: PathBuf,
    conversations: ConversationStore,
    diagnostics: Diagnostics,
) {
    let mut wake_requested = false;
    let mut paused = false;
    let mut embedder = None;
    loop {
        while let Ok(command) = receiver.try_recv() {
            handle_command(
                command,
                &cache_dir,
                &mut embedder,
                &mut wake_requested,
                &mut paused,
            );
        }
        if !paused && wake_requested {
            match process_requested_work(&cache_dir, &conversations, &diagnostics, &mut embedder) {
                WorkerOutcome::Continue => continue,
                WorkerOutcome::Idle | WorkerOutcome::Failed => wake_requested = false,
            }
        }
        let Ok(command) = receiver.recv() else {
            break;
        };
        handle_command(
            command,
            &cache_dir,
            &mut embedder,
            &mut wake_requested,
            &mut paused,
        );
    }
}

/// Loads the model on first use, returning the load's own error code (such as `model_cache_dir`) on failure (#226).
fn loaded_embedder<'a>(
    cache_dir: &Path,
    embedder: &'a mut Option<FastEmbedder>,
) -> Result<&'a mut FastEmbedder, String> {
    match embedder {
        Some(embedder) => Ok(embedder),
        None => Ok(embedder.insert(FastEmbedder::load(cache_dir.to_path_buf())?)),
    }
}

/// Result of one bounded worker iteration.
enum WorkerOutcome {
    Continue,
    Idle,
    Failed,
}

/// Loads the model lazily and commits at most one semantic batch.
fn process_requested_work(
    cache_dir: &PathBuf,
    conversations: &ConversationStore,
    diagnostics: &Diagnostics,
    embedder: &mut Option<FastEmbedder>,
) -> WorkerOutcome {
    let progress = match conversations.semantic_index_progress() {
        Ok(progress) => progress,
        Err(error) => {
            record_failure(diagnostics.clone(), "Semantic indexing paused", error.code);
            return WorkerOutcome::Failed;
        }
    };
    if progress.total_chunks == progress.completed_chunks
        && progress.state == SemanticIndexState::Ready
    {
        return WorkerOutcome::Idle;
    }
    if embedder.is_none() {
        if let Err(error) = conversations.mark_semantic_model_loading() {
            record_failure(diagnostics.clone(), "Semantic indexing paused", error.code);
            return WorkerOutcome::Failed;
        }
        record_loading(diagnostics.clone());
        match FastEmbedder::load(cache_dir.clone()) {
            Ok(loaded) => *embedder = Some(loaded),
            Err(code) => {
                // An unsaved failure leaves the settings view on "Preparing local model", so say so.
                let code = match conversations.mark_semantic_failed(&code) {
                    Ok(()) => code,
                    Err(error) => format!("{code}; state not saved: {}", error.code),
                };
                record_failure(diagnostics.clone(), "Local memory model unavailable", &code);
                return WorkerOutcome::Failed;
            }
        }
    }
    let Some(embedder) = embedder.as_mut() else {
        return WorkerOutcome::Failed;
    };
    match conversations.process_next_semantic_batch(embedder, DEFAULT_SEMANTIC_BATCH_SIZE) {
        Ok(Some(_)) => WorkerOutcome::Continue,
        Ok(None) => {
            record_ready(diagnostics.clone());
            WorkerOutcome::Idle
        }
        Err(error) => {
            record_failure(diagnostics.clone(), "Semantic indexing paused", error.code);
            WorkerOutcome::Failed
        }
    }
}

/// Applies one scheduler command without touching chunk or model content.
fn apply_command(command: WorkerCommand, wake_requested: &mut bool, paused: &mut bool) {
    match command {
        WorkerCommand::Wake => *wake_requested = true,
        WorkerCommand::Pause(acknowledge) => {
            *paused = true;
            let _ = acknowledge.send(());
        }
        WorkerCommand::Resume => *paused = false,
        WorkerCommand::Embed { response, .. } => {
            let _ = response.send(Err("embedding_worker".into()));
        }
    }
}

/// Applies control commands or services one query embedding through the worker-owned runtime.
fn handle_command(
    command: WorkerCommand,
    cache_dir: &PathBuf,
    embedder: &mut Option<FastEmbedder>,
    wake_requested: &mut bool,
    paused: &mut bool,
) {
    match command {
        WorkerCommand::Embed { texts, response } => {
            let result = if *paused {
                Err("embedding_paused".into())
            } else {
                loaded_embedder(cache_dir, embedder).and_then(|embedder| embedder.embed(&texts))
            };
            let _ = response.send(result);
        }
        control => apply_command(control, wake_requested, paused),
    }
}

/// Records model-cache preparation without exposing paths or model-file identities.
fn record_loading(diagnostics: Diagnostics) {
    tauri::async_runtime::spawn(async move {
        record_diagnostic(
            &diagnostics,
            "info",
            "Preparing local memory model",
            None,
            Some("Bottie is checking or populating its application-owned EmbeddingGemma cache."),
        )
        .await;
    });
}

/// Records a stable path-free worker failure without blocking the native thread.
fn record_failure(diagnostics: Diagnostics, event: &'static str, code: &str) {
    let detail = failure_detail(code);
    tauri::async_runtime::spawn(async move {
        record_diagnostic(&diagnostics, "error", event, None, Some(&detail)).await;
    });
}

/// Retry note for a failure diagnostic, naming the path-free error code (#231).
fn failure_detail(code: &str) -> String {
    format!("Bottie will retry the local semantic index after its next wake. ({code})")
}

/// Records completion counts without exposing chunks, model paths, or cache paths.
fn record_ready(diagnostics: Diagnostics) {
    tauri::async_runtime::spawn(async move {
        record_diagnostic(
            &diagnostics,
            "info",
            "Local semantic index current",
            None,
            Some("Every eligible memory chunk has a durable local vector."),
        )
        .await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_model_load_returns_its_own_code_and_retries_next_time() {
        let blocker =
            std::env::temp_dir().join(format!("bottie-cache-blocker-{}", uuid::Uuid::new_v4()));
        std::fs::write(&blocker, b"not a directory").expect("blocking file should be created");
        let mut embedder = None;

        let error = loaded_embedder(&blocker.join("cache"), &mut embedder).err();

        assert_eq!(error.as_deref(), Some("model_cache_dir"));
        assert!(embedder.is_none());
        let _ = std::fs::remove_file(blocker);
    }

    #[test]
    fn failure_detail_names_the_failure_code() {
        assert_eq!(
            failure_detail("model_cache_read"),
            "Bottie will retry the local semantic index after its next wake. (model_cache_read)"
        );
    }

    #[test]
    fn pause_times_out_against_a_silent_worker_and_queues_resume() {
        let (sender, receiver) = channel();
        let indexer = SemanticIndexer::with_sender(sender, Diagnostics::default());
        let started = std::time::Instant::now();

        let error = indexer
            .pause_within(Duration::from_millis(20))
            .err()
            .expect("an unacknowledged pause must time out");

        assert_eq!(error.code, "invalid_request");
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(matches!(receiver.try_recv(), Ok(WorkerCommand::Pause(_))));
        assert!(matches!(receiver.try_recv(), Ok(WorkerCommand::Resume)));
    }

    #[test]
    fn a_dead_worker_is_reported_as_semantic_indexing_stopped() {
        let (sender, receiver) = channel();
        let indexer = SemanticIndexer::with_sender(sender, Diagnostics::default());
        drop(receiver);

        indexer.wake();
        assert!(indexer.pause_within(Duration::ZERO).is_ok());

        assert!(indexer.is_stopped());
        assert_eq!(
            crate::worker_pause::tests::recorded_events(indexer.health.diagnostics()),
            ["Semantic indexing stopped"]
        );
    }

    #[test]
    fn pause_preserves_coalesced_work_until_resume() {
        let mut wake_requested = false;
        let mut paused = false;
        apply_command(WorkerCommand::Wake, &mut wake_requested, &mut paused);
        let (acknowledge, acknowledged) = channel();
        apply_command(
            WorkerCommand::Pause(acknowledge),
            &mut wake_requested,
            &mut paused,
        );

        assert!(wake_requested);
        assert!(paused);
        acknowledged.recv().expect("pause should acknowledge");

        apply_command(WorkerCommand::Resume, &mut wake_requested, &mut paused);
        assert!(wake_requested);
        assert!(!paused);
    }

    #[test]
    fn embedding_response_wait_is_bounded() {
        let (_response, result) = channel::<Result<Vec<Vec<f32>>, String>>();
        assert_eq!(
            receive_embedding_response(result, std::time::Duration::ZERO),
            Err("embedding_worker".into())
        );
    }
}
