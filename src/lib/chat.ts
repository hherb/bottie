import type { ModelInfo, ProviderId, ReasoningEffort, Usage } from "./inference";

/** Number of bytes in one kibibyte, displayed with the familiar KB label. */
const BYTES_PER_KIBIBYTE = 1_024;

/** Number of kibibytes in one mebibyte, displayed with the familiar MB label. */
const KIBIBYTES_PER_MEBIBYTE = 1_024;

/** Number of milliseconds in one second. */
const MILLISECONDS_PER_SECOND = 1_000;

/** Maximum number of Unicode characters used for a generated conversation title. */
const MAX_CONVERSATION_TITLE_CHARACTERS = 80;

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
    };
  }
  if (state === "cancelled") {
    return {
      fallbackText: hasContent ? undefined : "Generation stopped.",
      meta: "Stopped · saved partial response",
      error: false,
    };
  }
  if (state === "failed") {
    return {
      fallbackText: hasContent ? undefined : "Generation failed before any response was saved.",
      meta: "Generation failed · saved partial response",
      error: true,
    };
  }
  return { fallbackText: undefined, meta: undefined, error: false };
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
