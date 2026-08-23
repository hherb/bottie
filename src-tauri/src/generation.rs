//! Native provider-run lifecycle, cancellation, and durable provenance orchestration.

use futures_util::future::{AbortHandle, Abortable};
use tauri::{State, ipc::Channel};

use crate::{
    ActiveRun, AppState,
    diagnostics::{record_diagnostic, sanitized},
    generation_localmail_tools::{configured_localmail_tools, email_tools_enabled},
    generation_tools::stream_native_tools,
    generation_web_tools::{
        configured_web_fetch, configured_web_search, memory_tools_enabled, provider_tools_enabled,
        web_fetch_enabled, web_tools_enabled, with_web_citation_guidance,
    },
    inference::{
        ChatRequest, ChatRole, ChatRun, ChatTurn, ContentBlock, ImageMediaType, ProviderError,
        ReasoningEffort, StreamEvent, Usage,
    },
    provider_registry::{RoutedProvider, routed_provider},
    storage::{
        ConversationStore, NewProviderRun, ProviderAttachmentContext, ProviderImageFormat,
        ProviderRunContext, ProviderRunState, StoredReasoningEffort, StoredRole, StoredUsage,
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
    let mut request = match attachment_context
        .and_then(|context| request_with_attachment_context(request, context, supports_vision))
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
    state.runs.lock().await.insert(
        run_id.clone(),
        ActiveRun {
            abort_handle,
            tool_cancellation: tool_cancellation.clone(),
        },
    );

    let runs = state.runs.clone();
    let diagnostics = state.diagnostics.clone();
    let conversations = state.conversations.clone();
    let semantic_indexing = state.semantic_indexing.clone();
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

/// Removes provider-neutral sampling defaults that Anthropic may reject before durable provenance is recorded.
fn normalize_provider_request(mut request: ChatRequest) -> ChatRequest {
    if request.provider_id == "anthropic" {
        request.settings.temperature = None;
    }
    request
}

/// Reconciles WebView text with durable selected-lineage context and adds native images when allowed.
fn request_with_attachment_context(
    mut request: ChatRequest,
    context: ProviderAttachmentContext,
    supports_vision: bool,
) -> Result<ChatRequest, ProviderError> {
    let provided_request = request
        .messages
        .iter()
        .rev()
        .find(|turn| turn.role == ChatRole::User)
        .map(text_for_turn);
    let durable_request = context
        .messages
        .iter()
        .rev()
        .find(|message| message.role == StoredRole::User)
        .map(|message| message.text.as_str());
    if provided_request.as_deref() != durable_request {
        return Err(ProviderError::invalid_request(
            "The provider request no longer matches the selected conversation branch.",
        ));
    }
    if context.current_request_has_image && !supports_vision {
        return Err(ProviderError::invalid_request(
            "The selected model is text-only. Choose a vision model or remove the image.",
        ));
    }
    request.messages = context
        .messages
        .into_iter()
        .map(|message| -> Result<ChatTurn, ProviderError> {
            let mut content = vec![ContentBlock::Text { text: message.text }];
            if supports_vision {
                for image in message.images {
                    content.push(ContentBlock::Image {
                        media_type: match image.format {
                            ProviderImageFormat::Jpeg => ImageMediaType::Jpeg,
                            ProviderImageFormat::Png => ImageMediaType::Png,
                        },
                        bytes: image.bytes.ok_or_else(|| {
                            ProviderError::internal(
                                "A normalized image was unavailable for provider delivery.",
                                None,
                            )
                        })?,
                    });
                }
            }
            Ok(ChatTurn {
                role: chat_role(message.role),
                content,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(request)
}

/// Joins WebView-supplied text blocks while native image variants remain impossible to deserialize.
fn text_for_turn(turn: &ChatTurn) -> String {
    turn.content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Maps the durable two-role schema into provider-neutral chat roles.
fn chat_role(role: StoredRole) -> ChatRole {
    match role {
        StoredRole::User => ChatRole::User,
        StoredRole::Assistant => ChatRole::Assistant,
    }
}

#[tauri::command]
/// Cancels an active generation by its opaque run identity.
pub(crate) async fn cancel_chat(
    run_id: String,
    state: State<'_, AppState>,
) -> Result<bool, ProviderError> {
    if let Some(run) = state.runs.lock().await.remove(&run_id) {
        run.tool_cancellation.cancel();
        run.abort_handle.abort();
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

#[cfg(test)]
#[path = "generation_tests.rs"]
mod tests;
