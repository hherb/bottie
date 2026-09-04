//! Native provider-run lifecycle, cancellation, and durable provenance orchestration.

use futures_util::future::{AbortHandle, Abortable};
use tauri::{State, ipc::Channel};

use crate::{
    AppState,
    diagnostics::{record_diagnostic, sanitized},
    generation_context::{
        captured_audio_error, normalize_provider_request, request_with_attachment_context,
    },
    generation_localmail_tools::{configured_localmail_tools, email_tools_enabled},
    generation_tools::stream_native_tools,
    generation_web_tools::{
        configured_web_fetch, configured_web_search, memory_tools_enabled, provider_tools_enabled,
        web_fetch_enabled, web_tools_enabled, with_web_citation_guidance,
    },
    inference::{ChatRequest, ChatRun, ProviderError, ReasoningEffort, StreamEvent, Usage},
    provider_registry::{RoutedProvider, routed_provider},
    python_runtime::PythonRuntimeState,
    run_cancellation::ActiveRun,
    storage::{
        ConversationStore, NewProviderRun, ProviderRunContext, ProviderRunState,
        StoredReasoningEffort, StoredUsage,
    },
    stream_channel::ChannelSink,
    tool_loop::ToolLoopCancellation,
};

#[tauri::command]
/// Starts one cancellable provider-qualified chat generation with durable provenance.
pub(crate) async fn start_chat(
    state: State<'_, AppState>,
    request: ChatRequest,
    context: ProviderRunContext,
    on_event: Channel<StreamEvent>,
) -> Result<ChatRun, ProviderError> {
    if state.microphone.is_capturing() {
        return Err(ProviderError::invalid_request(
            "Stop or discard local voice capture before sending a message.",
        ));
    }
    let request = normalize_provider_request(request);
    let providers = state.providers.read().await.clone();
    let run_id = uuid::Uuid::new_v4().to_string();
    let attachment_context = state
        .conversations
        .provider_attachment_context(&context.conversation_id, &context.request_message_id)
        .map_err(provider_run_storage_error)?;
    state
        .conversations
        .start_provider_run(NewProviderRun {
            id: run_id.clone(),
            conversation_id: context.conversation_id.clone(),
            request_message_id: context.request_message_id.clone(),
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
    let has_images = attachment_context
        .messages
        .iter()
        .any(|message| !message.images.is_empty());
    let needs_model_capabilities = has_images
        || matches!(
            &provider,
            RoutedProvider::Omlx(_)
                | RoutedProvider::Ollama(_)
                | RoutedProvider::OpenAi(_)
                | RoutedProvider::Anthropic(_)
        );
    let model_capabilities = if needs_model_capabilities {
        match provider.discover_models().await {
            Ok(models) => models
                .iter()
                .find(|model| model.model_id == request.model_id)
                .map(|model| model.capabilities.clone()),
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
        }
    } else {
        None
    };
    let supports_vision = model_capabilities
        .as_ref()
        .is_some_and(|capabilities| capabilities.vision);
    let supports_audio = model_capabilities
        .as_ref()
        .is_some_and(|capabilities| capabilities.audio)
        && matches!(
            &provider,
            RoutedProvider::Omlx(_) | RoutedProvider::OpenAi(_)
        );
    let captured_audio = if request.audio_enabled || request.retain_audio {
        if request.audio_enabled && !supports_audio {
            let error = ProviderError::invalid_request(
                "The selected model does not advertise compatible audio input support.",
            );
            finish_provider_run(
                &state.conversations,
                &run_id,
                ProviderRunState::Failed,
                Some(error.code.as_str()),
                None,
            )?;
            return Err(error);
        }
        match state.microphone.captured_audio() {
            Ok(audio) => Some(audio),
            Err(error) => {
                let error = captured_audio_error(error);
                finish_provider_run(
                    &state.conversations,
                    &run_id,
                    ProviderRunState::Failed,
                    Some(error.code.as_str()),
                    None,
                )?;
                return Err(error);
            }
        }
    } else {
        None
    };
    if request.retain_audio {
        let Some(audio) = captured_audio.as_ref() else {
            let error = ProviderError::invalid_request(
                "Local audio retention requires a stopped recording.",
            );
            finish_provider_run(
                &state.conversations,
                &run_id,
                ProviderRunState::Failed,
                Some(error.code.as_str()),
                None,
            )?;
            return Err(error);
        };
        let retained = state
            .conversations
            .ingest_native_audio(&audio.bytes)
            .and_then(|attachment| {
                state.conversations.associate_attachment_with_request(
                    &context.conversation_id,
                    &context.request_message_id,
                    &attachment.id,
                )
            });
        if let Err(error) = retained {
            let error = provider_run_storage_error(error);
            finish_provider_run(
                &state.conversations,
                &run_id,
                ProviderRunState::Failed,
                Some(error.code.as_str()),
                None,
            )?;
            return Err(error);
        }
        state.attachment_processing.wake();
    }
    let supports_memory_tools = memory_tools_enabled(
        request.memory_enabled,
        provider.provider_id(),
        model_capabilities
            .as_ref()
            .is_some_and(|capabilities| capabilities.tools),
    );
    let supports_web_tools = web_tools_enabled(
        request.web_enabled,
        provider.provider_id(),
        model_capabilities
            .as_ref()
            .is_some_and(|capabilities| capabilities.tools),
    );
    let model_supports_tools = model_capabilities
        .as_ref()
        .is_some_and(|capabilities| capabilities.tools);
    let wants_email_tools = email_tools_enabled(
        request.email_enabled,
        provider.provider_id(),
        model_supports_tools,
        true,
    );
    let localmail = if wants_email_tools {
        match configured_localmail_tools(
            state.localmail_config_path.clone(),
            state.credentials.clone(),
        ) {
            Ok(executor) => executor,
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
        }
    } else {
        None
    };
    let supports_email_tools = email_tools_enabled(
        request.email_enabled,
        provider.provider_id(),
        model_supports_tools,
        localmail.is_some(),
    );
    let provider_settings = providers.settings();
    let web_search = if supports_web_tools {
        match configured_web_search(
            &provider_settings.web_search_provider_id,
            state.credentials.as_ref(),
            provider_settings.web_network_policy.clone(),
        ) {
            Ok(provider) => Some(provider),
            Err(error) => {
                finish_provider_run(
                    &state.conversations,
                    &run_id,
                    ProviderRunState::Failed,
                    Some(error.code.as_str()),
                    None,
                )?;
                return Err(error);
            }
        }
    } else {
        None
    };
    let web_fetch = web_fetch_enabled(supports_web_tools, provider.provider_id())
        .then(|| configured_web_fetch(provider_settings.web_network_policy));
    let supports_tools = provider_tools_enabled(provider.provider_id(), model_supports_tools);
    let attachment_context = if supports_vision {
        state
            .conversations
            .load_provider_images(attachment_context)
            .map_err(provider_run_storage_error)
    } else {
        Ok(attachment_context)
    };
    let provider_audio = request
        .audio_enabled
        .then(|| captured_audio.clone())
        .flatten();
    let consumes_audio = request.audio_enabled || request.retain_audio;
    let mut request = match attachment_context
        .and_then(|context| {
            request_with_attachment_context(request, context, supports_vision, provider_audio)
        })
        .map(|request| with_web_citation_guidance(request, supports_web_tools))
    {
        Ok(request) => request,
        Err(error) => {
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
    request.memory_enabled = supports_memory_tools;
    request.web_enabled = supports_web_tools;
    request.email_enabled = supports_email_tools;
    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    let tool_cancellation = ToolLoopCancellation::default();
    {
        let _voice_guard = state.voice_interaction.lock().await;
        if state.microphone.is_capturing() {
            let error = ProviderError::invalid_request(
                "Stop or discard local voice capture before sending a message.",
            );
            finish_provider_run(
                &state.conversations,
                &run_id,
                ProviderRunState::Failed,
                Some(error.code.as_str()),
                None,
            )?;
            return Err(error);
        }
        state.runs.lock().await.insert(
            run_id.clone(),
            ActiveRun {
                abort_handle,
                tool_cancellation: tool_cancellation.clone(),
            },
        );
    }
    if consumes_audio {
        state.microphone.discard();
    }

    let runs = state.runs.clone();
    let diagnostics = state.diagnostics.clone();
    let conversations = state.conversations.clone();
    let semantic_indexing = state.semantic_indexing.clone();
    let python_approval = state.python_approval.clone();
    let python_runner = state
        .python_runtime
        .as_ref()
        .map(PythonRuntimeState::runner);
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
        let query_embedder = semantic_indexing.query_embedder();
        let generation = async {
            if supports_tools {
                stream_native_tools(
                    provider.clone(),
                    request,
                    sink,
                    conversations.clone(),
                    task_run_id.clone(),
                    query_embedder,
                    tool_cancellation,
                    localmail,
                    web_search,
                    web_fetch,
                    python_approval,
                    python_runner,
                )
                .await
            } else {
                provider.stream_chat(request, sink).await
            }
        };
        match Abortable::new(generation, abort_registration).await {
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
        semantic_indexing.wake();
        runs.lock().await.remove(&task_run_id);
    });

    Ok(ChatRun { run_id })
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

#[cfg(test)]
#[path = "generation_tests.rs"]
mod tests;
