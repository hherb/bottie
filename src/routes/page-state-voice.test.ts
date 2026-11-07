import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const inference = vi.hoisted(() => ({ startChat: vi.fn() }));

vi.mock("$lib/inference", async (importOriginal) => ({
  ...(await importOriginal<typeof import("$lib/inference")>()),
  startChat: inference.startChat,
}));
vi.mock("$lib/microphone", async (importOriginal) => ({
  ...(await importOriginal<typeof import("$lib/microphone")>()),
  getMicrophoneStatus: vi.fn(),
}));
vi.mock("$lib/voice-conversation", async (importOriginal) => ({
  ...(await importOriginal<typeof import("$lib/voice-conversation")>()),
  startVoiceConversation: vi.fn(),
  resumeVoiceListening: vi.fn(),
  armVoiceReply: vi.fn(),
  interruptVoiceTurn: vi.fn(),
  endVoiceConversation: vi.fn(),
  getVoiceConversationStatus: vi.fn(),
}));

import { modelKey } from "$lib/chat";
import type { ModelInfo } from "$lib/inference";
import { getMicrophoneStatus, type MicrophoneStatus } from "$lib/microphone";
import {
  armVoiceReply,
  endVoiceConversation,
  getVoiceConversationStatus,
  INITIAL_VOICE_CONVERSATION_STATUS,
  resumeVoiceListening,
  startVoiceConversation,
  type VoiceConversationStatus,
} from "$lib/voice-conversation";
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

const IDLE_MICROPHONE: MicrophoneStatus = {
  phase: "idle",
  permission: "prompt",
  durationMs: 0,
  maxDurationMs: 60_000,
  sampleRateHz: null,
  channels: null,
  retainedByteSize: 0,
  inputLevel: 0,
  voiceActivity: null,
  voiceSegments: [],
  transcriptionPhase: "idle",
  transcriptSegments: [],
  transcriptionErrorCode: null,
  errorCode: null,
  latency: { inputReadyMs: null, firstTranscriptMs: null, finalTranscriptMs: null },
};

const RECORDING_MICROPHONE: MicrophoneStatus = {
  ...IDLE_MICROPHONE,
  phase: "recording",
  permission: "granted",
  transcriptionPhase: "listening",
  inputLevel: 0.02,
  voiceActivity: "silence",
};

const CAPTURED_TRANSCRIPT: MicrophoneStatus = {
  ...IDLE_MICROPHONE,
  phase: "captured",
  permission: "granted",
  durationMs: 2_400,
  transcriptionPhase: "ready",
  transcriptSegments: [
    { text: "Draft the release note locally.", startMs: 0, endMs: 1_200, isFinal: true, isCorrected: false },
    { text: "Keep it short.", startMs: 1_300, endMs: 2_100, isFinal: true, isCorrected: false },
  ],
};

const TRANSCRIPT = "Draft the release note locally.\nKeep it short.";

function status(patch: Partial<VoiceConversationStatus> = {}): VoiceConversationStatus {
  return { ...INITIAL_VOICE_CONVERSATION_STATUS, active: true, phase: "listening", ...patch };
}

const LISTENING = status();
const THINKING = status({ phase: "thinking" });
const FINALIZING = status({ phase: "finalizing" });
const INACTIVE = { ...INITIAL_VOICE_CONVERSATION_STATUS };

const runtime = globalThis as typeof globalThis & { isTauri?: boolean };
const originalIsTauri = runtime.isTauri;

