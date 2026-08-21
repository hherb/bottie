import type { ChatTurn, ModelInfo, ProviderError, ProviderId, ReasoningEffort, Usage } from "./inference";
import type { Attachment, Message } from "./presentation";
import type { ResponseRating } from "./storage";

/** Number of bytes in one kibibyte, displayed with the familiar KB label. */
const BYTES_PER_KIBIBYTE = 1_024;

/** Number of kibibytes in one mebibyte, displayed with the familiar MB label. */
const KIBIBYTES_PER_MEBIBYTE = 1_024;

/** Number of milliseconds in one second. */
const MILLISECONDS_PER_SECOND = 1_000;

/** Maximum number of Unicode characters used for a generated conversation title. */
const MAX_CONVERSATION_TITLE_CHARACTERS = 80;

/** Provider failures whose original request may succeed when attempted again. */
const RETRYABLE_PROVIDER_ERROR_CODES = new Set<ProviderError["code"]>([
  "unavailable",
  "timeout",
  "server",
  "malformed_response",
]);

/** The provider-scoped result of resolving a model selection. */
export type ModelSelection = {
  providerId: ProviderId | "";
  models: ModelInfo[];
  selectedModelKey: string;
};

/** Durable message states that affect reopened assistant presentation. */
export type PersistedMessageState = "partial" | "final" | "cancelled" | "failed";

/** Presentation details derived from a durable assistant terminal state. */
export type PersistedMessagePresentation = {
  fallbackText: string | undefined;
  meta: string | undefined;
  error: boolean;
  retryable: boolean;
};

/** Creates a collision-safe key for a provider and model pair. */
export function modelKey(model: Pick<ModelInfo, "providerId" | "modelId">): string {
  return `${model.providerId}:${model.modelId}`;
}

/** Derives a compact single-line title from the first user prompt. */
export function conversationTitle(prompt: string): string {
  return Array.from(prompt.trim().replace(/\s+/g, " ")).slice(0, MAX_CONVERSATION_TITLE_CHARACTERS).join("");
}

