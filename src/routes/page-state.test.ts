import { describe, expect, it, vi } from "vitest";

import { modelKey } from "$lib/chat";
import type { ModelInfo } from "$lib/inference";
import { MAX_COMPOSER_DRAFT_BYTES } from "$lib/microphone";

import { PageState } from "./page-state.svelte";

const imageInference = vi.hoisted(() => ({
  startImageEditing: vi.fn(),
  validateQwenImageConfiguration: vi.fn(),
}));

vi.mock("$lib/inference", async (importOriginal) => ({
  ...(await importOriginal<typeof import("$lib/inference")>()),
  ...imageInference,
}));

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

describe("PageState image execution", () => {
  it("skips Cloud credential validation before persisting a ready local request", async () => {
    const tauriRuntime = globalThis as typeof globalThis & { isTauri?: boolean };
    const previousIsTauri = tauriRuntime.isTauri;
    tauriRuntime.isTauri = true;

    try {
      const state = new PageState();
      state.imageMode = true;
      state.imageExecution = "local";
      state.localImageAvailability = {
        modelId: "Qwen/Qwen-Image-2512",
        packageId: "qwen-image-2512",
        runtimeId: "local-image-worker",
        license: "Apache-2.0",
        sourceRevision: "review-fixture",
        expectedDiskBytes: 1,
        workerExpectedDiskBytes: 1,
        requiredMemoryBytes: 1,
        availability: "ready",
      };
      state.prompt = "Draw Bottie locally";
      const persist = vi.spyOn(state.history, "persistUserMessage").mockResolvedValue(null);

      await state.generateImage();

      expect(persist).toHaveBeenCalledWith("Draw Bottie locally", []);
      expect(state.imageFeedback).toBe("The image prompt could not be saved.");
    } finally {
      if (previousIsTauri === undefined) Reflect.deleteProperty(tauriRuntime, "isTauri");
      else tauriRuntime.isTauri = previousIsTauri;
    }
  });

  it("persists ready attachment IDs and invokes hosted editing in visible order", async () => {
    const tauriRuntime = globalThis as typeof globalThis & { isTauri?: boolean };
    const previousIsTauri = tauriRuntime.isTauri;
    tauriRuntime.isTauri = true;
    imageInference.validateQwenImageConfiguration.mockResolvedValue({});
    imageInference.startImageEditing.mockResolvedValue({
      runId: "edit-run",
      message: pendingAssistantMessage("assistant-edit"),
    });

    try {
      const state = new PageState();
      state.imageMode = true;
      state.imageExecution = "cloud";
      state.imageSize = "landscape";
      state.imageCount = 2;
      state.prompt = "Make these images nocturnal";
      state.attachment.items = [readyImage("source-b"), readyImage("source-a")];
      const persist = vi.spyOn(state.history, "persistUserMessage").mockResolvedValue({
        conversationId: "conversation",
        requestMessageId: "request",
      });

      await state.generateImage();

      expect(persist).toHaveBeenCalledWith("Make these images nocturnal", ["source-b", "source-a"]);
      expect(imageInference.startImageEditing).toHaveBeenCalledWith(
        {
          conversationId: "conversation",
          requestMessageId: "request",
          prompt: "Make these images nocturnal",
          width: 2_688,
          height: 1_536,
          count: 2,
          sources: [
            { sourceType: "attachment", sourceId: "source-b" },
            { sourceType: "attachment", sourceId: "source-a" },
          ],
        },
        expect.any(Function),
      );
      expect(state.attachment.items).toEqual([]);
      expect(
        state.messages.find((message) => message.storageId === "request")?.attachments?.map(({ id }) => id),
      ).toEqual(["source-b", "source-a"]);
    } finally {
      imageInference.startImageEditing.mockReset();
      imageInference.validateQwenImageConfiguration.mockReset();
      if (previousIsTauri === undefined) Reflect.deleteProperty(tauriRuntime, "isTauri");
      else tauriRuntime.isTauri = previousIsTauri;
    }
  });

  it("invokes hosted editing with a selected completed ancestor without attaching it to the request", async () => {
    const tauriRuntime = globalThis as typeof globalThis & { isTauri?: boolean };
    const previousIsTauri = tauriRuntime.isTauri;
    tauriRuntime.isTauri = true;
    imageInference.validateQwenImageConfiguration.mockResolvedValue({});
    imageInference.startImageEditing.mockResolvedValue({
      runId: "edit-run",
      message: pendingAssistantMessage("assistant-edit"),
    });

    try {
      const state = new PageState();
      state.imageMode = true;
      state.imageExecution = "cloud";
      state.prompt = "Use the prior composition";
      state.attachment.items = [];
      state.messages = [completedGeneratedMessage("generated-source")];
      state.toggleGeneratedImageSource("generated-source");
      const persist = vi.spyOn(state.history, "persistUserMessage").mockResolvedValue({
        conversationId: "conversation",
        requestMessageId: "request",
      });

      await state.generateImage();

      expect(persist).toHaveBeenCalledWith("Use the prior composition", []);
      expect(imageInference.startImageEditing).toHaveBeenCalledWith(
        expect.objectContaining({
          sources: [{ sourceType: "generated_asset", sourceId: "generated-source" }],
        }),
        expect.any(Function),
      );
      expect(state.selectedGeneratedImageSources).toEqual([]);
    } finally {
      imageInference.startImageEditing.mockReset();
      imageInference.validateQwenImageConfiguration.mockReset();
      if (previousIsTauri === undefined) Reflect.deleteProperty(tauriRuntime, "isTauri");
      else tauriRuntime.isTauri = previousIsTauri;
    }
  });
});

