import { describe, expect, it } from "vitest";

import { voicePreviewRequested } from "./voice-preview";

describe("voice preview", () => {
  it("enables only explicit bounded voice presentation fixtures", () => {
    expect(voicePreviewRequested("?voice=final-transcript")).toBe(true);
    expect(voicePreviewRequested("?voice=local-playback")).toBe(true);
    expect(voicePreviewRequested("?voice=audio-content")).toBe(true);
    expect(voicePreviewRequested("?voice=input-devices")).toBe(true);
    expect(voicePreviewRequested("?voice=other")).toBe(false);
    expect(voicePreviewRequested("")).toBe(false);
  });
});
