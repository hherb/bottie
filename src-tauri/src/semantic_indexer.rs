//! Process-lifetime scheduling for resumable native semantic indexing.

use std::{
    env,
    fs::File,
    io::Read,
    path::PathBuf,
    sync::mpsc::{Receiver, Sender, channel},
    thread,
    time::Duration,
};

use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use hf_hub::{
    Cache, Repo, RepoType,
    api::sync::{ApiBuilder, ApiRepo},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{
    diagnostics::{Diagnostics, record_diagnostic},
    storage::{
        ConversationStore, DEFAULT_SEMANTIC_BATCH_SIZE, SemanticEmbedder, SemanticIndexState,
    },
};

const WORKER_THREAD_NAME: &str = "bottie-semantic-indexing";
const EMBEDDING_RUNTIME_THREADS: usize = 2;
/// Maximum time a foreground memory lookup waits behind semantic indexing or model preparation.
const QUERY_EMBEDDING_WAIT: Duration = Duration::from_secs(5);
const RUNTIME_ASSET_MANIFEST: &str = include_str!("../../runtime-assets.json");

/// Compiled release contract for the only runtime-downloaded model.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeAssetManifest {
    schema_version: u8,
    embedding_gemma: EmbeddingGemmaContract,
}

/// Immutable repository revision, variant, terms, and file identities for EmbeddingGemma.
#[derive(Deserialize)]
struct EmbeddingGemmaContract {
    repository: String,
    revision: String,
    variant: String,
    files: Vec<ModelFileContract>,
}

/// One exact model-cache file expected by the built-in FastEmbed adapter.
#[derive(Deserialize)]
struct ModelFileContract {
    path: String,
    sha256: String,
    size: u64,
}

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
        prepare_pinned_model_snapshot(&cache_dir)?;
        let options = TextInitOptions::new(EmbeddingModel::EmbeddingGemma300MQ4)
            .with_cache_dir(cache_dir)
            .with_show_download_progress(false)
            .with_intra_threads(EMBEDDING_RUNTIME_THREADS);
        TextEmbedding::try_new(options)
            .map(|model| Self { model })
            .map_err(|_| "model_runtime".into())
    }
}

/// Downloads only the compiled revision, verifies every file, then points FastEmbed's main cache ref at it.
fn prepare_pinned_model_snapshot(default_cache_dir: &PathBuf) -> Result<(), String> {
    let contract: RuntimeAssetManifest =
        serde_json::from_str(RUNTIME_ASSET_MANIFEST).map_err(|_| "model_contract")?;
    if contract.schema_version != 1 || contract.embedding_gemma.variant != "EmbeddingGemma300MQ4" {
        return Err("model_contract".into());
    }
    let cache_dir = env::var_os("HF_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_cache_dir.clone());
    let endpoint = env::var("HF_ENDPOINT").unwrap_or_else(|_| "https://huggingface.co".to_owned());
    let api = ApiBuilder::new()
        .with_cache_dir(cache_dir.clone())
        .with_endpoint(endpoint)
        .with_progress(false)
        .build()
        .map_err(|_| "model_cache")?;
    let revision = contract.embedding_gemma.revision.clone();
    let repository = contract.embedding_gemma.repository.clone();
    let pinned = api.repo(Repo::with_revision(
        repository.clone(),
        RepoType::Model,
        revision.clone(),
    ));
    verify_pinned_files(&pinned, &contract.embedding_gemma.files)?;
    Cache::new(cache_dir)
        .repo(Repo::new(repository, RepoType::Model))
        .create_ref(&revision)
        .map_err(|_| "model_cache".to_owned())
}

/// Fetches and verifies the exact model files without retaining remote bodies or cache paths in errors.
fn verify_pinned_files(repo: &ApiRepo, files: &[ModelFileContract]) -> Result<(), String> {
    for expected in files {
        let path = repo.get(&expected.path).map_err(|_| "model_download")?;
        let file = File::open(path).map_err(|_| "model_cache")?;
        verify_model_reader(file, expected.size, &expected.sha256)?;
    }
    Ok(())
}

/// Verifies one bounded expected size and SHA-256 while streaming cache bytes.
fn verify_model_reader(
    mut reader: impl Read,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<(), String> {
    let mut hasher = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer).map_err(|_| "model_cache")?;
        if count == 0 {
            break;
        }
        size = size.checked_add(count as u64).ok_or("model_contract")?;
        hasher.update(&buffer[..count]);
    }
    if size != expected_size || format!("{:x}", hasher.finalize()) != expected_sha256 {
        return Err("model_integrity".into());
    }
    Ok(())
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

    #[test]
    fn compiled_model_contract_is_complete_and_reader_verification_fails_closed() {
        let manifest: RuntimeAssetManifest = serde_json::from_str(RUNTIME_ASSET_MANIFEST)
            .expect("compiled release contract should parse");
        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.embedding_gemma.files.len(), 6);
        assert!(
            manifest
                .embedding_gemma
                .revision
                .chars()
                .all(|value| value.is_ascii_hexdigit())
        );

        let digest = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(verify_model_reader(&b"abc"[..], 3, digest), Ok(()));
        assert_eq!(
            verify_model_reader(&b"abc"[..], 4, digest),
            Err("model_integrity".into())
        );
        assert_eq!(
            verify_model_reader(&b"abd"[..], 3, digest),
            Err("model_integrity".into())
        );
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
