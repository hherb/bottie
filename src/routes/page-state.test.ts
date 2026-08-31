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
    audio: false,
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
    state.speech.status = {
      phase: "speaking",
      selectedVoiceId: "local-voice-001",
      errorCode: null,
      latency: { playbackAcceptedMs: 12 },
    };

    let resolveStop = () => {};
    const stop = vi.spyOn(state.speech, "stop").mockImplementation(
      () =>
        new Promise<boolean>((resolve) => {
          resolveStop = () => resolve(true);
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

describe("PageState voice barge-in", () => {
  it("requests generation cancellation before awaiting playback shutdown and capture", async () => {
    const state = new PageState();
    state.isGenerating = true;
    state.speech.status = {
      phase: "speaking",
      selectedVoiceId: "local-voice-001",
      errorCode: null,
      latency: { playbackAcceptedMs: 12 },
    };

    const order: string[] = [];
    const cancel = vi.spyOn(state, "stopGenerating").mockImplementation(() => {
      order.push("cancel");
    });
    let resolveStop = () => {};
    vi.spyOn(state.speech, "stop").mockImplementation(
      () =>
        new Promise<boolean>((resolve) => {
          order.push("stop_playback");
          resolveStop = () => resolve(true);
        }),
    );
    const capture = vi.spyOn(state.microphone, "start").mockImplementation(async () => {
      order.push("start_capture");
    });

    const bargeIn = state.startMicrophoneCapture();

    expect(cancel).toHaveBeenCalledOnce();
    expect(capture).not.toHaveBeenCalled();
    resolveStop();
    await bargeIn;

    expect(order).toEqual(["cancel", "stop_playback", "start_capture"]);
  });

  it("keeps capture fail-closed when local playback cannot be stopped", async () => {
    const state = new PageState();
    state.speech.status = {
      phase: "speaking",
      selectedVoiceId: "local-voice-001",
      errorCode: null,
      latency: { playbackAcceptedMs: 12 },
    };
    vi.spyOn(state.speech, "stop").mockResolvedValue(false);
    const capture = vi.spyOn(state.microphone, "start").mockResolvedValue();

    await state.startMicrophoneCapture();

    expect(capture).not.toHaveBeenCalled();
  });
});

describe("PageState captured-audio choices", () => {
  it("allows an incompatible route change to turn delivery back off", () => {
    const state = new PageState();
    state.microphone.status = { ...state.microphone.status, phase: "captured" };

    state.microphone.toggleSendAudio(true);
    expect(state.microphone.sendAudio).toBe(true);

    state.microphone.toggleSendAudio(false);
    expect(state.microphone.sendAudio).toBe(false);
    state.microphone.toggleSendAudio(false);
    expect(state.microphone.sendAudio).toBe(false);
  });
});
