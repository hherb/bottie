//! Anthropic Messages execution through Bottie's shared durable native-tool loop.

use super::*;
use crate::tool_loop::AsyncToolExecutor;

/// Runs repeated Anthropic Messages tool rounds through shared durable loop policy.
#[allow(
    clippy::too_many_arguments,
    reason = "the mapped provider keeps each native executor and trust boundary explicit"
)]
pub(crate) async fn stream_anthropic_tools(
    provider: AnthropicProvider,
    request: ChatRequest,
    sink: impl StreamSink + Clone + Send + Sync + 'static,
    store: ConversationStore,
    provider_run_id: String,
    query_embedder: SemanticQueryEmbedder,
    cancellation: ToolLoopCancellation,
    web_search: Option<Arc<dyn NativeWebSearchExecutor>>,
    web_fetch: Option<Arc<dyn NativeWebFetchExecutor>>,
    localmail: Option<Arc<dyn NativeLocalmailToolExecutor>>,
    python_approval: Arc<crate::python_approval::PythonApprovalController>,
    python_runner: Option<Arc<dyn crate::python_execution::PythonRunner>>,
) -> Result<Option<Usage>, ProviderError> {
    let memory_enabled = request.memory_enabled;
    let mut session = AnthropicToolSession::new(request, python_runner.is_some())?;
    let mut loop_state: Option<ToolLoopState> = None;
    let mut cumulative_usage = None;
    let mut executor = MappedNativeToolExecutor::new(
        store,
        provider_run_id,
        query_embedder,
        cancellation.clone(),
        memory_enabled,
        web_search,
        web_fetch,
        localmail,
        python_approval,
        python_runner,
    );
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
        let results = execute_anthropic_tool_round_async(
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

/// Executes one Anthropic call batch while retaining exact provider correlation and loop budgets.
pub(crate) async fn execute_anthropic_tool_round_async<
    E: AsyncToolExecutor<Error = StorageError>,
>(
    state: &mut ToolLoopState,
    calls: Vec<AnthropicToolCall>,
    cancellation: &ToolLoopCancellation,
    executor: &mut E,
) -> Result<Vec<AnthropicToolResult>, ProviderError> {
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
            let is_error = matches!(result.execution, MemoryToolExecution::Error { .. });
            let content = serde_json::to_string(&result.execution).map_err(|_| {
                ProviderError::internal("The native tool result could not be serialized.", None)
            })?;
            Ok(AnthropicToolResult {
                tool_use_id: result.call_id,
                content,
                is_error,
            })
        })
        .collect()
}

/// Executes one Anthropic-emitted call batch while preserving provider identities exactly.
#[cfg(test)]
pub(crate) fn execute_anthropic_tool_round(
    store: &ConversationStore,
    provider_run_id: &str,
    embedder: &mut impl SemanticEmbedder,
    state: &mut ToolLoopState,
    calls: Vec<AnthropicToolCall>,
    cancellation: &ToolLoopCancellation,
    memory_enabled: bool,
    web_search: Option<&dyn NativeWebSearchExecutor>,
    web_fetch: Option<&dyn NativeWebFetchExecutor>,
    localmail: Option<&dyn NativeLocalmailToolExecutor>,
) -> Result<Vec<AnthropicToolResult>, ProviderError> {
    let native_calls = calls
        .into_iter()
        .map(|call| NativeToolCall {
            call_id: call.call_id().to_owned(),
            tool_name: call.tool_name().to_owned(),
            arguments: call.arguments().clone(),
        })
        .collect();
    state
        .execute_round_try_with(
            native_calls,
            cancellation,
            std::time::Instant::now,
            |call| {
                execute_and_checkpoint(
                    store,
                    provider_run_id,
                    embedder,
                    call,
                    memory_enabled,
                    localmail,
                    web_search,
                    web_fetch,
                )
            },
        )
        .map_err(round_error)?
        .into_iter()
        .map(|result| {
            let is_error = matches!(result.execution, MemoryToolExecution::Error { .. });
            let content = serde_json::to_string(&result.execution).map_err(|_| {
                ProviderError::internal("The native tool result could not be serialized.", None)
            })?;
            Ok(AnthropicToolResult {
                tool_use_id: result.call_id,
                content,
                is_error,
            })
        })
        .collect()
}
