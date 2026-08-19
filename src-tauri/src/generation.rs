//! Native provider-run lifecycle, cancellation, and durable provenance orchestration.

use futures_util::future::{AbortHandle, Abortable};
use tauri::{State, ipc::Channel};

use crate::{
    AppState,
    diagnostics::{record_diagnostic, sanitized},
    inference::{ChatRequest, ChatRun, ProviderError, ReasoningEffort, StreamEvent, Usage},
    provider_registry::routed_provider,
    storage::{
        ConversationStore, NewProviderRun, ProviderRunContext, ProviderRunState,
        StoredReasoningEffort, StoredUsage,
    },
    stream_channel::ChannelSink,
};

#[tauri::command]
/// Starts one cancellable provider-qualified chat generation with durable provenance.
pub(crate) async fn start_chat(
    state: State<'_, AppState>,
    request: ChatRequest,
    context: ProviderRunContext,
    on_event: Channel<StreamEvent>,
) -> Result<ChatRun, ProviderError> {
    let providers = state.providers.read().await.clone();
    let run_id = uuid::Uuid::new_v4().to_string();
    state
        .conversations
        .start_provider_run(NewProviderRun {
            id: run_id.clone(),
            conversation_id: context.conversation_id,
            request_message_id: context.request_message_id,
            provider_id: request.provider_id.clone(),
            model_id: request.model_id.clone(),
            reasoning_effort: stored_reasoning_effort(request.settings.reasoning_effort),
            temperature: request.settings.temperature,
            max_output_tokens: request.settings.max_output_tokens,
        })
        .map_err(provider_run_storage_error)?;
    let provider =
        match routed_provider(&request.provider_id, &providers, state.credentials.as_ref()) {
            Ok(provider) => provider,
            Err(error) => {
                let error = sanitized(error);
                finish_provider_run(
                    &state.conversations,
                    &run_id,
                    ProviderRunState::Failed,
                    Some(error.code.as_str()),
                    None,
                )?;
                return Err(error);
            }
        };
    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    state.runs.lock().await.insert(run_id.clone(), abort_handle);

    let runs = state.runs.clone();
    let diagnostics = state.diagnostics.clone();
    let conversations = state.conversations.clone();
    let task_run_id = run_id.clone();
    tauri::async_runtime::spawn(async move {
        record_diagnostic(
            &diagnostics,
            "info",
            "Generation started",
            Some(provider.provider_id()),
            Some(&format!("run {task_run_id}")),
        )
        .await;
        let _ = on_event.send(StreamEvent::Started {
            run_id: task_run_id.clone(),
            provider_id: provider.provider_id().into(),
            model_id: request.model_id.clone(),
        });
        let sink = ChannelSink {
            run_id: task_run_id.clone(),
            channel: on_event.clone(),
            conversations: conversations.clone(),
        };
        match Abortable::new(provider.stream_chat(request, sink), abort_registration).await {
            Ok(Ok(usage)) => {
                let event = match finish_provider_run(
                    &conversations,
                    &task_run_id,
                    ProviderRunState::Completed,
                    None,
                    usage.clone(),
                ) {
                    Ok(()) => StreamEvent::Completed {
                        run_id: task_run_id.clone(),
                        usage,
                    },
                    Err(error) => StreamEvent::Failed {
                        run_id: task_run_id.clone(),
                        error,
                    },
                };
                let _ = on_event.send(event);
                record_diagnostic(
                    &diagnostics,
                    "info",
                    "Generation completed",
                    Some(provider.provider_id()),
                    Some(&format!("run {task_run_id}")),
                )
                .await;
            }
            Ok(Err(error)) => {
                let error = sanitized(error);
                let event_error = finish_provider_run(
                    &conversations,
                    &task_run_id,
                    ProviderRunState::Failed,
                    Some(error.code.as_str()),
                    None,
                )
                .err()
                .unwrap_or_else(|| error.clone());
                let _ = on_event.send(StreamEvent::Failed {
                    run_id: task_run_id.clone(),
                    error: event_error,
                });
                record_diagnostic(
                    &diagnostics,
                    "error",
                    "Generation failed",
                    Some(provider.provider_id()),
                    error.diagnostic.as_deref().or(Some(&error.message)),
                )
                .await;
            }
            Err(_) => {
                let event = match finish_provider_run(
                    &conversations,
                    &task_run_id,
                    ProviderRunState::Cancelled,
                    None,
                    None,
                ) {
                    Ok(()) => StreamEvent::Cancelled {
                        run_id: task_run_id.clone(),
                    },
                    Err(error) => StreamEvent::Failed {
                        run_id: task_run_id.clone(),
                        error,
                    },
                };
                let _ = on_event.send(event);
                record_diagnostic(
                    &diagnostics,
                    "info",
                    "Generation cancelled",
                    Some(provider.provider_id()),
                    Some(&format!("run {task_run_id}")),
                )
                .await;
            }
        }
        runs.lock().await.remove(&task_run_id);
    });

    Ok(ChatRun { run_id })
}

#[tauri::command]
/// Cancels an active generation by its opaque run identity.
pub(crate) async fn cancel_chat(
    run_id: String,
    state: State<'_, AppState>,
) -> Result<bool, ProviderError> {
    if let Some(handle) = state.runs.lock().await.remove(&run_id) {
        handle.abort();
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Converts the provider-neutral reasoning setting to its durable representation.
fn stored_reasoning_effort(reasoning_effort: ReasoningEffort) -> StoredReasoningEffort {
    match reasoning_effort {
        ReasoningEffort::Off => StoredReasoningEffort::Off,
        ReasoningEffort::Low => StoredReasoningEffort::Low,
    }
}

/// Persists a terminal provider outcome before it is exposed to the WebView.
fn finish_provider_run(
    conversations: &ConversationStore,
    run_id: &str,
    state: ProviderRunState,
    error_code: Option<&str>,
    usage: Option<Usage>,
) -> Result<(), ProviderError> {
    conversations
        .finish_provider_run(
            run_id,
            state,
            error_code,
            usage.map(|usage| StoredUsage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cost_usd: usage.cost_usd,
            }),
        )
        .map_err(provider_run_storage_error)
}

/// Maps secret-free storage failures into the existing provider error surface.
fn provider_run_storage_error(error: crate::storage::StorageError) -> ProviderError {
    match error.code {
        "invalid_request" | "not_found" => ProviderError::invalid_request(error.message),
        _ => ProviderError::internal(error.message, None),
    }
}
