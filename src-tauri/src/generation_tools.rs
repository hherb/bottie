//! Durable mapped-provider native-tool execution inside Bottie's provider-neutral loop policy.

use std::sync::Arc;

use crate::{
    generation_usage::merge_usage,
    generation_web_tools::{NativeWebFetchExecutor, NativeWebSearchExecutor, dispatch_native_tool},
    inference::{
        AnthropicProvider, AnthropicToolCall, AnthropicToolResult, AnthropicToolSession,
        ChatRequest, OllamaProvider, OllamaToolCall, OllamaToolResult, OllamaToolSession,
        OmlxProvider, OmlxToolSession, OpenAiProvider, OpenAiToolCall, OpenAiToolResult,
        OpenAiToolSession, ProviderError, ProviderErrorCode, StreamSink, Usage,
    },
    provider_registry::RoutedProvider,
    semantic_indexer::SemanticQueryEmbedder,
    storage::{
        ConversationStore, NewToolInvocation, NewToolResult, SemanticEmbedder, StorageError,
        ToolAuditOutcome, ToolAuditPolicy,
    },
    tool_dispatch::{MemoryToolExecution, MemoryToolExecutionErrorCode},
    tool_loop::{
        NativeToolCall, ToolLoopCancellation, ToolLoopError, ToolLoopErrorCode, ToolLoopState,
        ToolRoundError,
    },
    tool_policy::{ToolExecutionPolicy, tool_execution_policy},
};

mod anthropic;

#[cfg(test)]
pub(crate) use anthropic::execute_anthropic_tool_round;
pub(crate) use anthropic::stream_anthropic_tools;

/// Routes one explicitly enabled generation into its provider-native tool loop.
pub(crate) async fn stream_native_tools(
    provider: RoutedProvider,
    request: ChatRequest,
    sink: impl StreamSink + Clone + Send + Sync + 'static,
    store: ConversationStore,
    provider_run_id: String,
    query_embedder: SemanticQueryEmbedder,
    cancellation: ToolLoopCancellation,
    web_search: Option<Arc<dyn NativeWebSearchExecutor>>,
    web_fetch: Option<Arc<dyn NativeWebFetchExecutor>>,
) -> Result<Option<Usage>, ProviderError> {
    match provider {
        RoutedProvider::Ollama(provider) => {
            stream_ollama_tools(
                provider,
                request,
                sink,
                store,
                provider_run_id,
                query_embedder,
                cancellation,
                web_search,
                web_fetch,
            )
            .await
        }
        RoutedProvider::OpenAi(provider) => {
            stream_openai_tools(
                provider,
                request,
                sink,
                store,
                provider_run_id,
                query_embedder,
                cancellation,
                web_search,
                web_fetch,
            )
            .await
        }
        RoutedProvider::Anthropic(provider) => {
            stream_anthropic_tools(
                provider,
                request,
                sink,
                store,
                provider_run_id,
                query_embedder,
                cancellation,
                web_search,
                web_fetch,
            )
            .await
        }
        RoutedProvider::Omlx(provider) => {
            stream_omlx_tools(
                provider,
                request,
                sink,
                store,
                provider_run_id,
                query_embedder,
                cancellation,
                web_search,
                web_fetch,
            )
            .await
        }
    }
}

/// Runs repeated oMLX Chat Completions rounds through Bottie's existing durable loop policy.
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
) -> Result<Option<Usage>, ProviderError> {
    let memory_enabled = request.memory_enabled;
    let mut session = OmlxToolSession::new(request)?;
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
        let round_web_fetch = web_fetch.clone();
        let (returned_state, results) = tauri::async_runtime::spawn_blocking(move || {
            let results = execute_openai_tool_round(
                &round_store,
                &round_run_id,
                &mut round_embedder,
                &mut state,
                calls,
                &round_cancellation,
                memory_enabled,
                round_web_search.as_ref().map(Arc::as_ref),
                round_web_fetch.as_ref().map(Arc::as_ref),
            );
            (state, results)
        })
        .await
        .map_err(|_| {
            ProviderError::internal("The native oMLX tool worker stopped unexpectedly.", None)
        })?;
        loop_state = Some(returned_state);
        session.append_results(round, results?)?;
    }
}

