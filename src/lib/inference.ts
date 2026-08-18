import { Channel, invoke, isTauri } from "@tauri-apps/api/core";

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

export type ChatTurn = {
  role: "system" | "user" | "assistant";
  content: Array<{ type: "text"; text: string }>;
};

export type ChatRequest = {
  providerId: string;
  modelId: string;
  messages: ChatTurn[];
  settings?: {
    temperature?: number;
    maxOutputTokens?: number;
  };
};

export type Usage = {
  inputTokens: number | null;
  outputTokens: number | null;
};

export type ProviderError = {
  code: "unavailable" | "timeout" | "invalid_request" | "server" | "malformed_response" | "internal";
  message: string;
  retryable: boolean;
  diagnostic?: string;
};

export type StreamEvent =
  | { type: "started"; runId: string; providerId: string; modelId: string }
  | { type: "text_delta"; runId: string; delta: string }
  | { type: "usage_updated"; runId: string; usage: Usage }
  | { type: "completed"; runId: string; usage: Usage | null }
  | { type: "cancelled"; runId: string }
  | { type: "failed"; runId: string; error: ProviderError };

export type ChatRun = { runId: string };

function unavailableInBrowser(): ProviderError {
  return {
    code: "unavailable",
    message: "Native inference is unavailable in the browser preview. Open the Tauri app to use a local provider.",
    retryable: false,
  };
}

export async function discoverModels(): Promise<ModelInfo[]> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ModelInfo[]>("discover_models");
}

export async function startChat(
  request: ChatRequest,
  onEvent: (event: StreamEvent) => void,
): Promise<ChatRun> {
  if (!isTauri()) throw unavailableInBrowser();
  const channel = new Channel<StreamEvent>();
  channel.onmessage = onEvent;
  return invoke<ChatRun>("start_chat", { request, onEvent: channel });
}

export async function cancelChat(runId: string): Promise<boolean> {
  if (!isTauri()) return false;
  return invoke<boolean>("cancel_chat", { runId });
}

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
