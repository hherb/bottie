//! Durable Ollama memory-tool execution inside Bottie's provider-neutral loop policy.

use crate::{
    inference::{
        ChatRequest, OllamaProvider, OllamaToolCall, OllamaToolResult, OllamaToolSession,
        ProviderError, ProviderErrorCode, StreamSink, Usage,
    },
    semantic_indexer::SemanticQueryEmbedder,
    storage::{
        ConversationStore, NewToolInvocation, NewToolResult, SemanticEmbedder, StorageError,
    },
    tool_dispatch::{MemoryToolExecution, dispatch_memory_tool},
    tool_loop::{
        NativeToolCall, ToolLoopCancellation, ToolLoopError, ToolLoopErrorCode, ToolLoopState,
        ToolRoundError,
    },
};

/// Runs repeated Ollama chat/tool rounds through one durable provider run and shared policy state.
pub(crate) async fn stream_ollama_memory_tools(
    provider: OllamaProvider,
    request: ChatRequest,
    sink: impl StreamSink + Clone + Send + Sync + 'static,
    store: ConversationStore,
    provider_run_id: String,
    query_embedder: SemanticQueryEmbedder,
    cancellation: ToolLoopCancellation,
) -> Result<Option<Usage>, ProviderError> {
    let mut session = OllamaToolSession::new(request)?;
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
        let (returned_state, results) = tauri::async_runtime::spawn_blocking(move || {
            let results = execute_ollama_memory_round(
                &round_store,
                &round_run_id,
                &mut round_embedder,
                &mut state,
                calls,
                &round_cancellation,
            );
            (state, results)
        })
        .await
        .map_err(|_| {
            ProviderError::internal("The native memory-tool worker stopped unexpectedly.", None)
        })?;
        loop_state = Some(returned_state);
        session.append_results(round, results?)?;
    }
}

/// Adds provider-reported usage across Ollama requests in one logical generation.
fn merge_usage(current: Option<Usage>, next: Option<Usage>) -> Option<Usage> {
    match (current, next) {
        (None, None) => None,
        (current, next) => {
            let current = current.unwrap_or_default();
            let next = next.unwrap_or_default();
            Some(Usage {
                input_tokens: merge_count(current.input_tokens, next.input_tokens),
                output_tokens: merge_count(current.output_tokens, next.output_tokens),
                cost_usd: None,
            })
        }
    }
}

/// Saturating-adds optional provider counts without inventing a value both rounds omitted.
fn merge_count(current: Option<u64>, next: Option<u64>) -> Option<u64> {
    match (current, next) {
        (None, None) => None,
        (current, next) => Some(
            current
                .unwrap_or_default()
                .saturating_add(next.unwrap_or_default()),
        ),
    }
}

/// Executes one Ollama-emitted call batch with durable call/result checkpoints before provider reuse.
pub(crate) fn execute_ollama_memory_round(
    store: &ConversationStore,
    provider_run_id: &str,
    embedder: &mut impl SemanticEmbedder,
    state: &mut ToolLoopState,
    calls: Vec<OllamaToolCall>,
    cancellation: &ToolLoopCancellation,
) -> Result<Vec<OllamaToolResult>, ProviderError> {
    let tool_names = calls
        .iter()
        .map(|call| call.tool_name().to_owned())
        .collect::<Vec<_>>();
    let native_calls = calls
        .into_iter()
        .map(|call| NativeToolCall {
            call_id: uuid::Uuid::new_v4().to_string(),
            tool_name: call.tool_name().to_owned(),
            arguments: call.arguments().clone(),
        })
        .collect();
    let results = state
        .execute_round_try_with(
            native_calls,
            cancellation,
            std::time::Instant::now,
            |call| execute_and_checkpoint(store, provider_run_id, embedder, call),
        )
        .map_err(round_error)?;

    tool_names
        .into_iter()
        .zip(results)
        .map(|(tool_name, result)| {
            let content = serde_json::to_string(&result.execution).map_err(|_| {
                ProviderError::internal("The native tool result could not be serialized.", None)
            })?;
            Ok(OllamaToolResult { tool_name, content })
        })
        .collect()
}

/// Checkpoints one accepted call, executes it, then checkpoints the exact provider-facing envelope.
fn execute_and_checkpoint(
    store: &ConversationStore,
    provider_run_id: &str,
    embedder: &mut impl SemanticEmbedder,
    call: &NativeToolCall,
) -> Result<MemoryToolExecution, StorageError> {
    store.checkpoint_tool_invocation(NewToolInvocation {
        provider_run_id: provider_run_id.into(),
        provider_call_id: call.call_id.clone(),
        tool_name: call.tool_name.clone(),
        arguments: call.arguments.clone(),
    })?;
    let execution = dispatch_memory_tool(store, embedder, &call.tool_name, &call.arguments);
    let output = serde_json::to_value(&execution).map_err(|_| StorageError::internal())?;
    store.checkpoint_tool_result(NewToolResult {
        provider_run_id: provider_run_id.into(),
        provider_call_id: call.call_id.clone(),
        output,
        is_error: matches!(execution, MemoryToolExecution::Error { .. }),
    })?;
    Ok(execution)
}

/// Converts loop policy or secret-free checkpoint failures into the existing generation surface.
fn round_error(error: ToolRoundError<StorageError>) -> ProviderError {
    match error {
        ToolRoundError::Policy(error) => tool_loop_error(error),
        ToolRoundError::Execution(error) => storage_error(error),
    }
}

/// Maps fixed tool-loop policy without reflecting provider-controlled call data.
fn tool_loop_error(error: ToolLoopError) -> ProviderError {
    match error.code {
        ToolLoopErrorCode::TimedOut => ProviderError {
            code: ProviderErrorCode::Timeout,
            message: error.message.into(),
            retryable: true,
            diagnostic: Some("native tool-loop deadline".into()),
        },
        ToolLoopErrorCode::Cancelled => ProviderError {
            code: ProviderErrorCode::Internal,
            message: error.message.into(),
            retryable: false,
            diagnostic: Some("native tool-loop cancellation".into()),
        },
        ToolLoopErrorCode::CallLimitExceeded
        | ToolLoopErrorCode::RecursionLimitExceeded
        | ToolLoopErrorCode::AggregateOutputExceeded
        | ToolLoopErrorCode::InvalidState => {
            ProviderError::malformed(error.message, Some("native tool-loop policy".into()))
        }
    }
}

/// Maps one path-free native checkpoint failure without forwarding storage diagnostics.
fn storage_error(error: StorageError) -> ProviderError {
    match error.code {
        "invalid_request" | "not_found" => ProviderError::internal(
            "Bottie could not retain the native tool activity safely.",
            None,
        ),
        _ => ProviderError::internal("Bottie could not execute the native memory tool.", None),
    }
}