/// Runs repeated OpenAI Chat Completions tool rounds through shared durable loop policy.
pub(crate) async fn stream_openai_tools(
    provider: OpenAiProvider,
    request: ChatRequest,
    sink: impl StreamSink + Clone + Send + Sync + 'static,
    store: ConversationStore,
    provider_run_id: String,
    query_embedder: SemanticQueryEmbedder,
    cancellation: ToolLoopCancellation,
    web_search: Option<Arc<dyn NativeWebSearchExecutor>>,
    web_fetch: Option<Arc<dyn NativeWebFetchExecutor>>,
) -> Result<Option<Usage>, ProviderError> {
    let memory_enabled = request.memory_enabled;
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
        let round_web_search = web_search.clone();
        let round_web_fetch = web_fetch.clone();
        let (returned_state, results) = tauri::async_runtime::spawn_blocking(move || {
            let results = execute_openai_tool_round(
                &round_store,
                &round_run_id,
                &mut round_embedder,
                &mut state,
                calls,
                &round_cancellation,
                memory_enabled,
                round_web_search.as_ref().map(Arc::as_ref),
                round_web_fetch.as_ref().map(Arc::as_ref),
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
pub(crate) async fn stream_ollama_tools(
    provider: OllamaProvider,
    request: ChatRequest,
    sink: impl StreamSink + Clone + Send + Sync + 'static,
    store: ConversationStore,
    provider_run_id: String,
    query_embedder: SemanticQueryEmbedder,
    cancellation: ToolLoopCancellation,
    web_search: Option<Arc<dyn NativeWebSearchExecutor>>,
    web_fetch: Option<Arc<dyn NativeWebFetchExecutor>>,
) -> Result<Option<Usage>, ProviderError> {
    let memory_enabled = request.memory_enabled;
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
        let round_web_search = web_search.clone();
        let round_web_fetch = web_fetch.clone();
        let (returned_state, results) = tauri::async_runtime::spawn_blocking(move || {
            let results = execute_ollama_tool_round(
                &round_store,
                &round_run_id,
                &mut round_embedder,
                &mut state,
                calls,
                &round_cancellation,
                memory_enabled,
                round_web_search.as_ref().map(Arc::as_ref),
                round_web_fetch.as_ref().map(Arc::as_ref),
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

/// Executes one Ollama-emitted call batch with durable call/result checkpoints before provider reuse.
pub(crate) fn execute_ollama_tool_round(
    store: &ConversationStore,
    provider_run_id: &str,
    embedder: &mut impl SemanticEmbedder,
    state: &mut ToolLoopState,
    calls: Vec<OllamaToolCall>,
    cancellation: &ToolLoopCancellation,
    memory_enabled: bool,
    web_search: Option<&dyn NativeWebSearchExecutor>,
    web_fetch: Option<&dyn NativeWebFetchExecutor>,
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
            |call| {
                execute_and_checkpoint(
                    store,
                    provider_run_id,
                    embedder,
                    call,
                    memory_enabled,
                    web_search,
                    web_fetch,
                )
            },
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
pub(crate) fn execute_openai_tool_round(
    store: &ConversationStore,
    provider_run_id: &str,
    embedder: &mut impl SemanticEmbedder,
    state: &mut ToolLoopState,
    calls: Vec<OpenAiToolCall>,
    cancellation: &ToolLoopCancellation,
    memory_enabled: bool,
    web_search: Option<&dyn NativeWebSearchExecutor>,
    web_fetch: Option<&dyn NativeWebFetchExecutor>,
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
            |call| {
                execute_and_checkpoint(
                    store,
                    provider_run_id,
                    embedder,
                    call,
                    memory_enabled,
                    web_search,
                    web_fetch,
                )
            },
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

/// Checkpoints one accepted call, executes it, then checkpoints the exact provider-facing envelope.
fn execute_and_checkpoint(
    store: &ConversationStore,
    provider_run_id: &str,
    embedder: &mut impl SemanticEmbedder,
    call: &NativeToolCall,
    memory_enabled: bool,
    web_search: Option<&dyn NativeWebSearchExecutor>,
    web_fetch: Option<&dyn NativeWebFetchExecutor>,
) -> Result<MemoryToolExecution, StorageError> {
    let audit_policy = match tool_execution_policy(&call.tool_name) {
        Some(ToolExecutionPolicy::Safe) => ToolAuditPolicy::Safe,
        Some(ToolExecutionPolicy::ApprovalRequired) => ToolAuditPolicy::ApprovalRequired,
        None => ToolAuditPolicy::Unregistered,
    };
    store.checkpoint_tool_invocation(NewToolInvocation {
        provider_run_id: provider_run_id.into(),
        provider_call_id: call.call_id.clone(),
        tool_name: call.tool_name.clone(),
        arguments: call.arguments.clone(),
        audit_policy,
    })?;
    let started = std::time::Instant::now();
    let execution =
        dispatch_native_tool(store, embedder, call, memory_enabled, web_search, web_fetch);
    let audit_outcome = audit_outcome(&execution);
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let output = serde_json::to_value(&execution).map_err(|_| StorageError::internal())?;
    store.checkpoint_tool_result(NewToolResult {
        provider_run_id: provider_run_id.into(),
        provider_call_id: call.call_id.clone(),
        output,
        is_error: matches!(execution, MemoryToolExecution::Error { .. }),
        audit_outcome,
        duration_ms,
    })?;
    Ok(execution)
}

/// Maps the bounded dispatcher envelope into its durable path-free audit category.
fn audit_outcome(execution: &MemoryToolExecution) -> ToolAuditOutcome {
    let MemoryToolExecution::Error { error } = execution else {
        return ToolAuditOutcome::Success;
    };
    match error.code {
        MemoryToolExecutionErrorCode::UnsupportedTool => ToolAuditOutcome::UnsupportedTool,
        MemoryToolExecutionErrorCode::InvalidArguments => ToolAuditOutcome::InvalidArguments,
        MemoryToolExecutionErrorCode::ApprovalRequired => ToolAuditOutcome::ApprovalRequired,
        MemoryToolExecutionErrorCode::Unavailable => ToolAuditOutcome::Unavailable,
        MemoryToolExecutionErrorCode::ExecutionFailed => ToolAuditOutcome::ExecutionFailed,
        MemoryToolExecutionErrorCode::OutputTooLarge => ToolAuditOutcome::OutputTooLarge,
    }
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
        _ => ProviderError::internal("Bottie could not execute the native tool.", None),
    }
}
