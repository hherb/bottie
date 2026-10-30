//! The first step of a chat run: register it for cancellation and restore, then write its durable row.

use futures_util::future::AbortRegistration;

use crate::{
    inference::{ChatRequest, ReasoningEffort},
    run_cancellation::{ActiveRunRegistration, ActiveRuns, reserve_then_write},
    storage::{
        ConversationStore, NewProviderRun, ProviderAttachmentContext, ProviderRunContext,
        StorageError, StoredReasoningEffort,
    },
    tool_loop::ToolLoopCancellation,
};

/// Registers the chat run, then reads its attachment context and writes its durable row, in that order, so restore
/// can never swap the store between the two (#216). Kept out of `start_chat` so a test can pin the order (#223).
pub(crate) async fn reserve_chat_run(
    runs: &ActiveRuns,
    store: &ConversationStore,
    run_id: String,
    context: &ProviderRunContext,
    request: &ChatRequest,
) -> Result<
    (
        ActiveRunRegistration,
        AbortRegistration,
        ToolLoopCancellation,
        ProviderAttachmentContext,
    ),
    StorageError,
> {
    reserve_then_write(runs, run_id.clone(), || {
        let attachment_context = store
            .provider_attachment_context(&context.conversation_id, &context.request_message_id)?;
        store.start_provider_run(NewProviderRun {
            id: run_id,
            conversation_id: context.conversation_id.clone(),
            request_message_id: context.request_message_id.clone(),
            provider_id: request.provider_id.clone(),
            model_id: request.model_id.clone(),
            reasoning_effort: stored_reasoning_effort(request.settings.reasoning_effort),
            temperature: request.settings.temperature,
            max_output_tokens: request.settings.max_output_tokens,
        })?;
        Ok(attachment_context)
    })
    .await
}

/// Converts the provider-neutral reasoning setting to its durable representation.
fn stored_reasoning_effort(reasoning_effort: ReasoningEffort) -> StoredReasoningEffort {
    match reasoning_effort {
        ReasoningEffort::Off => StoredReasoningEffort::Off,
        ReasoningEffort::Low => StoredReasoningEffort::Low,
    }
}
