//! Process-lifetime scheduling for resumable native semantic indexing.

use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, Sender, channel},
    thread,
};

use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

use crate::{
    diagnostics::{Diagnostics, record_diagnostic},
    storage::{
        ConversationStore, DEFAULT_SEMANTIC_BATCH_SIZE, SemanticEmbedder, SemanticIndexState,
    },
};

const WORKER_THREAD_NAME: &str = "bottie-semantic-indexing";
const EMBEDDING_RUNTIME_THREADS: usize = 2;

/// Cheap wake handle for Bottie's single semantic-index worker.
#[derive(Clone)]
pub(crate) struct SemanticIndexer {
    sender: Sender<WorkerCommand>,
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

/// Production adapter around Bottie's one built-in FastEmbed model.
struct FastEmbedder {
    model: TextEmbedding,
}

impl FastEmbedder {
    /// Loads or downloads the Q4 EmbeddingGemma model through the app-owned cache directory.
    fn load(cache_dir: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&cache_dir).map_err(|_| "model_cache")?;
        let options = TextInitOptions::new(EmbeddingModel::EmbeddingGemma300MQ4)
            .with_cache_dir(cache_dir)
            .with_show_download_progress(false)
            .with_intra_threads(EMBEDDING_RUNTIME_THREADS);
        TextEmbedding::try_new(options)
            .map(|model| Self { model })
            .map_err(|_| "model_runtime".into())
    }
}

impl SemanticEmbedder for FastEmbedder {
    /// Produces normalized EmbeddingGemma vectors for versioned document inputs.
    fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        self.model
            .embed(texts, None)
            .map_err(|_| "embedding_runtime".into())
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
        result.recv().map_err(|_| "embedding_worker".to_owned())?
    }
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
            .spawn(move || run_worker(receiver, cache_dir, conversations, diagnostics))
            .expect("the semantic indexing worker must start");
        Self { sender }
    }

    /// Wakes the worker after startup, source changes, or a completed store restore.
    pub(crate) fn wake(&self) {
        let _ = self.sender.send(WorkerCommand::Wake);
    }

    /// Returns a cheap proxy for query embeddings through the existing model-owning worker.
    pub(crate) fn query_embedder(&self) -> SemanticQueryEmbedder {
        SemanticQueryEmbedder {
            sender: self.sender.clone(),
        }
    }

    /// Waits for any current batch to finish, then pauses work for one store replacement.
    pub(crate) fn pause(&self) -> SemanticIndexingPause {
        let (acknowledge, acknowledged) = channel();
        if self.sender.send(WorkerCommand::Pause(acknowledge)).is_ok() {
            let _ = acknowledged.recv();
        }
        SemanticIndexingPause {
            sender: self.sender.clone(),
        }
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
        Err(_) => {
            record_failure(diagnostics.clone(), "Semantic indexing paused");
            return WorkerOutcome::Failed;
        }
    };
    if progress.total_chunks == progress.completed_chunks
        && progress.state == SemanticIndexState::Ready
    {
        return WorkerOutcome::Idle;
    }
    if embedder.is_none() {
        if conversations.mark_semantic_model_loading().is_err() {
            record_failure(diagnostics.clone(), "Semantic indexing paused");
            return WorkerOutcome::Failed;
        }
        record_loading(diagnostics.clone());
        match FastEmbedder::load(cache_dir.clone()) {
            Ok(loaded) => *embedder = Some(loaded),
            Err(code) => {
                let _ = conversations.mark_semantic_failed(&code);
                record_failure(diagnostics.clone(), "Local memory model unavailable");
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
        Err(_) => {
            record_failure(diagnostics.clone(), "Semantic indexing paused");
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
                if embedder.is_none() {
                    *embedder = FastEmbedder::load(cache_dir.clone()).ok();
                }
                embedder
                    .as_mut()
                    .ok_or_else(|| "model_runtime".to_owned())
                    .and_then(|embedder| embedder.embed(&texts))
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
fn record_failure(diagnostics: Diagnostics, event: &'static str) {
    tauri::async_runtime::spawn(async move {
        record_diagnostic(
            &diagnostics,
            "error",
            event,
            None,
            Some("Bottie will retry the local semantic index after its next wake."),
        )
        .await;
    });
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
}
