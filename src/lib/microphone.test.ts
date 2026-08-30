import { describe, expect, it } from "vitest";

import { formatMicrophoneDuration, microphoneFeedback, type MicrophoneStatus } from "./microphone";

const IDLE_STATUS: MicrophoneStatus = {
  phase: "idle",
  permission: "prompt",
  durationMs: 0,
  maxDurationMs: 60_000,
  sampleRateHz: null,
  channels: null,
  retainedByteSize: 0,
  inputLevel: 0,
  errorCode: null,
};

describe("microphone presentation", () => {
  it("formats bounded capture durations without exposing native audio", () => {
    expect(formatMicrophoneDuration(0)).toBe("0:00");
    expect(formatMicrophoneDuration(3_420)).toBe("0:03");
    expect(formatMicrophoneDuration(60_000)).toBe("1:00");
  });

  it("describes explicit permission, active capture, and session-only retention", () => {
    expect(microphoneFeedback(IDLE_STATUS)).toBe("Microphone access is requested only when you choose Record voice.");
    expect(
      microphoneFeedback({
        ...IDLE_STATUS,
        phase: "recording",
        permission: "granted",
        durationMs: 3_420,
        sampleRateHz: 48_000,
        channels: 1,
      }),
    ).toBe("Recording locally · 0:03 of 1:00");
    expect(
      microphoneFeedback({
        ...IDLE_STATUS,
        phase: "captured",
        permission: "granted",
        durationMs: 3_420,
        retainedByteSize: 656_640,
      }),
    ).toBe("Voice capture · 0:03 · held only in native memory");
  });

  it("maps stable native errors without reflecting backend details", () => {
    expect(
      microphoneFeedback({ ...IDLE_STATUS, phase: "error", permission: "denied", errorCode: "permission_denied" }),
    ).toBe("Microphone access was denied. Allow Bottie in system privacy settings, then try again.");
    expect(
      microphoneFeedback({
        ...IDLE_STATUS,
        phase: "error",
        permission: "unavailable",
        errorCode: "device_unavailable",
      }),
    ).toBe("No microphone is available. Connect or enable an input device, then try again.");
  });
});
