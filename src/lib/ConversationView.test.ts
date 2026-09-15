import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import ConversationView from "./ConversationView.svelte";

describe("ConversationView", () => {
  it("renders a completed generated image from only opaque path-free metadata", () => {
    const html = render(ConversationView, {
      props: {
        messages: [
          {
            id: 1,
            storageId: "message-1",
            role: "assistant",
            content: "Generated image.",
            generatedAssets: [
              {
                id: "asset-1",
                ordinal: 0,
                status: "completed",
                mediaType: "image/png",
                width: 2_688,
                height: 1_536,
                byteSize: 4_096,
                providerId: "qwen-image",
                modelId: "qwen-image-2.0-2026-03-03",
                execution: "cloud",
                seed: null,
                errorCode: null,
                createdAtMs: 1,
                sources: [],
                previewUrl: "bottie-generated-asset://asset-1",
              },
            ],
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
        speechAvailable: false,
        speechVoices: [],
        speechStatus: {
          phase: "idle",
          selectedVoiceId: null,
          errorCode: null,
          latency: { playbackAcceptedMs: null },
        },
        speakingMessageId: null,
        microphoneCapturing: false,
        imageEditing: { active: true, execution: "cloud", selectedSourceIds: [], canSelectSource: true },
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

    expect(html).toContain('aria-label="Generated images"');
    expect(html).toContain('src="bottie-generated-asset://asset-1"');
    expect(html).toContain('width="2688" height="1536"');
    expect(html).toContain("qwen-image-2.0-2026-03-03");
    expect(html).toContain("2688×1536");
    expect(html).toContain('aria-label="Open generated image 1"');
    expect(html).toContain('aria-label="Copy generated image 1"');
    expect(html).toContain('aria-label="Export generated image 1"');
    expect(html).toContain('aria-label="Delete generated image 1"');
    expect(html).toContain('aria-label="Use generated image 1 as a reference"');
    expect(html).toContain('aria-pressed="false"');
    expect(html).not.toContain('aria-label="Regenerate response"');
  });

  it("marks an already selected generated reference as removable", () => {
    const html = render(ConversationView, {
      props: {
        messages: [
          {
            id: 1,
            role: "assistant",
            content: "Generated image.",
            generatedAssets: [
              {
                id: "asset-1",
                ordinal: 0,
                status: "completed",
                mediaType: "image/png",
                width: 64,
                height: 64,
                byteSize: 4_096,
                providerId: "qwen-image",
                modelId: "qwen-image-2.0-2026-03-03",
                execution: "cloud",
                seed: null,
                errorCode: null,
                createdAtMs: 1,
                sources: [],
                previewUrl: "bottie-generated-asset://asset-1",
              },
            ],
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
        speechAvailable: false,
        speechVoices: [],
        speechStatus: { phase: "idle", selectedVoiceId: null, errorCode: null, latency: { playbackAcceptedMs: null } },
        speakingMessageId: null,
        microphoneCapturing: false,
        imageEditing: {
          active: true,
          execution: "cloud",
          selectedSourceIds: ["asset-1"],
          canSelectSource: false,
        },
        onretry: vi.fn(),
        onselectbranch: vi.fn(),
        oneditmessage: vi.fn(),
        onregenerate: vi.fn(),
        onretryresponse: vi.fn(),
        ontoggleimagesource: vi.fn(),
        onrateresponse: vi.fn(),
        onremoveattachment: vi.fn(),
        onspeakresponse: vi.fn(),
        onstopspeech: vi.fn(),
        onscrollready: vi.fn(),
      },
    }).body;

    expect(html).toContain('aria-label="Remove generated image 1 from references"');
    expect(html).toContain('aria-pressed="true"');
    expect(html).not.toMatch(/aria-label="Remove generated image 1 from references"[^>]*disabled/);
  });

  it("offers exact retry only for a durable terminal generated-image request", () => {
    const html = render(ConversationView, {
      props: {
        messages: [
          {
            id: 1,
            storageId: "message-1",
            role: "assistant",
            content: "Image generation failed.",
            generatedAssets: [
              {
                id: "asset-1",
                ordinal: 0,
                status: "failed",
                mediaType: null,
                width: null,
                height: null,
                byteSize: null,
                providerId: "qwen-image",
                modelId: "qwen-image-2.0-2026-03-03",
                execution: "cloud",
                seed: null,
                errorCode: "provider_failed",
                createdAtMs: 1,
                sources: [],
                previewUrl: null,
              },
            ],
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
        speechAvailable: false,
        speechVoices: [],
        speechStatus: {
          phase: "idle",
          selectedVoiceId: null,
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

    expect(html).toContain('aria-label="Retry image generation"');
    expect(html).not.toContain('aria-label="Retry response"');
    expect(html).not.toContain('aria-label="Copy generated image 1"');
  });
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
