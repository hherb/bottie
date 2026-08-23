/** Pure presentation helpers for provider routing and visible attachment context. */

import { displayEndpoint } from "$lib/chat";
import type { ModelInfo, ProviderId, ProviderSettings, ReasoningEffort } from "$lib/inference";
import type { Attachment, InferenceStage, Message, MessageAttachment } from "$lib/presentation";

/** Providers with a Bottie-owned native tool loop after capability discovery. */
const NATIVE_TOOL_PROVIDER_IDS: ReadonlySet<string> = new Set(["omlx", "ollama", "openai", "anthropic"]);

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

/** Confirms that Bottie maps native memory tools for the selected advertised capability. */
export function memoryToolsAvailable(model: ModelInfo | undefined): boolean {
  return Boolean(model?.capabilities.tools && NATIVE_TOOL_PROVIDER_IDS.has(model.providerId));
}

/** Confirms that Bottie maps native web search for the selected advertised capability. */
export function webToolsAvailable(model: ModelInfo | undefined): boolean {
  return Boolean(model?.capabilities.tools && NATIVE_TOOL_PROVIDER_IDS.has(model.providerId));
}

/** Confirms that Email is mapped only for an explicitly tool-capable Ollama model. */
export function emailToolsAvailable(model: ModelInfo | undefined): boolean {
  return Boolean(model?.capabilities.tools && model.providerId === "ollama");
}

/** Explains every unmet Email prerequisite without exposing connector details. */
export function emailToolsUnavailableReason(model: ModelInfo | undefined, localmailConfigured: boolean): string {
  const ollamaSelected = model?.providerId === "ollama";
  const toolsAdvertised = Boolean(model?.capabilities.tools);
  if (!localmailConfigured && (!ollamaSelected || !toolsAdvertised)) {
    return "Save Localmail certificate trust and a bearer token in Settings, then switch to a tool-capable Ollama model.";
  }
  if (!localmailConfigured) {
    return "Save Localmail certificate trust and a bearer token in Settings before enabling Email.";
  }
  if (!ollamaSelected) {
    return "Email is currently mapped only for Ollama. Switch to a tool-capable Ollama model.";
  }
  if (!toolsAdvertised) {
    return "The selected Ollama model does not advertise tool support. Choose a tool-capable Ollama model.";
  }
  return "";
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
  webEnabled: boolean,
  providerName: string | undefined,
  webSearchProviderName: string,
  reasoningEffort: ReasoningEffort,
): InferenceStage[] {
  return [
    {
      icon: "shield",
      label: isLocalRoute
        ? webEnabled
          ? "Local model with web access"
          : "Connected locally"
        : "Cloud route confirmed",
      detail: `Rust → ${providerName ?? "provider"}${webEnabled ? ` + ${webSearchProviderName}` : ""}`,
    },
    {
      icon: "sparkles",
      label: "Streaming response",
      detail: reasoningEffort === "low" ? "Low reasoning" : "Reasoning off",
    },
  ];
}
