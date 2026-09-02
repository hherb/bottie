import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import ConversationView from "./ConversationView.svelte";

describe("ConversationView", () => {
  it("labels a durable failed response without replacing its stable content or retry action", () => {
    const html = render(ConversationView, {
      props: {
        messages: [
          {
            id: 1,
            storageId: "response-1",
            role: "assistant",
            content: "Generation failed before any response was saved.",
            meta: "Generation failed · saved partial response",
            error: true,
            retryable: true,
          },
        ],
        providerStatus: "available",
        providerError: null,
        selectedModel: undefined,
        activeStage: -1,
        inferenceStages: [],
        isGenerating: false,
        canGenerate: true,
        branches: [],
        currentBranchId: null,
        speechAvailable: true,
        speechVoices: [{ id: "voice.en-au", name: "Karen", language: "en-AU" }],
        speechStatus: {
          phase: "idle",
          selectedVoiceId: "voice.en-au",
          errorCode: null,
          latency: { playbackAcceptedMs: null },
        },
        speakingMessageId: null,
        microphoneCapturing: false,
        onretry: vi.fn(),
        onselectbranch: vi.fn(),
        oneditmessage: vi.fn(),
        onregenerate: vi.fn(),
        onretryresponse: vi.fn(),
        onrateresponse: vi.fn(),
        onremoveattachment: vi.fn(),
        onspeakresponse: vi.fn(),
        onstopspeech: vi.fn(),
        onscrollready: vi.fn(),
      },
    }).body;

    expect(html).toContain('class="message-state error-state"');
    expect(html).toContain("Response needs attention");
    expect(html).toContain("Generation failed before any response was saved.");
    expect(html).toContain('aria-label="Retry response"');
    expect(html).not.toContain('aria-label="Local playback voice"');
    expect(html).toContain('aria-label="Play response aloud"');
  });

  it("renders an explicit stop action only for the response playing locally", () => {
    const html = render(ConversationView, {
      props: {
        messages: [
          { id: 1, storageId: "response-1", role: "assistant", content: "First response." },
          { id: 2, storageId: "response-2", role: "assistant", content: "Second response." },
        ],
        providerStatus: "available",
        providerError: null,
        selectedModel: undefined,
        activeStage: -1,
        inferenceStages: [],
        isGenerating: false,
        canGenerate: true,
        branches: [],
        currentBranchId: null,
        speechAvailable: true,
        speechVoices: [{ id: "voice.en-au", name: "Karen", language: "en-AU" }],
        speechStatus: {
          phase: "speaking",
          selectedVoiceId: "voice.en-au",
          errorCode: null,
          latency: { playbackAcceptedMs: 12 },
        },
        speakingMessageId: 2,
        microphoneCapturing: false,
        onretry: vi.fn(),
        onselectbranch: vi.fn(),
        oneditmessage: vi.fn(),
        onregenerate: vi.fn(),
        onretryresponse: vi.fn(),
        onrateresponse: vi.fn(),
        onremoveattachment: vi.fn(),
        onspeakresponse: vi.fn(),
        onstopspeech: vi.fn(),
        onscrollready: vi.fn(),
      },
    }).body;

    expect(html.match(/aria-label="Play response aloud"/g)).toHaveLength(1);
    expect(html.match(/aria-label="Stop local playback"/g)).toHaveLength(1);
    expect(html).toContain("Playing locally · engine accepted playback in 12 ms · use Stop to end playback");
  });
});