/** Formats a provider root URL for compact display in the interface. */
export function displayEndpoint(baseUrl: string): string {
  return baseUrl.replace(/^https?:\/\//, "").replace(/\/$/, "");
}

/** Returns whether selecting a provider will transmit prompt content off-device. */
export function isCloudProvider(providerId: ProviderId | ""): boolean {
  return providerId === "openai" || providerId === "anthropic";
}

/** Formats a byte count using the binary thresholds used by the attachment preview. */
export function formatBytes(bytes: number): string {
  if (bytes < BYTES_PER_KIBIBYTE) return `${bytes} B`;
  if (bytes < BYTES_PER_KIBIBYTE * KIBIBYTES_PER_MEBIBYTE) {
    return `${Math.round(bytes / BYTES_PER_KIBIBYTE)} KB`;
  }
  const mebibytes = bytes / (BYTES_PER_KIBIBYTE * KIBIBYTES_PER_MEBIBYTE);
  return `${mebibytes.toFixed(1)} MB`;
}

/** Builds the completion metadata shown under an assistant response. */
export function completionMeta(startedAt: number, finishedAt: number, usage: Usage | null): string {
  return completionMetaForDuration(finishedAt - startedAt, usage);
}

/** Reconstructs completion metadata from a terminal durable provider run. */
export function persistedCompletionMeta(
  startedAtMs: number,
  completedAtMs: number | null,
  usage: Usage | null,
): string | undefined {
  if (completedAtMs === null) return undefined;
  return completionMetaForDuration(Math.max(0, completedAtMs - startedAtMs), usage);
}

/** Derives stable reopened-response labels without exposing provider diagnostics. */
export function persistedMessagePresentation(
  state: PersistedMessageState,
  errorCode: string | null,
  hasContent: boolean,
): PersistedMessagePresentation {
  if (state === "partial" && errorCode === "interrupted") {
    return {
      fallbackText: hasContent ? undefined : "Generation interrupted before any response was saved.",
      meta: "Interrupted · saved partial response",
      error: true,
      retryable: true,
    };
  }
  if (state === "cancelled") {
    return {
      fallbackText: hasContent ? undefined : "Generation stopped.",
      meta: "Stopped · saved partial response",
      error: false,
      retryable: true,
    };
  }
  if (state === "failed") {
    return {
      fallbackText: hasContent ? undefined : "Generation failed before any response was saved.",
      meta: "Generation failed · saved partial response",
      error: true,
      retryable: RETRYABLE_PROVIDER_ERROR_CODES.has(errorCode as ProviderError["code"]),
    };
  }
  return { fallbackText: undefined, meta: undefined, error: false, retryable: false };
}

/** Finds the durable user request immediately preceding one rendered assistant response. */
export function requestMessageForResponse(messages: Message[], responseId: number): Message | undefined {
  const responseIndex = messages.findIndex((message) => message.id === responseId && message.role === "assistant");
  if (responseIndex <= 0) return undefined;
  const request = messages[responseIndex - 1];
  return request.role === "user" && request.storageId ? request : undefined;
}

/** Selects a requested response rating, or clears it when the active choice is selected again. */
export function nextResponseRating(current: ResponseRating | null, selected: ResponseRating): ResponseRating | null {
  return current === selected ? null : selected;
}

/** Converts visible, successful message text into provider-neutral chat turns. */
export function chatTurnsForMessages(messages: Message[]): ChatTurn[] {
  return messages
    .filter((message) => message.content.trim() !== "" && !message.error)
    .map((message) => ({
      role: message.role,
      content: [{ type: "text", text: message.content }],
    }));
}

/** Describes whether one path-free attachment participates in the next model request. */
export function attachmentDeliveryLabel(attachment: Attachment, model: ModelInfo | undefined): string {
  if (attachment.kind !== "image") return "Not sent";
  if (attachment.normalization.state === "pending") return "Not sent · normalization pending";
  if (attachment.normalization.state !== "ready") return "Not sent · image unavailable for delivery";
  if (!model) return "Not sent · choose a vision model";
  return model.capabilities.vision ? "Included with vision requests" : "Not sent · selected model is text-only";
}

/** Returns the current-draft image policy that must be resolved before persistence and generation. */
export function draftImageDeliveryBlocker(attachments: Attachment[], model: ModelInfo | undefined): string | null {
  const images = attachments.filter((attachment) => attachment.kind === "image");
  if (images.length === 0) return null;
  if (images.some((attachment) => attachment.normalization.state === "pending")) {
    return "Wait for image normalization to finish before sending.";
  }
  if (images.some((attachment) => attachment.normalization.state !== "ready")) {
    return "Remove images that could not be normalized as JPEG or PNG before sending.";
  }
  if (model && !model.capabilities.vision) {
    return "The selected model is text-only. Choose a vision model or remove the image.";
  }
  return null;
}

/** Summarizes draft attachment transmission without implying that documents are delivered. */
export function composerAttachmentNote(attachments: Attachment[], model: ModelInfo | undefined): string {
  if (attachments.length === 0) return "Bottie can make mistakes. Check important information.";
  const blocker = draftImageDeliveryBlocker(attachments, model);
  if (blocker) return blocker;
  const hasReadyImage = attachments.some(
    (attachment) => attachment.kind === "image" && attachment.normalization.state === "ready",
  );
  const hasDocument = attachments.some((attachment) => attachment.kind !== "image");
  if (hasReadyImage && !model) return "Choose a vision-capable model to send normalized images.";
  if (hasReadyImage && hasDocument) {
    return "Normalized images will be sent; document attachments stay linked locally.";
  }
  if (hasReadyImage) return "Normalized images will be sent to the selected vision model.";
  return "Document attachments stay linked locally; only your text is sent.";
}

/** Formats measured duration and provider-reported usage without estimating missing values. */
function completionMetaForDuration(durationMs: number, usage: Usage | null): string {
  const seconds = (durationMs / MILLISECONDS_PER_SECOND).toFixed(1);
  const outputTokens = usage?.outputTokens;
  const cost = usage?.costUsd;
  const usageLabel = outputTokens == null ? "usage unavailable" : `${outputTokens} tokens`;
  const costLabel = cost == null ? "" : ` · $${cost.toFixed(4)}`;
  return `${seconds}s · ${usageLabel}${costLabel}`;
}

/** Retains models that can participate in the current streaming text interface. */
export function filterUsableModels(models: ModelInfo[]): ModelInfo[] {
  return models.filter((model) => model.capabilities.text && model.capabilities.streaming);
}

/** Toggles between disabled reasoning and the lowest enabled effort. */
export function toggleReasoningEffort(current: ReasoningEffort): ReasoningEffort {
  return current === "off" ? "low" : "off";
}

/** Resolves a provider-scoped model list and selection from discovery results and remembered state. */
export function resolveModelSelection(
  usableModels: ModelInfo[],
  requestedProviderId: ProviderId | "",
  rememberedProviderId: ProviderId | null,
  rememberedModelId: string | null,
): ModelSelection {
  const rememberedProviderAvailable =
    rememberedProviderId !== null && usableModels.some((model) => model.providerId === rememberedProviderId);
  const providerId =
    requestedProviderId ||
    (rememberedProviderAvailable ? rememberedProviderId : undefined) ||
    (usableModels[0]?.providerId as ProviderId | undefined) ||
    "";
  const models = usableModels.filter((model) => model.providerId === providerId);
  const rememberedModel = models.find(
    (model) => providerId === rememberedProviderId && model.modelId === rememberedModelId,
  );
  const selectedModel = rememberedModel ?? models[0];
  return {
    providerId,
    models,
    selectedModelKey: selectedModel ? modelKey(selectedModel) : "",
  };
}