/** Builds one ready normalized image attachment without exposing native bytes or paths. */
function readyImage(id: string): import("$lib/presentation").Attachment {
  return {
    id,
    name: `${id}.png`,
    size: "4 KB",
    kind: "image",
    mimeType: "image/png",
    previewUrl: null,
    extraction: {
      state: "unsupported",
      format: null,
      characterCount: null,
      pageCount: null,
      errorCode: null,
    },
    indexing: { state: "unsupported" },
    normalization: { state: "ready", format: "png", width: 64, height: 64, byteSize: 4_096, errorCode: null },
  };
}

/** Builds one pending assistant record returned by accepted native image commands. */
function pendingAssistantMessage(id: string): import("$lib/storage").StoredMessage {
  return {
    id,
    role: "assistant",
    text: "",
    reasoning: null,
    state: "partial",
    providerId: "qwen-image",
    modelId: "qwen-image-2.0-2026-03-03",
    providerRun: null,
    rating: null,
    attachments: [],
    generatedAssets: [],
    createdAtMs: 1,
  };
}

/** Builds one visible completed generated image eligible for exact ancestry revalidation. */
function completedGeneratedMessage(assetId: string): import("$lib/presentation").Message {
  return {
    id: 1,
    storageId: "prior-assistant",
    role: "assistant",
    content: "Generated image.",
    generatedAssets: [
      {
        id: assetId,
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
        previewUrl: "bottie-generated-asset://generated-source",
      },
    ],
  };
}

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

describe("PageState transcript text fallback", () => {
  it("keeps a copied transcript editable when no provider or model is available", () => {
    const state = new PageState();
    state.providerStatus = "offline";
    state.models = [];
    state.selectedModelKey = "";
    state.microphone.status = {
      ...state.microphone.status,
      phase: "captured",
      transcriptionPhase: "ready",
      transcriptSegments: [{ text: "Offline draft", startMs: 0, endMs: 800, isFinal: true, isCorrected: false }],
    };

    expect(state.canSend).toBe(false);
    expect(state.canCompose).toBe(true);

    state.useMicrophoneTranscriptAsText();

    expect(state.prompt).toBe("Offline draft");
    expect(state.canCompose).toBe(true);
  });

  it("keeps an existing offline draft editable but does not open an empty unavailable composer", () => {
    const state = new PageState();
    state.providerStatus = "offline";

    expect(state.canCompose).toBe(false);
    state.prompt = "Keep editing locally";
    expect(state.canCompose).toBe(true);
  });

  it("does not let an existing draft bypass the native persistence lock", () => {
    const state = new PageState();
    state.prompt = "Submission in progress";
    state.isPersistingMessage = true;

    expect(state.canCompose).toBe(false);
  });

  it("appends the current corrected transcript without consuming or submitting capture state", () => {
    const state = new PageState();
    state.prompt = "Existing draft";
    state.microphone.status = {
      ...state.microphone.status,
      phase: "captured",
      retainedByteSize: 48_000,
      transcriptionPhase: "ready",
      transcriptSegments: [
        { text: "First turn", startMs: 0, endMs: 800, isFinal: true, isCorrected: false },
        { text: "Corrected turn", startMs: 900, endMs: 1_600, isFinal: true, isCorrected: true },
      ],
    };
    const retainedStatus = state.microphone.status;
    const focus = vi.spyOn(state.interaction, "focusDraftAfterUpdate").mockResolvedValue();

    state.useMicrophoneTranscriptAsText();

    expect(state.prompt).toBe("Existing draft\n\nFirst turn\nCorrected turn");
    expect(state.microphone.status).toBe(retainedStatus);
    expect(state.microphone.status.retainedByteSize).toBe(48_000);
    expect(state.microphoneTranscriptDraftFeedback).toBe(
      "Transcript appended to the editable draft. Nothing was sent.",
    );
    expect(state.microphoneTranscriptDraftError).toBe(false);
    expect(focus).toHaveBeenCalledOnce();
  });

  it("fails visibly without changing an over-limit draft or retained capture", () => {
    const state = new PageState();
    state.prompt = "é".repeat(MAX_COMPOSER_DRAFT_BYTES / 2);
    state.microphone.status = {
      ...state.microphone.status,
      phase: "captured",
      retainedByteSize: 48_000,
      transcriptionPhase: "ready",
      transcriptSegments: [{ text: "One more turn", startMs: 0, endMs: 800, isFinal: true, isCorrected: false }],
    };
    const originalDraft = state.prompt;
    const retainedStatus = state.microphone.status;

    state.useMicrophoneTranscriptAsText();

    expect(state.prompt).toBe(originalDraft);
    expect(state.microphone.status).toBe(retainedStatus);
    expect(state.microphoneTranscriptDraftError).toBe(true);
    expect(state.microphoneTranscriptDraftFeedback).toContain("combined draft exceeds the 32 KiB text limit");
  });
});
