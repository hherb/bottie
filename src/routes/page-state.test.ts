import { describe, expect, it, vi } from "vitest";

import { modelKey } from "$lib/chat";
import type { ModelInfo } from "$lib/inference";

import { PageState } from "./page-state.svelte";

const LOCAL_MODEL: ModelInfo = {
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
    vision: false,
    embeddings: false,
  },
};

describe("PageState message submission", () => {
  it("guards duplicate submission before awaiting local playback shutdown", async () => {
    const state = new PageState();
    state.prompt = "Send this once";
    state.attachment.items = [];
    state.providerStatus = "available";
    state.models = [LOCAL_MODEL];
    state.selectedModelKey = modelKey(LOCAL_MODEL);
    state.speech.status = { phase: "speaking", selectedVoiceId: "local-voice-001", errorCode: null };

    let resolveStop = () => {};
    const stop = vi.spyOn(state.speech, "stop").mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          resolveStop = resolve;
        }),
    );
    const persist = vi.spyOn(state.history, "persistUserMessage").mockResolvedValue(null);

    const firstSubmission = state.sendMessage();
    expect(state.isPersistingMessage).toBe(true);
    const duplicateSubmission = state.sendMessage();
    expect(stop).toHaveBeenCalledTimes(1);

    resolveStop();
    await Promise.all([firstSubmission, duplicateSubmission]);

    expect(persist).toHaveBeenCalledOnce();
    expect(state.isPersistingMessage).toBe(false);
  });
});
