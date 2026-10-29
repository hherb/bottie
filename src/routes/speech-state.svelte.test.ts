import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true, invoke: vi.fn() }));
vi.mock("$lib/speech", async (importOriginal) => ({
  ...(await importOriginal<typeof import("$lib/speech")>()),
  listSpeechVoices: vi.fn(),
  getSpeechStatus: vi.fn(),
  speakText: vi.fn(),
  stopSpeech: vi.fn(),
}));

import {
  getSpeechStatus,
  listSpeechVoices,
  speakText,
  stopSpeech,
  type SpeechStatus,
  type SpeechVoice,
} from "$lib/speech";
import { SpeechState } from "./speech-state.svelte";

describe("native saved speech selection", () => {
  beforeEach(() => vi.clearAllMocks());

  it("reads the restored selection after voice discovery has initialized the native engine", async () => {
    let finishDiscovery!: (voices: SpeechVoice[]) => void;
    vi.mocked(listSpeechVoices).mockReturnValue(new Promise((resolve) => (finishDiscovery = resolve)));
    vi.mocked(getSpeechStatus).mockResolvedValue({
      phase: "idle",
      selectedVoiceId: "local-voice-002",
      errorCode: null,
      latency: { playbackAcceptedMs: null },
    });
    const state = new SpeechState();
    const initialized = state.initialize();
    expect(getSpeechStatus).not.toHaveBeenCalled();
    finishDiscovery([{ id: "local-voice-002", name: "MOSS Nano · Ava", language: "mul" }]);
    await initialized;
    expect(state.status.selectedVoiceId).toBe("local-voice-002");
    expect(state.voices[0].name).toBe("MOSS Nano · Ava");
  });
});

describe("speech status polling", () => {
  beforeEach(() => vi.clearAllMocks());

  const status = (phase: SpeechStatus["phase"]): SpeechStatus => ({
    phase,
    selectedVoiceId: "local-voice-001",
    errorCode: null,
    latency: { playbackAcceptedMs: null },
  });

  it("ignores a poll that was in flight when Stop was pressed", async () => {
    vi.useFakeTimers();
    try {
      vi.mocked(speakText).mockResolvedValue(status("speaking"));
      vi.mocked(stopSpeech).mockResolvedValue(status("idle"));
      let finishPoll!: (value: SpeechStatus) => void;
      vi.mocked(getSpeechStatus).mockReturnValue(new Promise((resolve) => (finishPoll = resolve)));
      const state = new SpeechState();
      state.voices = [{ id: "local-voice-001", name: "Voice", language: "en" }];

      await state.speak(2, "Hello there.");
      await vi.advanceTimersByTimeAsync(200);
      expect(getSpeechStatus).toHaveBeenCalledOnce();
      await state.stop();
      finishPoll(status("speaking"));
      await vi.advanceTimersByTimeAsync(0);

      expect(state.status.phase).toBe("idle");
      expect(state.activeMessageId).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});
