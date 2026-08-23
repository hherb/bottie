/** Pure presentation helpers for provider routing and visible attachment context. */

import { displayEndpoint } from "$lib/chat";
import type { ModelInfo, ProviderId, ProviderSettings, ReasoningEffort } from "$lib/inference";
import type { Attachment, InferenceStage, Message, MessageAttachment } from "$lib/presentation";

/** Providers with a Bottie-owned native tool loop after capability discovery. */
const NATIVE_TOOL_PROVIDER_IDS: ReadonlySet<string> = new Set(["omlx", "ollama", "openai", "anthropic"]);
/** Providers with an explicit Localmail mapping in Bottie's native tool loop. */
const EMAIL_TOOL_PROVIDER_IDS: ReadonlySet<string> = new Set(["ollama", "openai"]);
/** Actionable setup guidance shared by the disabled Email control and its status note. */
const EMAIL_SETUP_REASON = "Save Localmail certificate trust and a bearer token in Settings before enabling Email.";
/** Combined connector and provider guidance when more than one prerequisite is absent. */
const EMAIL_SETUP_AND_PROVIDER_REASON = [
  "Save Localmail certificate trust and a bearer token in Settings, then switch to a tool-capable Ollama",
  "or OpenAI-compatible model.",
].join(" ");
/** Supported-provider guidance without exposing connector configuration detail. */
const EMAIL_PROVIDER_REASON = [
  "Email is currently mapped only for Ollama and OpenAI-compatible models.",
  "Switch to a supported tool-capable model.",
].join(" ");
/** Exact enabled disclosure for an OpenAI-compatible generation. */
const OPENAI_EMAIL_BOUNDARY_NOTE = [
  "Your prompt and bounded Localmail tool results go to the selected OpenAI-compatible cloud endpoint;",
  "model-selected email queries and exact message IDs go only to your pinned Localmail server.",
].join(" ");
/** Exact enabled disclosure for an Ollama generation. */
const OLLAMA_EMAIL_BOUNDARY_NOTE = [
  "Your prompt stays with Ollama on loopback; model-selected email queries and exact message IDs go only",
  "to your pinned Localmail server.",
].join(" ");

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

/** Confirms that Email is mapped only for an explicitly tool-capable supported provider. */
export function emailToolsAvailable(model: ModelInfo | undefined): boolean {
  return Boolean(model?.capabilities.tools && EMAIL_TOOL_PROVIDER_IDS.has(model.providerId));
}

/** Explains every unmet Email prerequisite without exposing connector details. */
export function emailToolsUnavailableReason(model: ModelInfo | undefined, localmailConfigured: boolean): string {
  const supportedProviderSelected = model?.providerId === "ollama" || model?.providerId === "openai";
  const toolsAdvertised = Boolean(model?.capabilities.tools);
  if (!localmailConfigured && (!supportedProviderSelected || !toolsAdvertised)) {
    return EMAIL_SETUP_AND_PROVIDER_REASON;
  }
  if (!localmailConfigured) {
    return EMAIL_SETUP_REASON;
  }
  if (!supportedProviderSelected) {
    return EMAIL_PROVIDER_REASON;
  }
  if (!toolsAdvertised) {
    const providerName = model?.providerId === "openai" ? "OpenAI-compatible" : "Ollama";
    return [
      `The selected ${providerName} model does not advertise tool support.`,
      `Choose a tool-capable ${providerName} model.`,
    ].join(" ");
  }
  return "";
}

/** Describes the two exact delivery routes used by an enabled Email request. */
export function emailToolsBoundaryNote(model: ModelInfo | undefined): string {
  if (model?.providerId === "openai") {
    return OPENAI_EMAIL_BOUNDARY_NOTE;
  }
  return OLLAMA_EMAIL_BOUNDARY_NOTE;
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
