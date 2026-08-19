import type { LocalProviderId, ModelInfo, ReasoningEffort, Usage } from "./inference";

/** Number of bytes in one kibibyte, displayed with the familiar KB label. */
const BYTES_PER_KIBIBYTE = 1_024;

/** Number of kibibytes in one mebibyte, displayed with the familiar MB label. */
const KIBIBYTES_PER_MEBIBYTE = 1_024;

/** Number of milliseconds in one second. */
const MILLISECONDS_PER_SECOND = 1_000;

/** The provider-scoped result of resolving a model selection. */
export type ModelSelection = {
  providerId: LocalProviderId | "";
  models: ModelInfo[];
  selectedModelKey: string;
};

/** Creates a collision-safe key for a provider and model pair. */
export function modelKey(model: Pick<ModelInfo, "providerId" | "modelId">): string {
  return `${model.providerId}:${model.modelId}`;
}

/** Formats a provider root URL for compact display in the interface. */
export function displayEndpoint(baseUrl: string): string {
  return baseUrl.replace(/^https?:\/\//, "").replace(/\/$/, "");
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
  const seconds = ((finishedAt - startedAt) / MILLISECONDS_PER_SECOND).toFixed(1);
  const outputTokens = usage?.outputTokens;
  return outputTokens == null ? `${seconds}s · local` : `${seconds}s · ${outputTokens} tokens`;
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
  requestedProviderId: LocalProviderId | "",
  rememberedProviderId: LocalProviderId | null,
  rememberedModelId: string | null,
): ModelSelection {
  const rememberedProviderAvailable =
    rememberedProviderId !== null && usableModels.some((model) => model.providerId === rememberedProviderId);
  const providerId =
    requestedProviderId ||
    (rememberedProviderAvailable ? rememberedProviderId : undefined) ||
    (usableModels[0]?.providerId as LocalProviderId | undefined) ||
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
