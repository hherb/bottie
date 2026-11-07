import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true, invoke: vi.fn() }));
vi.mock("$lib/voice-conversation", async (importOriginal) => ({
  ...(await importOriginal<typeof import("$lib/voice-conversation")>()),
  startVoiceConversation: vi.fn(),
  resumeVoiceListening: vi.fn(),
  armVoiceReply: vi.fn(),
  interruptVoiceTurn: vi.fn(),
  endVoiceConversation: vi.fn(),
  getVoiceConversationStatus: vi.fn(),
}));

import {
  armVoiceReply,
  endVoiceConversation,
  getVoiceConversationStatus,
  INITIAL_VOICE_CONVERSATION_STATUS,
  interruptVoiceTurn,
  resumeVoiceListening,
  startVoiceConversation,
  type VoiceConversationStatus,
} from "$lib/voice-conversation";
import { VoiceConversationState } from "./voice-conversation-state.svelte";

function status(patch: Partial<VoiceConversationStatus> = {}): VoiceConversationStatus {
  return { ...INITIAL_VOICE_CONVERSATION_STATUS, active: true, phase: "listening", ...patch };
}

const LISTENING = status();
const INACTIVE = { ...INITIAL_VOICE_CONVERSATION_STATUS };

describe("voice conversation state", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("starts one session and reports every applied status to its observer", async () => {
    vi.mocked(startVoiceConversation).mockResolvedValue(LISTENING);
    const observed: VoiceConversationStatus[] = [];
    const state = new VoiceConversationState((next) => observed.push(next));

    expect(state.available).toBe(true);
    await expect(state.start()).resolves.toBe(true);

    expect(startVoiceConversation).toHaveBeenCalledOnce();
    expect(state.status.active).toBe(true);
    expect(state.status.phase).toBe("listening");
    expect(state.starting).toBe(false);
    expect(state.error).toBeNull();
    expect(observed).toHaveLength(1);
    state.dispose();
  });

  it("shows one bounded sentence for a rejected command, never the raw value", async () => {
    vi.mocked(startVoiceConversation).mockRejectedValue("speech_unavailable");
    const state = new VoiceConversationState();

    await expect(state.start()).resolves.toBe(false);

    expect(state.error).toBe("Local speech is unavailable, so the spoken reply cannot start.");
    expect(state.status.active).toBe(false);
    expect(state.starting).toBe(false);
  });

  it("maps an unrecognized rejection without echoing native detail", async () => {
    vi.mocked(startVoiceConversation).mockRejectedValue(new Error("engine at /tmp/bottie-speech failed"));
    const state = new VoiceConversationState();

    await expect(state.start()).resolves.toBe(false);

    expect(state.error).toBe("The voice conversation request failed. Try again.");
  });

  it("treats a still-open capture as recoverable instead of an error", async () => {
    vi.mocked(startVoiceConversation).mockResolvedValue(LISTENING);
    vi.mocked(armVoiceReply).mockRejectedValue("capture_active");
    const state = new VoiceConversationState();

    await state.start();
    await expect(state.armReply()).resolves.toBe(false);

    expect(state.error).toBeNull();
    expect(state.status.active).toBe(true);
    state.dispose();
  });

  it("reports a real arm failure as bounded feedback", async () => {
    vi.mocked(startVoiceConversation).mockResolvedValue(LISTENING);
    vi.mocked(armVoiceReply).mockRejectedValue("speech_playback_failed");
    const state = new VoiceConversationState();

    await state.start();
    await expect(state.armReply()).resolves.toBe(false);

    expect(state.error).toBe("Local speech failed during the spoken reply.");
    state.dispose();
  });

  it("polls while active and stops once the session is gone", async () => {
    vi.mocked(startVoiceConversation).mockResolvedValue(LISTENING);
    vi.mocked(getVoiceConversationStatus).mockResolvedValue(INACTIVE);
    const state = new VoiceConversationState();

    await state.start();
    await vi.advanceTimersByTimeAsync(200);
    expect(getVoiceConversationStatus).toHaveBeenCalledOnce();
    expect(state.status.active).toBe(false);

    // No further polls once the native session reports inactive.
    await vi.advanceTimersByTimeAsync(600);
    expect(getVoiceConversationStatus).toHaveBeenCalledOnce();
  });

  it("discards a poll that finished after the user ended the session", async () => {
    vi.mocked(startVoiceConversation).mockResolvedValue(LISTENING);
    vi.mocked(endVoiceConversation).mockResolvedValue(INACTIVE);
    let finishPoll!: (value: VoiceConversationStatus) => void;
    vi.mocked(getVoiceConversationStatus).mockReturnValue(
      new Promise<VoiceConversationStatus>((resolve) => (finishPoll = resolve)),
    );
    const state = new VoiceConversationState();

    await state.start();
    await vi.advanceTimersByTimeAsync(200);
    expect(getVoiceConversationStatus).toHaveBeenCalledOnce();
    await expect(state.end()).resolves.toBe(true);
    finishPoll(LISTENING);
    await vi.advanceTimersByTimeAsync(0);

    expect(state.status.active).toBe(false);
    expect(state.status.phase).toBe("idle");
  });

  it("keeps polling an open session through a rejected poll and clears the error once it recovers", async () => {
    vi.mocked(startVoiceConversation).mockResolvedValue(LISTENING);
    vi.mocked(getVoiceConversationStatus).mockRejectedValueOnce("command_failed").mockResolvedValue(LISTENING);
    const state = new VoiceConversationState();

    await state.start();
    await vi.advanceTimersByTimeAsync(200);
    expect(state.error).toBe("The voice conversation request failed. Try again.");

    // Native capture is still open, so polling must continue rather than strand it.
    await vi.advanceTimersByTimeAsync(200);
    expect(getVoiceConversationStatus).toHaveBeenCalledTimes(2);
    expect(state.error).toBeNull();
    expect(state.status.active).toBe(true);
  });

  it("refuses to interrupt or resume without an active session", async () => {
    const state = new VoiceConversationState();

    await expect(state.interrupt()).resolves.toBe(false);
    await expect(state.resume()).resolves.toBe(false);
    await expect(state.armReply()).resolves.toBe(false);

    expect(interruptVoiceTurn).not.toHaveBeenCalled();
    expect(resumeVoiceListening).not.toHaveBeenCalled();
    expect(armVoiceReply).not.toHaveBeenCalled();
  });

  it("interrupts an active reply and keeps the session listening", async () => {
    vi.mocked(startVoiceConversation).mockResolvedValue(LISTENING);
    vi.mocked(interruptVoiceTurn).mockResolvedValue(status({ phase: "listening" }));
    const state = new VoiceConversationState();

    await state.start();
    await expect(state.interrupt()).resolves.toBe(true);

    expect(interruptVoiceTurn).toHaveBeenCalledOnce();
    expect(state.status.phase).toBe("listening");
    state.dispose();
  });

  it("reopens listening for the next turn", async () => {
    vi.mocked(startVoiceConversation).mockResolvedValue(status({ phase: "speaking" }));
    vi.mocked(resumeVoiceListening).mockResolvedValue(LISTENING);
    const state = new VoiceConversationState();

    await state.start();
    await expect(state.resume()).resolves.toBe(true);

    expect(resumeVoiceListening).toHaveBeenCalledOnce();
    expect(state.status.phase).toBe("listening");
    state.dispose();
  });

  it("ends native capture when an active session is disposed", async () => {
    vi.mocked(startVoiceConversation).mockResolvedValue(LISTENING);
    vi.mocked(endVoiceConversation).mockResolvedValue(INACTIVE);
    const state = new VoiceConversationState();

    await state.start();
    state.dispose();
    await vi.advanceTimersByTimeAsync(0);

    expect(endVoiceConversation).toHaveBeenCalledOnce();
  });
});
