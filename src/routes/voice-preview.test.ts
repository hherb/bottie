import { describe, expect, it } from "vitest";

import { voicePreviewRequested } from "./voice-preview";

describe("voice preview", () => {
  it("enables only the explicit final-transcript development fixture query", () => {
    expect(voicePreviewRequested("?voice=final-transcript")).toBe(true);
    expect(voicePreviewRequested("?voice=other")).toBe(false);
    expect(voicePreviewRequested("")).toBe(false);
  });
});
