//! Anthropic Messages execution through Bottie's shared durable native-tool loop.

use super::*;

/// Runs repeated Anthropic Messages tool rounds through shared durable loop policy.
pub(crate) async fn stream_anthropic_tools(
    provider: AnthropicProvider,
    request: ChatRequest,
    sink: impl StreamSink + Clone + Send + Sync + 'static,
    store: ConversationStore,
    provider_run_id: String,
    query_embedder: SemanticQueryEmbedder,
    cancellation: ToolLoopCancellation,
    web_search: Option<Arc<dyn NativeWebSearchExecutor>>,
) -> Result<Option<Usage>, ProviderError> {
    let memory_enabled = request.memory_enabled;
    let mut session = AnthropicToolSession::new(request)?;
    let mut loop_state: Option<ToolLoopState> = None;
    let mut cumulative_usage = None;
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
        let calls = round.tool_calls.clone();
        let round_store = store.clone();
        let round_run_id = provider_run_id.clone();
        let mut round_embedder = query_embedder.clone();
        let round_cancellation = cancellation.clone();
        let round_web_search = web_search.clone();
        let (returned_state, results) = tauri::async_runtime::spawn_blocking(move || {
            let results = execute_anthropic_tool_round(
                &round_store,
                &round_run_id,
                &mut round_embedder,
                &mut state,
                calls,
                &round_cancellation,
                memory_enabled,
                round_web_search.as_ref().map(Arc::as_ref),
            );
            (state, results)
        })
        .await
        .map_err(|_| {
            ProviderError::internal("The native tool worker stopped unexpectedly.", None)
        })?;
        loop_state = Some(returned_state);
        session.append_results(round, results?)?;
    }
}

/// Executes one Anthropic-emitted call batch while preserving provider identities exactly.
pub(crate) fn execute_anthropic_tool_round(
    store: &ConversationStore,
    provider_run_id: &str,
    embedder: &mut impl SemanticEmbedder,
    state: &mut ToolLoopState,
    calls: Vec<AnthropicToolCall>,
    cancellation: &ToolLoopCancellation,
    memory_enabled: bool,
    web_search: Option<&dyn NativeWebSearchExecutor>,
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
                    web_search,
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
