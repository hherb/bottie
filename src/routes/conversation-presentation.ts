/** Pure reconstruction of durable native messages for the conversation surface. */

import { persistedCompletionMeta, persistedMessagePresentation } from "$lib/chat";
import { nextMessageId, type Message } from "$lib/presentation";
import { convertFileSrc } from "@tauri-apps/api/core";

import { kastellanResponsePresentation } from "$lib/kastellan";
import { storedAttachmentToPresentation, type StoredMessage } from "$lib/storage";

/** Maps one durable record into the richer ephemeral presentation shape. */
export function storedMessageToPresentation(message: StoredMessage): Message {
  const model = message.modelId && message.providerId ? `${message.modelId} · ${message.providerId}` : undefined;
  const completedMeta = message.providerRun
    ? persistedCompletionMeta(
        message.providerRun.startedAtMs,
        message.providerRun.completedAtMs,
        message.providerRun.usage,
      )
    : undefined;
  const hasContent = message.text.length > 0 || Boolean(message.reasoning);
  const presentation = persistedMessagePresentation(message.state, message.providerRun?.errorCode ?? null, hasContent);
  const kastellan = kastellanResponsePresentation(message);
  return {
    id: nextMessageId(),
    storageId: message.id,
    role: message.role,
    content: message.text || presentation.fallbackText || "",
    reasoning: kastellan ? kastellan.reasoning : (message.reasoning ?? undefined),
    model: kastellan?.model ?? model,
    meta: kastellan?.meta ?? presentation.meta ?? completedMeta,
    error: presentation.error,
    retryable: presentation.retryable,
    rating: message.rating ?? undefined,
    toolInvocations: message.providerRun?.toolInvocations,
    attachments: message.attachments.map(storedAttachmentToPresentation),
    generatedAssets: message.generatedAssets.map((asset) => ({
      ...asset,
      previewUrl: asset.status === "completed" ? convertFileSrc(asset.id, "bottie-generated-asset") : null,
    })),
  };
}
