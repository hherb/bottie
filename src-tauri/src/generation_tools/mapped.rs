//! Shared async native executor for local providers that can pause for Python approval.

use std::{future::Future, pin::Pin, sync::Arc};

use crate::{
    generation_localmail_tools::NativeLocalmailToolExecutor,
    generation_web_tools::{NativeWebFetchExecutor, NativeWebSearchExecutor},
    python_approval::PythonApprovalController,
    python_audit::execute_audited_python_for_provider,
    python_execution::PythonRunner,
    semantic_indexer::SemanticQueryEmbedder,
    storage::{ConversationStore, StorageError},
    tool_contract::RUN_PYTHON_TOOL_NAME,
    tool_dispatch::MemoryToolExecution,
    tool_loop::{AsyncToolExecutor, NativeToolCall, ToolLoopCancellation},
};

use super::execute_and_checkpoint;

/// Provider-neutral executor that routes Python asynchronously and ordinary tools off-thread.
pub(super) struct MappedNativeToolExecutor {
    store: ConversationStore,
    provider_run_id: String,
    query_embedder: Option<SemanticQueryEmbedder>,
    cancellation: ToolLoopCancellation,
    memory_enabled: bool,
    web_search: Option<Arc<dyn NativeWebSearchExecutor>>,
    web_fetch: Option<Arc<dyn NativeWebFetchExecutor>>,
    localmail: Option<Arc<dyn NativeLocalmailToolExecutor>>,
    python_approval: Arc<PythonApprovalController>,
    python_runner: Option<Arc<dyn PythonRunner>>,
}

impl MappedNativeToolExecutor {
    /// Retains every native boundary required by one mapped provider generation.
    #[allow(
        clippy::too_many_arguments,
        reason = "native executors remain explicit at the trust boundary"
    )]
    pub(super) fn new(
        store: ConversationStore,
        provider_run_id: String,
        query_embedder: SemanticQueryEmbedder,
        cancellation: ToolLoopCancellation,
        memory_enabled: bool,
        web_search: Option<Arc<dyn NativeWebSearchExecutor>>,
        web_fetch: Option<Arc<dyn NativeWebFetchExecutor>>,
        localmail: Option<Arc<dyn NativeLocalmailToolExecutor>>,
        python_approval: Arc<PythonApprovalController>,
        python_runner: Option<Arc<dyn PythonRunner>>,
    ) -> Self {
        Self {
            store,
            provider_run_id,
            query_embedder: Some(query_embedder),
            cancellation,
            memory_enabled,
            web_search,
            web_fetch,
            localmail,
            python_approval,
            python_runner,
        }
    }
}

impl AsyncToolExecutor for MappedNativeToolExecutor {
    type Error = StorageError;

    /// Routes available Python through approval while keeping ordinary native work blocking.
    fn execute<'a>(
        &'a mut self,
        call: &'a NativeToolCall,
    ) -> Pin<Box<dyn Future<Output = Result<MemoryToolExecution, Self::Error>> + Send + 'a>> {
        Box::pin(async move {
            if call.tool_name == RUN_PYTHON_TOOL_NAME
                && let Some(runner) = &self.python_runner
            {
                return execute_audited_python_for_provider(
                    &self.store,
                    &self.provider_run_id,
                    &self.python_approval,
                    runner.as_ref(),
                    call.clone(),
                    &self.cancellation,
                )
                .await;
            }

            let store = self.store.clone();
            let provider_run_id = self.provider_run_id.clone();
            let mut embedder = self
                .query_embedder
                .take()
                .ok_or_else(StorageError::internal)?;
            let call = call.clone();
            let memory_enabled = self.memory_enabled;
            let web_search = self.web_search.clone();
            let web_fetch = self.web_fetch.clone();
            let localmail = self.localmail.clone();
            let (returned_embedder, result) = tauri::async_runtime::spawn_blocking(move || {
                let result = execute_and_checkpoint(
                    &store,
                    &provider_run_id,
                    &mut embedder,
                    &call,
                    memory_enabled,
                    localmail.as_ref().map(Arc::as_ref),
                    web_search.as_ref().map(Arc::as_ref),
                    web_fetch.as_ref().map(Arc::as_ref),
                );
                (embedder, result)
            })
            .await
            .map_err(|_| StorageError::internal())?;
            self.query_embedder = Some(returned_embedder);
            result
        })
    }
}
