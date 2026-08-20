import { describe, expect, it } from "vitest";

import {
  chatTurnsForMessages,
  completionMeta,
  conversationTitle,
  displayEndpoint,
  filterUsableModels,
  formatBytes,
  isCloudProvider,
  modelKey,
  nextResponseRating,
  persistedCompletionMeta,
  persistedMessagePresentation,
  requestMessageForResponse,
  resolveModelSelection,
  toggleReasoningEffort,
} from "./chat";
import type { ModelInfo, Usage } from "./inference";

const ollamaModel: ModelInfo = {
  providerId: "ollama",
  providerName: "Ollama",
  modelId: "gemma3:4b",
  displayName: "gemma3:4b",
  maxContextTokens: 131_072,
  loadState: "loaded",
  capabilities: {
    text: true,
    streaming: true,
    tools: false,
    vision: true,
    embeddings: false,
  },
};

const omlxModel: ModelInfo = {
  ...ollamaModel,
  providerId: "omlx",
  providerName: "oMLX",
  modelId: "Qwen3.6-35B-A3B-8bit",
  displayName: "Qwen3.6-35B-A3B-8bit",
};

describe("chat presentation helpers", () => {
  it("builds collision-safe provider-qualified model keys", () => {
    expect(modelKey(ollamaModel)).toBe("ollama:gemma3:4b");
  });

  it("derives a bounded single-line title from the first prompt", () => {
    expect(conversationTitle("  A durable\n\nconversation   title  ")).toBe("A durable conversation title");
    expect(conversationTitle("x".repeat(100))).toHaveLength(80);
  });

  it("formats loopback endpoints and binary file sizes", () => {
    expect(displayEndpoint("http://127.0.0.1:11434/")).toBe("127.0.0.1:11434");
    expect(formatBytes(1_023)).toBe("1023 B");
    expect(formatBytes(1_024)).toBe("1 KB");
    expect(formatBytes(1_572_864)).toBe("1.5 MB");
  });

  it("does not mislabel an empty native-preview selection as a cloud route", () => {
    expect(isCloudProvider("")).toBe(false);
    expect(isCloudProvider("ollama")).toBe(false);
    expect(isCloudProvider("openai")).toBe(true);
    expect(isCloudProvider("anthropic")).toBe(true);
  });

  it("reports elapsed time and optional output usage", () => {
    const usage: Usage = { inputTokens: 12, outputTokens: 7, costUsd: null };
    expect(completionMeta(1_000, 2_250, usage)).toBe("1.3s · 7 tokens");
    expect(completionMeta(1_000, 2_250, null)).toBe("1.3s · usage unavailable");
  });

  it("reconstructs completion metadata from durable provider-run timestamps", () => {
    const usage: Usage = { inputTokens: 18, outputTokens: 9, costUsd: 0.0025 };
    expect(persistedCompletionMeta(10_000, 11_250, usage)).toBe("1.3s · 9 tokens · $0.0025");
    expect(persistedCompletionMeta(10_000, null, usage)).toBeUndefined();
  });

  it("labels recovered interruption and terminal partial-response states", () => {
    expect(persistedMessagePresentation("partial", "interrupted", true)).toEqual({
      fallbackText: undefined,
      meta: "Interrupted · saved partial response",
      error: true,
      retryable: true,
    });
    expect(persistedMessagePresentation("partial", "interrupted", false).fallbackText).toBe(
      "Generation interrupted before any response was saved.",
    );
    expect(persistedMessagePresentation("cancelled", null, false).fallbackText).toBe("Generation stopped.");
    expect(persistedMessagePresentation("failed", "timeout", false).meta).toBe(
      "Generation failed · saved partial response",
    );
    expect(persistedMessagePresentation("cancelled", null, true).retryable).toBe(true);
    expect(persistedMessagePresentation("failed", "server", true).retryable).toBe(true);
    expect(persistedMessagePresentation("failed", "invalid_request", true).retryable).toBe(false);
    expect(persistedMessagePresentation("failed", "internal", true).retryable).toBe(false);
    expect(persistedMessagePresentation("final", null, true).retryable).toBe(false);
  });

  it("finds the persisted user request immediately preceding a response", () => {
    const messages = [
      { id: 1, storageId: "request", role: "user" as const, content: "Question" },
      { id: 2, storageId: "response", role: "assistant" as const, content: "Answer" },
    ];

    expect(requestMessageForResponse(messages, 2)).toEqual(messages[0]);
    expect(requestMessageForResponse(messages, 1)).toBeUndefined();
    expect(requestMessageForResponse(messages, 99)).toBeUndefined();
  });

  it("selects a response rating and clears an already active choice", () => {
    expect(nextResponseRating(null, "good")).toBe("good");
    expect(nextResponseRating("poor", "good")).toBe("good");
    expect(nextResponseRating("good", "good")).toBeNull();
  });

  it("keeps only meaningful successful messages in provider context", () => {
    expect(
      chatTurnsForMessages([
        { id: 1, role: "user", content: "Question" },
        { id: 2, role: "assistant", content: "Partial failure", error: true, retryable: true },
        { id: 3, role: "assistant", content: "   " },
        { id: 4, role: "assistant", content: "Answer" },
      ]),
    ).toEqual([
      { role: "user", content: [{ type: "text", text: "Question" }] },
      { role: "assistant", content: [{ type: "text", text: "Answer" }] },
    ]);
  });

  it("keeps only streaming text models", () => {
    const embeddingModel: ModelInfo = {
      ...ollamaModel,
      modelId: "nomic-embed-text",
      capabilities: { ...ollamaModel.capabilities, text: false, streaming: false, embeddings: true },
    };
    expect(filterUsableModels([ollamaModel, embeddingModel])).toEqual([ollamaModel]);
  });

  it("prefers a remembered provider and model when both remain available", () => {
    expect(resolveModelSelection([ollamaModel, omlxModel], "", "omlx", omlxModel.modelId)).toEqual({
      providerId: "omlx",
      models: [omlxModel],
      selectedModelKey: modelKey(omlxModel),
    });
  });

  it("falls back deterministically when remembered selection is unavailable", () => {
    expect(resolveModelSelection([ollamaModel, omlxModel], "ollama", "omlx", "missing")).toEqual({
      providerId: "ollama",
      models: [ollamaModel],
      selectedModelKey: modelKey(ollamaModel),
    });
  });

  it("toggles reasoning between the safe off and low-effort states", () => {
    expect(toggleReasoningEffort("off")).toBe("low");
    expect(toggleReasoningEffort("low")).toBe("off");
  });
});
