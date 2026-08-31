import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import MicrophoneControl from "./MicrophoneControl.svelte";
import type { MicrophoneStatus } from "./microphone";

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
};

/** Renders the microphone control with inert actions and one path-free native status. */
function rendered(status: MicrophoneStatus, disabled = false, willInterrupt = false): string {
  return render(MicrophoneControl, {
    props: {
      status,
      disabled,
      willInterrupt,
      onstart: vi.fn(),
      onstop: vi.fn(),
      ondiscard: vi.fn(),
      oncorrect: vi.fn(),
    },
  }).body;
}

describe("MicrophoneControl", () => {
  it("requests microphone access only behind an explicit labelled action", () => {
    const html = rendered(IDLE_STATUS);

    expect(html).toContain('aria-label="Record voice locally"');
    expect(html).toContain("Record voice");
    expect(html).toContain("requested only when you choose Record voice");
    expect(html).not.toContain("getUserMedia");
  });

  it("labels Record as an explicit interruption when Bottie is producing output", () => {
    const html = rendered(IDLE_STATUS, false, true);

    expect(html).toContain('aria-label="Interrupt Bottie and record voice locally"');
    expect(html).toContain("Interrupt &amp; record");
    expect(html).not.toMatch(/aria-label="Interrupt Bottie and record voice locally"[^>]*disabled/);
  });

  it("shows a clear stop action and bounded local activity while recording", () => {
    const html = rendered({
      ...IDLE_STATUS,
      phase: "recording",
      permission: "granted",
      durationMs: 3_420,
      sampleRateHz: 48_000,
      channels: 1,
      inputLevel: 0.42,
      voiceActivity: "speech",
      voiceSegments: [{ activity: "speech", startMs: 0, endMs: 3_420 }],
    });

    expect(html).toContain('aria-label="Stop voice capture"');
    expect(html).toContain("Recording locally");
    expect(html).toContain('role="progressbar"');
    expect(html).toContain('aria-valuenow="42"');
    expect(html).toContain("Speech detected");
  });

  it("keeps a completed capture visibly session-only and discardable", () => {
    const html = rendered({
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
    });

    expect(html).toContain("held only in native memory");
    expect(html).toContain("0:02 speech");
    expect(html).toContain('aria-label="Discard voice capture"');
    expect(html).toContain("Record again");
    expect(html).not.toContain("Send voice");
  });

  it("renders path-free partial and final transcript turns with visible timing", () => {
    const html = rendered({
      ...IDLE_STATUS,
      phase: "captured",
      permission: "granted",
      durationMs: 3_420,
      transcriptionPhase: "ready",
      transcriptSegments: [
        { text: "Hello there", startMs: 420, endMs: 1_800, isFinal: true, isCorrected: false },
        { text: "General Kenobi", startMs: 1_900, endMs: 3_100, isFinal: true, isCorrected: true },
      ],
    });

    expect(html).toContain('aria-label="Local voice transcript"');
    expect(html).toContain("Hello there");
    expect(html).toContain("General Kenobi");
    expect(html).toContain("0:00–0:01");
    expect(html).toContain("0:01–0:03");
    expect(html).toContain("Turn 1");
    expect(html).toContain("Turn 2");
    expect(html).toContain('aria-label="Correct voice turn 1"');
    expect(html).toContain('aria-label="Save correction for voice turn 1"');
    expect(html).toContain("Corrected");
    expect(html).not.toMatch(/model path|sample|hash/i);
  });

  it("keeps partial turns visible but non-editable", () => {
    const html = rendered({
      ...IDLE_STATUS,
      phase: "recording",
      permission: "granted",
      transcriptionPhase: "transcribing",
      transcriptSegments: [{ text: "Still listening", startMs: 420, endMs: 1_800, isFinal: false, isCorrected: false }],
    });

    expect(html).toContain("Turn 1");
    expect(html).toContain("Still listening");
    expect(html).toContain("Partial");
    expect(html).not.toContain("Correct voice turn 1");
  });

  it("keeps capture actions bounded while the native recognizer is preparing", () => {
    const html = rendered({
      ...IDLE_STATUS,
      phase: "captured",
      permission: "granted",
      transcriptionPhase: "preparing_model",
    });

    expect(html).toContain("Preparing the local speech model");
    expect(html).toMatch(/aria-label="Record voice locally"[^>]*disabled/);
    expect(html).toContain('aria-label="Discard voice capture"');
  });
});
