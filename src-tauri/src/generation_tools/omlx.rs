//! oMLX-only asynchronous Python mapping inside the shared durable tool loop.

use std::{future::Future, pin::Pin, sync::Arc};

use crate::{
    generation_localmail_tools::NativeLocalmailToolExecutor,
    generation_usage::merge_usage,
    generation_web_tools::{NativeWebFetchExecutor, NativeWebSearchExecutor},
    inference::{
        ChatRequest, OmlxProvider, OmlxToolSession, OpenAiToolCall, OpenAiToolResult,
        ProviderError, StreamSink, Usage,
    },
    python_approval::PythonApprovalController,
    python_audit::execute_audited_python_for_provider,
    python_execution::PythonRunner,
    semantic_indexer::SemanticQueryEmbedder,
    storage::{ConversationStore, StorageError},
    tool_contract::RUN_PYTHON_TOOL_NAME,
    tool_dispatch::MemoryToolExecution,
    tool_loop::{AsyncToolExecutor, NativeToolCall, ToolLoopCancellation, ToolLoopState},
};

use super::{execute_and_checkpoint, round_error, tool_loop_error};

/// Runs repeated oMLX Chat Completions rounds through Bottie's existing durable loop policy.
#[allow(
    clippy::too_many_arguments,
    reason = "the mapped provider keeps each native executor and trust boundary explicit"
)]
pub(crate) async fn stream_omlx_tools(
    provider: OmlxProvider,
    request: ChatRequest,
    sink: impl StreamSink + Clone + Send + Sync + 'static,
    store: ConversationStore,
    provider_run_id: String,
    query_embedder: SemanticQueryEmbedder,
    cancellation: ToolLoopCancellation,
    web_search: Option<Arc<dyn NativeWebSearchExecutor>>,
    web_fetch: Option<Arc<dyn NativeWebFetchExecutor>>,
    localmail: Option<Arc<dyn NativeLocalmailToolExecutor>>,
    python_approval: Arc<PythonApprovalController>,
    python_runner: Option<Arc<dyn PythonRunner>>,
) -> Result<Option<Usage>, ProviderError> {
    let memory_enabled = request.memory_enabled;
    let mut session = OmlxToolSession::new(request, python_runner.is_some())?;
    let mut loop_state: Option<ToolLoopState> = None;
    let mut cumulative_usage = None;
    let mut executor = OmlxNativeToolExecutor {
        store,
        provider_run_id,
        query_embedder: Some(query_embedder),
        cancellation: cancellation.clone(),
        memory_enabled,
        web_search,
        web_fetch,
        localmail,
        python_approval,
        python_runner,
    };
    loop {
        let round = provider.stream_tool_round(&session, sink.clone()).await?;
        cumulative_usage = merge_usage(cumulative_usage, round.usage.clone());
        if let Some(usage) = &cumulative_usage {
            sink.usage_updated(usage.clone())?;
        }
        if round.tool_calls.is_empty() {
            if let Some(state) = &mut loop_state {
                state.complete(&cancellation).map_err(tool_loop_error)?;
            }
            return Ok(cumulative_usage);
        }

        let mut state = loop_state.unwrap_or_else(|| ToolLoopState::new(std::time::Instant::now()));
        let results = execute_omlx_tool_round(
            &mut state,
            round.tool_calls.clone(),
            &cancellation,
            &mut executor,
        )
        .await?;
        loop_state = Some(state);
        session.append_results(round, results)?;
    }
}

/// Executes one oMLX call batch while retaining exact provider correlation and loop budgets.
pub(crate) async fn execute_omlx_tool_round<E: AsyncToolExecutor<Error = StorageError>>(
    state: &mut ToolLoopState,
    calls: Vec<OpenAiToolCall>,
    cancellation: &ToolLoopCancellation,
    executor: &mut E,
) -> Result<Vec<OpenAiToolResult>, ProviderError> {
    let native_calls = calls
        .into_iter()
        .map(|call| NativeToolCall {
            call_id: call.call_id().to_owned(),
            tool_name: call.tool_name().to_owned(),
            arguments: call.arguments().clone(),
        })
        .collect();
    state
        .execute_round_try_with_async(
            native_calls,
            cancellation,
            std::time::Instant::now,
            executor,
        )
        .await
        .map_err(round_error)?
        .into_iter()
        .map(|result| {
            let content = serde_json::to_string(&result.execution).map_err(|_| {
                ProviderError::internal("The native tool result could not be serialized.", None)
            })?;
            Ok(OpenAiToolResult {
                tool_call_id: result.call_id,
                content,
            })
        })
        .collect()
}

/// Production oMLX executor that keeps ordinary native work off the async runtime thread.
struct OmlxNativeToolExecutor {
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

impl AsyncToolExecutor for OmlxNativeToolExecutor {
    type Error = StorageError;

    /// Routes available Python through approval while preserving existing blocking tool dispatch.
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
