import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";
import LocalSpeechSettings from "./LocalSpeechSettings.svelte";

describe("optional Nano voices", () => {
  it("keeps Nano in the ordinary voice picker with understandable multilingual and warm-up feedback", () => {
    const html = render(LocalSpeechSettings, {
      props: {
        disabled: false,
        speech: {
          available: true,
          voices: [
            { id: "local-voice-001", name: "Karen", language: "en-AU" },
            { id: "local-voice-002", name: "MOSS Nano · Ava", language: "mul" },
          ],
          status: {
            phase: "idle",
            selectedVoiceId: "local-voice-002",
            errorCode: null,
            latency: { playbackAcceptedMs: null },
          },
          selectVoice: vi.fn(),
        },
      },
    }).body;
    expect(html).toContain("MOSS Nano · Ava · Multilingual");
    expect(html).toContain("first Nano reply");
    expect(html).toContain("Karen · en-AU");
    expect(html).not.toContain("assetDirectory");
    expect(html).not.toContain("pythonPath");
  });
});
