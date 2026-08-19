import { Channel, invoke, isTauri } from "@tauri-apps/api/core";

/** Provider-qualified model metadata returned by native discovery. */
export type ModelInfo = {
  providerId: string;
  providerName: string;
  modelId: string;
  displayName: string;
  maxContextTokens: number | null;
  loadState: "loaded" | "unloaded" | "unknown";
  capabilities: {
    text: boolean;
    streaming: boolean;
    tools: boolean;
    vision: boolean;
    embeddings: boolean;
  };
};

/** One provider-neutral chat turn sent across the native boundary. */
export type ChatTurn = {
  role: "system" | "user" | "assistant";
  content: Array<{ type: "text"; text: string }>;
};

/** User-facing reasoning effort supported by the current local-provider slice. */
export type ReasoningEffort = "off" | "low";

/** Provider-qualified chat request accepted by the native inference command. */
export type ChatRequest = {
  providerId: string;
  modelId: string;
  messages: ChatTurn[];
  settings?: {
    temperature?: number;
    maxOutputTokens?: number;
    reasoningEffort?: ReasoningEffort;
  };
};

/** Provider-normalized input and output token counts. */
export type Usage = {
  inputTokens: number | null;
  outputTokens: number | null;
};

/** Stable provider failure exposed without raw provider response data. */
export type ProviderError = {
  code: "unavailable" | "timeout" | "invalid_request" | "server" | "malformed_response" | "internal";
  message: string;
  retryable: boolean;
  diagnostic?: string;
};

/** Events delivered over the typed native channel for one generation. */
export type StreamEvent =
  | { type: "started"; runId: string; providerId: string; modelId: string }
  | { type: "text_delta"; runId: string; delta: string }
  | { type: "reasoning_delta"; runId: string; delta: string }
  | { type: "usage_updated"; runId: string; usage: Usage }
  | { type: "completed"; runId: string; usage: Usage | null }
  | { type: "cancelled"; runId: string }
  | { type: "failed"; runId: string; error: ProviderError };

/** Opaque identity returned when a native generation is accepted. */
export type ChatRun = { runId: string };

/** Local inference providers currently supported by the native shell. */
export type LocalProviderId = "omlx" | "ollama";

/** Persisted non-secret local provider configuration. */
export type ProviderSettings = {
  omlxBaseUrl: string;
  ollamaBaseUrl: string;
  lastProviderId: LocalProviderId | null;
  lastModelId: string | null;
};

/** Result of testing one draft provider endpoint. */
export type ProviderConnectionTest = {
  providerId: string;
  baseUrl: string;
  modelCount: number;
  elapsedMs: number;
  message: string;
};

/** Secret-redacted native provider diagnostic. */
export type DiagnosticEntry = {
  timestampMs: number;
  level: "info" | "warn" | "error";
  event: string;
  providerId: string | null;
  detail: string | null;
};

/** Produces the normalized failure returned when native commands are unavailable. */
function unavailableInBrowser(): ProviderError {
  return {
    code: "unavailable",
    message: "Native inference is unavailable in the browser preview. Open the Tauri app to use a local provider.",
    retryable: false,
  };
}

/** Discovers provider-qualified models through the native boundary. */
export async function discoverModels(providerId?: LocalProviderId): Promise<ModelInfo[]> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ModelInfo[]>("discover_models", { providerId: providerId ?? null });
}

/** Reads persisted local provider settings from the native application. */
export async function getProviderSettings(): Promise<ProviderSettings> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ProviderSettings>("get_provider_settings");
}

/** Validates and persists local provider settings through the native application. */
export async function updateProviderSettings(settings: ProviderSettings): Promise<ProviderSettings> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ProviderSettings>("update_provider_settings", { settings });
}

/** Persists the last successfully selected provider and model pair. */
export async function rememberProviderSelection(
  providerId: LocalProviderId,
  modelId: string,
): Promise<ProviderSettings> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ProviderSettings>("remember_provider_selection", {
    selection: { providerId, modelId },
  });
}

/** Tests a draft provider endpoint without changing active settings. */
export async function testProviderConnection(
  providerId: LocalProviderId,
  baseUrl: string,
): Promise<ProviderConnectionTest> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ProviderConnectionTest>("test_provider_connection", {
    draft: { providerId, baseUrl },
  });
}

/** Reads the bounded, secret-redacted native diagnostic history. */
export async function getDiagnostics(): Promise<DiagnosticEntry[]> {
  if (!isTauri()) return [];
  return invoke<DiagnosticEntry[]>("get_diagnostics");
}

/** Starts a provider-qualified native chat stream. */
export async function startChat(request: ChatRequest, onEvent: (event: StreamEvent) => void): Promise<ChatRun> {
  if (!isTauri()) throw unavailableInBrowser();
  const channel = new Channel<StreamEvent>();
  channel.onmessage = onEvent;
  return invoke<ChatRun>("start_chat", { request, onEvent: channel });
}

/** Requests cancellation of one native generation by opaque run identity. */
export async function cancelChat(runId: string): Promise<boolean> {
  if (!isTauri()) return false;
  return invoke<boolean>("cancel_chat", { runId });
}

/** Converts an unknown native invocation failure into the stable provider error shape. */
export function providerErrorFromUnknown(error: unknown): ProviderError {
  if (typeof error === "object" && error !== null && "message" in error) {
    const candidate = error as Partial<ProviderError>;
    if (typeof candidate.message === "string") {
      return {
        code: candidate.code ?? "internal",
        message: candidate.message,
        retryable: candidate.retryable ?? false,
        diagnostic: candidate.diagnostic,
      };
    }
  }
  return {
    code: "internal",
    message: typeof error === "string" ? error : "Local inference failed unexpectedly.",
    retryable: false,
  };
}
