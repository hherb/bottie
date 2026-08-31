import { describe, expect, it } from "vitest";

import {
  formatMicrophoneDuration,
  microphoneLatencyFeedback,
  microphoneFeedback,
  normalizeTranscriptCorrection,
  type MicrophoneStatus,
} from "./microphone";

const IDLE_STATUS: MicrophoneStatus = {
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

describe("microphone presentation", () => {
  it("normalizes transcript corrections within the shared UTF-8 boundary", () => {
    expect(normalizeTranscriptCorrection("  corrected turn  ")).toBe("corrected turn");
    expect(normalizeTranscriptCorrection("   ")).toBeNull();
    expect(normalizeTranscriptCorrection("é".repeat(256))).toHaveLength(256);
    expect(normalizeTranscriptCorrection(`é${"a".repeat(511)}`)).toBeNull();
  });

  it("formats bounded capture durations without exposing native audio", () => {
    expect(formatMicrophoneDuration(0)).toBe("0:00");
    expect(formatMicrophoneDuration(3_420)).toBe("0:03");
    expect(formatMicrophoneDuration(60_000)).toBe("1:00");
  });

  it("labels only observable native timing endpoints and never renders missing values as zero", () => {
    expect(
      microphoneLatencyFeedback({
        ...IDLE_STATUS,
        phase: "recording",
        latency: { inputReadyMs: 18, firstTranscriptMs: null, finalTranscriptMs: null },
      }),
    ).toBe("Native timing · Input ready after Record: 18 ms · First transcript after Record: waiting");
    expect(
      microphoneLatencyFeedback({
        ...IDLE_STATUS,
        phase: "captured",
        transcriptionPhase: "ready",
        latency: { inputReadyMs: 0, firstTranscriptMs: 1_725, finalTranscriptMs: 245 },
      }),
    ).toBe(
      "Native timing · Input ready after Record: <1 ms · First transcript after Record: 1.73 s · Final transcript after capture stopped: 245 ms",
    );
    expect(microphoneLatencyFeedback(IDLE_STATUS)).toBeNull();
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
        voiceActivity: "speech",
        voiceSegments: [{ activity: "speech", startMs: 0, endMs: 3_420 }],
      }),
    ).toBe("Recording locally · Speech detected · 0:03 of 1:00");
    expect(
      microphoneFeedback({
        ...IDLE_STATUS,
        phase: "captured",
        permission: "granted",
        durationMs: 3_420,
        retainedByteSize: 656_640,
        voiceActivity: "silence",
        voiceSegments: [
          { activity: "silence", startMs: 0, endMs: 420 },
          { activity: "speech", startMs: 420, endMs: 2_920 },
          { activity: "silence", startMs: 2_920, endMs: 3_420 },
        ],
      }),
    ).toBe("Voice capture · 0:03 · 0:02 speech · held only in native memory");
  });

  it("describes silence calmly without exposing detector internals", () => {
    expect(
      microphoneFeedback({
        ...IDLE_STATUS,
        phase: "recording",
        permission: "granted",
        durationMs: 800,
        sampleRateHz: 48_000,
        channels: 1,
        voiceActivity: "silence",
        voiceSegments: [{ activity: "silence", startMs: 0, endMs: 800 }],
      }),
    ).toBe("Recording locally · Listening for speech · 0:00 of 1:00");
  });

  it("describes bounded local partial and final transcription state", () => {
    expect(
      microphoneFeedback({
        ...IDLE_STATUS,
        phase: "recording",
        permission: "granted",
        transcriptionPhase: "preparing_model",
      }),
    ).toContain("Preparing the local speech model");
    expect(
      microphoneFeedback({
        ...IDLE_STATUS,
        phase: "captured",
        permission: "granted",
        transcriptionPhase: "ready",
        transcriptSegments: [{ text: "Hello locally", startMs: 420, endMs: 2_920, isFinal: true, isCorrected: false }],
      }),
    ).toContain("Transcript ready locally");
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
    expect(
      microphoneFeedback({
        ...IDLE_STATUS,
        phase: "error",
        permission: "unavailable",
        errorCode: "selected_device_unavailable",
      }),
    ).toBe(
      "The selected microphone is no longer available. Choose another microphone or System default, then try again.",
    );
  });
});
