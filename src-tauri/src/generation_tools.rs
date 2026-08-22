//! Durable mapped-provider memory-tool execution inside Bottie's provider-neutral loop policy.

use crate::{
    inference::{
        AnthropicProvider, AnthropicToolCall, AnthropicToolResult, AnthropicToolSession,
        ChatRequest, OllamaProvider, OllamaToolCall, OllamaToolResult, OllamaToolSession,
        OpenAiProvider, OpenAiToolCall, OpenAiToolResult, OpenAiToolSession, ProviderError,
        ProviderErrorCode, StreamSink, Usage,
    },
    provider_registry::RoutedProvider,
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

/// Confirms explicit intent plus a mapped provider's discovered per-model tool capability.
pub(crate) fn memory_tools_enabled(
    memory_enabled: bool,
    provider_id: &str,
    model_supports_tools: bool,
) -> bool {
    memory_enabled
        && matches!(provider_id, "ollama" | "openai" | "anthropic")
        && model_supports_tools
}

/// Routes one explicitly enabled generation into its provider-native memory-tool loop.
pub(crate) async fn stream_memory_tools(
    provider: RoutedProvider,
    request: ChatRequest,
    sink: impl StreamSink + Clone + Send + Sync + 'static,
    store: ConversationStore,
    provider_run_id: String,
    query_embedder: SemanticQueryEmbedder,
    cancellation: ToolLoopCancellation,
) -> Result<Option<Usage>, ProviderError> {
    match provider {
        RoutedProvider::Ollama(provider) => {
            stream_ollama_memory_tools(
                provider,
                request,
                sink,
                store,
                provider_run_id,
                query_embedder,
                cancellation,
            )
            .await
        }
        RoutedProvider::OpenAi(provider) => {
            stream_openai_memory_tools(
                provider,
                request,
                sink,
                store,
                provider_run_id,
                query_embedder,
                cancellation,
            )
            .await
        }
        RoutedProvider::Anthropic(provider) => {
            stream_anthropic_memory_tools(
                provider,
                request,
                sink,
                store,
                provider_run_id,
                query_embedder,
                cancellation,
            )
            .await
        }
        RoutedProvider::Omlx(_) => Err(ProviderError::internal(
            "The selected provider does not map Bottie's native memory tools.",
            None,
        )),
    }
}

/// Runs repeated Anthropic Messages tool rounds through shared durable loop policy.
pub(crate) async fn stream_anthropic_memory_tools(
    provider: AnthropicProvider,
    request: ChatRequest,
    sink: impl StreamSink + Clone + Send + Sync + 'static,
    store: ConversationStore,
    provider_run_id: String,
    query_embedder: SemanticQueryEmbedder,
    cancellation: ToolLoopCancellation,
) -> Result<Option<Usage>, ProviderError> {
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
        let (returned_state, results) = tauri::async_runtime::spawn_blocking(move || {
            let results = execute_anthropic_memory_round(
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

/// Runs repeated OpenAI Chat Completions tool rounds through shared durable loop policy.
pub(crate) async fn stream_openai_memory_tools(
    provider: OpenAiProvider,
    request: ChatRequest,
    sink: impl StreamSink + Clone + Send + Sync + 'static,
    store: ConversationStore,
    provider_run_id: String,
    query_embedder: SemanticQueryEmbedder,
    cancellation: ToolLoopCancellation,
) -> Result<Option<Usage>, ProviderError> {
    let mut session = OpenAiToolSession::new(request)?;
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
            let results = execute_openai_memory_round(
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

/// Adds provider-reported usage and cost across requests in one logical generation.
fn merge_usage(current: Option<Usage>, next: Option<Usage>) -> Option<Usage> {
    match (current, next) {
        (None, None) => None,
        (current, next) => {
            let current = current.unwrap_or_default();
            let next = next.unwrap_or_default();
            Some(Usage {
                input_tokens: merge_count(current.input_tokens, next.input_tokens),
                output_tokens: merge_count(current.output_tokens, next.output_tokens),
                cost_usd: merge_cost(current.cost_usd, next.cost_usd),
            })
        }
    }
}

/// Adds optional provider costs without inventing a value when both rounds omitted it.
fn merge_cost(current: Option<f64>, next: Option<f64>) -> Option<f64> {
    match (current, next) {
        (None, None) => None,
        (current, next) => Some(current.unwrap_or_default() + next.unwrap_or_default()),
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

/// Executes one OpenAI-emitted call batch while preserving provider identities exactly.
pub(crate) fn execute_openai_memory_round(
    store: &ConversationStore,
    provider_run_id: &str,
    embedder: &mut impl SemanticEmbedder,
    state: &mut ToolLoopState,
    calls: Vec<OpenAiToolCall>,
    cancellation: &ToolLoopCancellation,
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
        .execute_round_try_with(
            native_calls,
            cancellation,
            std::time::Instant::now,
            |call| execute_and_checkpoint(store, provider_run_id, embedder, call),
        )
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

/// Executes one Anthropic-emitted call batch while preserving provider identities exactly.
pub(crate) fn execute_anthropic_memory_round(
    store: &ConversationStore,
    provider_run_id: &str,
    embedder: &mut impl SemanticEmbedder,
    state: &mut ToolLoopState,
    calls: Vec<AnthropicToolCall>,
    cancellation: &ToolLoopCancellation,
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
            |call| execute_and_checkpoint(store, provider_run_id, embedder, call),
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
