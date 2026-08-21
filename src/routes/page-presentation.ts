/** Pure presentation helpers for provider routing and visible attachment context. */

import { displayEndpoint } from "$lib/chat";
import type { ProviderId, ProviderSettings, ReasoningEffort } from "$lib/inference";
import type { Attachment, InferenceStage, Message, MessageAttachment } from "$lib/presentation";

/** Flattens visible message associations while retaining their owning durable identities. */
export function messageAttachmentAssociations(messages: Message[]): MessageAttachment[] {
  return messages.flatMap((message) =>
    message.storageId
      ? (message.attachments ?? []).map((attachment) => ({ messageId: message.storageId!, attachment }))
      : [],
  );
}

/** Combines draft and branch-independent attachments for next-request policy and feedback. */
export function nextRequestAttachments(draft: Attachment[], conversation: Attachment[]): Attachment[] {
  return [...draft, ...conversation];
}

/** Selects the compact endpoint label for the active provider. */
export function selectedProviderEndpoint(providerId: ProviderId | "", settings: ProviderSettings): string {
  const baseUrl = {
    ollama: settings.ollamaBaseUrl,
    omlx: settings.omlxBaseUrl,
    openai: settings.openaiBaseUrl,
    anthropic: settings.anthropicBaseUrl,
  }[providerId || "ollama"];
  return displayEndpoint(baseUrl);
}

/** Builds normalized activity stages for the current privacy route and reasoning policy. */
export function inferenceStages(
  isLocalRoute: boolean,
  providerName: string | undefined,
  reasoningEffort: ReasoningEffort,
): InferenceStage[] {
  return [
    {
      icon: "shield",
      label: isLocalRoute ? "Connected locally" : "Cloud route confirmed",
      detail: `Rust → ${providerName ?? "provider"}`,
    },
    {
      icon: "sparkles",
      label: "Streaming response",
      detail: reasoningEffort === "low" ? "Low reasoning" : "Reasoning off",
    },
  ];
}
