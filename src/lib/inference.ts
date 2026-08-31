import { Channel, invoke, isTauri } from "@tauri-apps/api/core";

import type { ProviderRunContext } from "./storage";

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
    audio: boolean;
    embeddings: boolean;
  };
};

/** One provider-neutral chat turn sent across the native boundary. */
export type ChatTurn = {
  role: "system" | "user" | "assistant";
  content: Array<{ type: "text"; text: string }>;
};

/** User-facing reasoning effort supported across provider routes. */
export type ReasoningEffort = "off" | "low";

/** Provider-qualified chat request accepted by the native inference command. */
export type ChatRequest = {
  providerId: string;
  modelId: string;
  messages: ChatTurn[];
  memoryEnabled?: boolean;
  webEnabled?: boolean;
  emailEnabled?: boolean;
  audioEnabled?: boolean;
  retainAudio?: boolean;
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
  costUsd: number | null;
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

/** Inference providers currently supported by the native shell. */
export type ProviderId = "omlx" | "ollama" | "openai" | "anthropic";

/** Fixed native web-search adapters available for explicit selection. */
export type WebSearchProviderId = "brave" | "exa";

/** Saved path-free restrictions applied by Rust to public Web destinations. */
export type WebNetworkPolicy = {
  httpsOnly: boolean;
  allowedDomains: string[];
  blockedDomains: string[];
};

/** Persisted non-secret provider configuration. */
export type ProviderSettings = {
  omlxBaseUrl: string;
  ollamaBaseUrl: string;
  openaiBaseUrl: string;
  anthropicBaseUrl: string;
  webSearchProviderId: WebSearchProviderId;
  webNetworkPolicy: WebNetworkPolicy;
  setupCompleted: boolean;
  lastProviderId: ProviderId | null;
  lastModelId: string | null;
  memoryEnabled: boolean;
  webEnabled: boolean;
  emailEnabled: boolean;
};

/** Stable native provider identities allowed to own an OS-vault credential. */
export type CredentialProviderId = "openai" | "anthropic" | WebSearchProviderId;

/** Secret-free availability for one native provider credential. */
export type ProviderCredentialStatus = {
  providerId: CredentialProviderId;
  configured: boolean;
  unlocked: boolean;
  biometricProtected: boolean;
};

/** Result of testing one draft provider endpoint. */
export type ProviderConnectionTest = {
  providerId: string;
  baseUrl: string;
  modelCount: number;
  elapsedMs: number;
  message: string;
};

/** Result of testing the fixed native web-search provider route. */
export type WebSearchConnectionTest = {
  providerId: WebSearchProviderId;
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

/** Result of one native redacted-diagnostics Save-dialog interaction. */
export type DiagnosticsExportOutcome = {
  status: "saved" | "cancelled";
  fileName: string | null;
};

/** Produces the normalized failure returned when native commands are unavailable. */
function unavailableInBrowser(): ProviderError {
  return {
    code: "unavailable",
    message: "Native inference is unavailable in the browser preview. Open the Tauri app to use a provider.",
    retryable: false,
  };
}

/** Discovers provider-qualified models through the native boundary. */
export async function discoverModels(providerId?: ProviderId): Promise<ModelInfo[]> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ModelInfo[]>("discover_models", { providerId: providerId ?? null });
}

/** Reads persisted provider settings from the native application. */
export async function getProviderSettings(): Promise<ProviderSettings> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ProviderSettings>("get_provider_settings");
}

/** Validates and persists provider settings through the native application. */
export async function updateProviderSettings(settings: ProviderSettings): Promise<ProviderSettings> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ProviderSettings>("update_provider_settings", { settings });
}

/** Persists the last successfully selected provider and model pair. */
export async function rememberProviderSelection(providerId: ProviderId, modelId: string): Promise<ProviderSettings> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ProviderSettings>("remember_provider_selection", {
    selection: { providerId, modelId },
  });
}

/** Persists completion of the native first-run provider and privacy disclosure. */
export async function completeFirstRunSetup(): Promise<ProviderSettings> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ProviderSettings>("complete_first_run_setup");
}

/** Tests a draft provider endpoint without changing active settings. */
export async function testProviderConnection(
  providerId: ProviderId,
  baseUrl: string,
  apiKey?: string,
): Promise<ProviderConnectionTest> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ProviderConnectionTest>("test_provider_connection", {
    draft: { providerId, baseUrl, apiKey: apiKey || null },
  });
}

/** Reads secret-free native-provider credential availability from the native vault. */
export async function getProviderCredentialStatus(): Promise<ProviderCredentialStatus[]> {
  if (!isTauri()) return [];
  return invoke<ProviderCredentialStatus[]>("get_provider_credential_status");
}

/** Stores, retains, or removes one native provider key without returning its value. */
export async function updateProviderCredential(
  providerId: CredentialProviderId,
  apiKey: string | null,
  remove = false,
): Promise<ProviderCredentialStatus> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ProviderCredentialStatus>("update_provider_credential", {
    update: { providerId, apiKey, remove },
  });
}

/** Tests one fixed search route with a draft or saved OS-vault credential. */
export async function testWebSearchConnection(
  providerId: WebSearchProviderId,
  apiKey?: string,
): Promise<WebSearchConnectionTest> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<WebSearchConnectionTest>("test_web_search_connection", {
    draft: { providerId, apiKey: apiKey || null },
  });
}

/** Reads the bounded, secret-redacted native diagnostic history. */
export async function getDiagnostics(): Promise<DiagnosticEntry[]> {
  if (!isTauri()) return [];
  return invoke<DiagnosticEntry[]>("get_diagnostics");
}

/** Saves the bounded current-session diagnostics without exposing the selected path. */
export async function exportDiagnostics(): Promise<DiagnosticsExportOutcome> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<DiagnosticsExportOutcome>("export_diagnostics");
}

/** Starts a provider-qualified native chat stream. */
export async function startChat(
  request: ChatRequest,
  context: ProviderRunContext,
  onEvent: (event: StreamEvent) => void,
): Promise<ChatRun> {
  if (!isTauri()) throw unavailableInBrowser();
  const channel = new Channel<StreamEvent>();
  channel.onmessage = onEvent;
  return invoke<ChatRun>("start_chat", { request, context, onEvent: channel });
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
