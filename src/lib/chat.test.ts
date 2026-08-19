import { describe, expect, it } from "vitest";

import {
  completionMeta,
  conversationTitle,
  displayEndpoint,
  filterUsableModels,
  formatBytes,
  isCloudProvider,
  modelKey,
  persistedCompletionMeta,
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
