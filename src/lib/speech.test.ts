import { describe, expect, it } from "vitest";

import {
  assistantSpeechText,
  MAX_SPEECH_TEXT_BYTES,
  speechFeedback,
  speechTextWithinLimit,
  type SpeechStatus,
} from "./speech";

const IDLE_STATUS: SpeechStatus = {
  phase: "idle",
  selectedVoiceId: "voice.en-au",
  errorCode: null,
  latency: { playbackAcceptedMs: null },
};

describe("local speech presentation", () => {
  it("derives speakable text from rendered Markdown without destinations or syntax", () => {
    expect(
      assistantSpeechText("# Local answer\n\nRead **this** [guide](https://example.com) and run `cargo test`."),
    ).toBe("Local answer Read this guide and run cargo test.");
    expect(assistantSpeechText("![Architecture diagram](https://example.com/image.png)\n\n- One\n- Two")).toBe(
      "Architecture diagram One Two",
    );
  });

  it("checks the exact UTF-8 ceiling before invoking native playback", () => {
    expect(speechTextWithinLimit("é".repeat(MAX_SPEECH_TEXT_BYTES / 2))).toBe(true);
    expect(speechTextWithinLimit(`${"é".repeat(MAX_SPEECH_TEXT_BYTES / 2)}a`)).toBe(false);
    expect(speechTextWithinLimit("  ")).toBe(false);
  });

  it("uses calm path-free feedback for idle, playing, and fixed errors", () => {
    expect(speechFeedback(IDLE_STATUS, 3)).toBe("3 local voices available · playback stays on this device");
    expect(speechFeedback({ ...IDLE_STATUS, phase: "speaking" }, 3)).toBe(
      "Playing locally · native acceptance timing unavailable · use Stop to end playback",
    );
    expect(speechFeedback({ ...IDLE_STATUS, phase: "speaking", latency: { playbackAcceptedMs: 0 } }, 3)).toBe(
      "Playing locally · engine accepted playback in <1 ms · use Stop to end playback",
    );
    expect(speechFeedback({ ...IDLE_STATUS, phase: "error", errorCode: "unavailable" }, 0)).toBe(
      "Local voices are unavailable on this device.",
    );
  });
});