describe("PageState voice conversation turns", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    runtime.isTauri = true;
    inference.startChat.mockResolvedValue({ runId: "voice-run" });
    vi.mocked(getMicrophoneStatus).mockResolvedValue({ ...RECORDING_MICROPHONE });
    vi.mocked(startVoiceConversation).mockResolvedValue(LISTENING);
    vi.mocked(getVoiceConversationStatus).mockResolvedValue(LISTENING);
    vi.mocked(armVoiceReply).mockResolvedValue(THINKING);
    vi.mocked(resumeVoiceListening).mockResolvedValue(LISTENING);
    vi.mocked(endVoiceConversation).mockResolvedValue(INACTIVE);
  });

  afterEach(() => {
    if (originalIsTauri === undefined) Reflect.deleteProperty(runtime, "isTauri");
    else runtime.isTauri = originalIsTauri;
  });

  /** Builds a page whose selected local model can answer, with native voice wrappers mocked. */
  function readyPage(): PageState {
    const state = new PageState();
    state.providerStatus = "available";
    state.models = [LOCAL_MODEL];
    state.selectedModelKey = modelKey(LOCAL_MODEL);
    state.microphone.status = { ...IDLE_MICROPHONE };
    return state;
  }

  it("starts one conversation over the selected input", async () => {
    vi.mocked(startVoiceConversation).mockResolvedValue(LISTENING);
    const state = readyPage();

    await state.startVoiceConversation();

    expect(startVoiceConversation).toHaveBeenCalledOnce();
    expect(state.voice.status.active).toBe(true);
    expect(state.voice.status.phase).toBe("listening");
    state.dispose();
  });

  it("submits a finished capture through the ordinary send path exactly once", async () => {
    vi.mocked(startVoiceConversation).mockResolvedValue(LISTENING);
    vi.mocked(armVoiceReply).mockResolvedValue(THINKING);
    vi.mocked(getVoiceConversationStatus).mockResolvedValue(LISTENING);
    const state = readyPage();
    const persist = vi
      .spyOn(state.history, "persistUserMessage")
      .mockResolvedValue({ conversationId: "conversation", requestMessageId: "request" });

    await state.startVoiceConversation();
    vi.mocked(getMicrophoneStatus).mockResolvedValue({ ...CAPTURED_TRANSCRIPT });
    await state.voice.refresh();

    await vi.waitFor(() => expect(armVoiceReply).toHaveBeenCalledOnce());
    await vi.waitFor(() => expect(persist).toHaveBeenCalledOnce());
    expect(persist).toHaveBeenCalledWith(TRANSCRIPT, []);
    await vi.waitFor(() => expect(inference.startChat).toHaveBeenCalledOnce());
    expect(state.prompt).toBe("");
    expect(state.isGenerating).toBe(true);

    // A later status for the same submitted turn must never submit a second time.
    state.isGenerating = false;
    await state.voice.refresh();
    await vi.waitFor(() => expect(resumeVoiceListening).toHaveBeenCalledOnce());
    expect(armVoiceReply).toHaveBeenCalledOnce();
    expect(persist).toHaveBeenCalledOnce();
    state.dispose();
  });

  it("never sends a turn whose session the user ended while its reply was being armed", async () => {
    let finishArm!: (next: VoiceConversationStatus) => void;
    vi.mocked(armVoiceReply).mockReturnValue(new Promise((resolve) => (finishArm = resolve)));
    const state = readyPage();
    const persist = vi.spyOn(state.history, "persistUserMessage");

    await state.startVoiceConversation();
    vi.mocked(getMicrophoneStatus).mockResolvedValue({ ...CAPTURED_TRANSCRIPT });
    await state.voice.refresh();
    await vi.waitFor(() => expect(armVoiceReply).toHaveBeenCalledOnce());

    const ending = state.endVoiceConversation();
    finishArm(THINKING);
    await ending;

    expect(endVoiceConversation).toHaveBeenCalledOnce();
    expect(state.voice.status.active).toBe(false);
    expect(persist).not.toHaveBeenCalled();
    expect(inference.startChat).not.toHaveBeenCalled();
    state.dispose();
  });

  it("reopens listening once when a turn closed with nothing heard", async () => {
    const state = readyPage();

    await state.startVoiceConversation();
    vi.mocked(getMicrophoneStatus).mockResolvedValue({ ...CAPTURED_TRANSCRIPT, transcriptSegments: [] });
    await state.voice.refresh();

    await vi.waitFor(() => expect(resumeVoiceListening).toHaveBeenCalledOnce());
    expect(armVoiceReply).not.toHaveBeenCalled();
    // The same settled capture must not reopen twice before native input starts again.
    await state.voice.refresh();
    expect(resumeVoiceListening).toHaveBeenCalledOnce();
    expect(state.voice.status.active).toBe(true);
    state.dispose();
  });

  it("ends a session whose microphone failed and says why", async () => {
    const state = readyPage();

    await state.startVoiceConversation();
    vi.mocked(getMicrophoneStatus).mockResolvedValue({
      ...IDLE_MICROPHONE,
      phase: "error",
      permission: "denied",
      errorCode: "permission_denied",
    });
    await state.voice.refresh();

    await vi.waitFor(() => expect(endVoiceConversation).toHaveBeenCalledOnce());
    expect(resumeVoiceListening).not.toHaveBeenCalled();
    expect(state.voiceFeedback).toBe(
      "Microphone access was denied. Allow Bottie in system privacy settings, then try again.",
    );
    state.dispose();
  });

  it("keeps the transcript and ends the session when no model can answer", async () => {
    vi.mocked(startVoiceConversation).mockResolvedValue(LISTENING);
    vi.mocked(endVoiceConversation).mockResolvedValue(INACTIVE);
    const state = new PageState();
    state.microphone.status = { ...IDLE_MICROPHONE };

    await state.startVoiceConversation();
    vi.mocked(getMicrophoneStatus).mockResolvedValue({ ...CAPTURED_TRANSCRIPT });
    await state.voice.refresh();

    await vi.waitFor(() => expect(endVoiceConversation).toHaveBeenCalledOnce());
    expect(armVoiceReply).not.toHaveBeenCalled();
    expect(state.prompt).toContain(TRANSCRIPT);
    expect(state.voiceFeedback).toBe(
      "A voice turn needs a reachable provider and a selected model. The transcript stays in your draft.",
    );
    state.dispose();
  });

  it("keeps the transcript once and ends when the ordinary send path fails", async () => {
    const state = readyPage();
    vi.spyOn(state.history, "persistUserMessage").mockResolvedValue(null);

    await state.startVoiceConversation();
    vi.mocked(getMicrophoneStatus).mockResolvedValue({ ...CAPTURED_TRANSCRIPT });
    await state.voice.refresh();

    await vi.waitFor(() => expect(endVoiceConversation).toHaveBeenCalledOnce());
    expect(armVoiceReply).toHaveBeenCalledOnce();
    expect(state.prompt).toBe(TRANSCRIPT);
    expect(state.voiceFeedback).toBe("The voice turn was not sent. The transcript stays in your draft.");
    state.dispose();
  });

  it("does not submit while a generation is still running", async () => {
    vi.mocked(startVoiceConversation).mockResolvedValue(LISTENING);
    vi.mocked(getVoiceConversationStatus).mockResolvedValue(LISTENING);
    const state = readyPage();
    state.isGenerating = true;
    state.microphone.status = { ...CAPTURED_TRANSCRIPT };

    await state.startVoiceConversation();
    await state.voice.refresh();
    await Promise.resolve();

    expect(armVoiceReply).not.toHaveBeenCalled();
    state.dispose();
  });

  it("resumes listening only after the spoken reply finished", async () => {
    vi.mocked(startVoiceConversation).mockResolvedValue(status({ phase: "speaking" }));
    vi.mocked(getVoiceConversationStatus).mockResolvedValue(status({ phase: "speaking" }));
    vi.mocked(getMicrophoneStatus).mockResolvedValue({ ...IDLE_MICROPHONE });
    vi.mocked(resumeVoiceListening).mockResolvedValue(LISTENING);
    const state = readyPage();

    await state.startVoiceConversation();
    await state.voice.refresh();
    await Promise.resolve();
    expect(resumeVoiceListening).not.toHaveBeenCalled();

    state.isGenerating = false;
    vi.mocked(getVoiceConversationStatus).mockResolvedValue(FINALIZING);
    await state.voice.refresh();

    await vi.waitFor(() => expect(resumeVoiceListening).toHaveBeenCalledOnce());
    expect(state.voice.status.phase).toBe("listening");
    state.dispose();
  });

  it("keeps push-to-talk from taking the microphone while a conversation owns it", async () => {
    vi.mocked(startVoiceConversation).mockResolvedValue(LISTENING);
    const state = readyPage();

    await state.startVoiceConversation();
    await state.startMicrophoneCapture();

    expect(state.microphone.status.phase).toBe("recording");
    state.dispose();
  });
});
