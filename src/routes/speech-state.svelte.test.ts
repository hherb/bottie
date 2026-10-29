import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true, invoke: vi.fn() }));
vi.mock("$lib/speech", async (importOriginal) => ({
  ...(await importOriginal<typeof import("$lib/speech")>()),
  listSpeechVoices: vi.fn(),
  getSpeechStatus: vi.fn(),
}));

import { getSpeechStatus, listSpeechVoices, type SpeechVoice } from "$lib/speech";
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
