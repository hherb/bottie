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
  errorCode: null,
};

/** Renders the microphone control with inert actions and one path-free native status. */
function rendered(status: MicrophoneStatus, disabled = false): string {
  return render(MicrophoneControl, {
    props: { status, disabled, onstart: vi.fn(), onstop: vi.fn(), ondiscard: vi.fn() },
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

  it("shows a clear stop action and bounded local activity while recording", () => {
    const html = rendered({
      ...IDLE_STATUS,
      phase: "recording",
      permission: "granted",
      durationMs: 3_420,
      sampleRateHz: 48_000,
      channels: 1,
      inputLevel: 0.42,
    });

    expect(html).toContain('aria-label="Stop voice capture"');
    expect(html).toContain("Recording locally");
    expect(html).toContain('role="progressbar"');
    expect(html).toContain('aria-valuenow="42"');
  });

  it("keeps a completed capture visibly session-only and discardable", () => {
    const html = rendered({
      ...IDLE_STATUS,
      phase: "captured",
      permission: "granted",
      durationMs: 3_420,
      retainedByteSize: 656_640,
    });

    expect(html).toContain("held only in native memory");
    expect(html).toContain('aria-label="Discard voice capture"');
    expect(html).toContain("Record again");
    expect(html).not.toContain("Send voice");
  });
});
