//! Native provider-run lifecycle, cancellation, and durable provenance orchestration.

use futures_util::future::{AbortHandle, Abortable};
use tauri::{State, ipc::Channel};

use crate::{
    ActiveRun, AppState,
    diagnostics::{record_diagnostic, sanitized},
    generation_tools::{memory_tools_enabled, stream_memory_tools},
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
        || (request.memory_enabled
            && matches!(
                &provider,
                RoutedProvider::Ollama(_)
                    | RoutedProvider::OpenAi(_)
                    | RoutedProvider::Anthropic(_)
            ));
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
    let supports_tools = memory_tools_enabled(
        request.memory_enabled,
        provider.provider_id(),
        model_capabilities
            .as_ref()
            .is_some_and(|capabilities| capabilities.tools),
    );
    let attachment_context = if supports_vision {
        state
            .conversations
            .load_provider_images(attachment_context)
            .map_err(provider_run_storage_error)
    } else {
        Ok(attachment_context)
    };
    let request = match attachment_context
        .and_then(|context| request_with_attachment_context(request, context, supports_vision))
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
                stream_memory_tools(
                    provider.clone(),
                    request,
                    sink,
                    conversations.clone(),
                    task_run_id.clone(),
                    query_embedder,
                    tool_cancellation,
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
mod tests {
    use super::*;
    use crate::storage::{ProviderContextImage, ProviderContextMessage};

    /// Builds the WebView text request matched by each durable context fixture.
    fn text_request(text: &str) -> ChatRequest {
        serde_json::from_value(serde_json::json!({
            "providerId": "ollama",
            "modelId": "vision-model",
            "messages": [{"role": "user", "content": [{"type": "text", "text": text}]}]
        }))
        .expect("text request should deserialize")
    }

    /// Builds one current user turn with normalized native-only bytes.
    fn image_context() -> ProviderAttachmentContext {
        ProviderAttachmentContext {
            messages: vec![ProviderContextMessage {
                role: StoredRole::User,
                text: "Describe this".into(),
                images: vec![ProviderContextImage {
                    format: ProviderImageFormat::Png,
                    sha256: "normalized".repeat(4),
                    byte_size: 10,
                    bytes: Some(b"normalized".to_vec()),
                }],
            }],
            current_request_has_image: true,
        }
    }

    #[test]
    fn adds_native_images_only_after_vision_capability_confirmation() {
        let request =
            request_with_attachment_context(text_request("Describe this"), image_context(), true)
                .expect("vision request should prepare");

        assert!(matches!(
            &request.messages[0].content[1],
            ContentBlock::Image { media_type: ImageMediaType::Png, bytes } if bytes == b"normalized"
        ));
    }

    #[test]
    fn rejects_current_images_for_text_only_models() {
        let error =
            request_with_attachment_context(text_request("Describe this"), image_context(), false)
                .expect_err("text-only request must be rejected");

        assert_eq!(error.code.as_str(), "invalid_request");
        assert_eq!(
            error.message,
            "The selected model is text-only. Choose a vision model or remove the image."
        );
    }

    #[test]
    fn omits_unloaded_historical_images_for_text_only_models() {
        let mut context = image_context();
        context.current_request_has_image = false;
        context.messages[0].images[0].bytes = None;

        let request =
            request_with_attachment_context(text_request("Describe this"), context, false)
                .expect("historical images should not block a text-only request");

        assert_eq!(request.messages[0].content.len(), 1);
        assert!(matches!(
            request.messages[0].content[0],
            ContentBlock::Text { .. }
        ));
    }

    #[test]
    fn rejects_webview_text_that_does_not_match_durable_context() {
        let error =
            request_with_attachment_context(text_request("Different text"), image_context(), true)
                .expect_err("stale WebView context must be rejected");

        assert_eq!(error.code.as_str(), "invalid_request");
    }
}
