import { describe, expect, it } from "vitest";

import { PageState } from "./page-state.svelte";
import { applyVoicePreview, voicePreviewRequested } from "./voice-preview";

describe("voice preview", () => {
  it("enables only explicit bounded voice presentation fixtures", () => {
    expect(voicePreviewRequested("?voice=final-transcript")).toBe(true);
    expect(voicePreviewRequested("?voice=local-playback")).toBe(true);
    expect(voicePreviewRequested("?voice=audio-content")).toBe(true);
    expect(voicePreviewRequested("?voice=input-devices")).toBe(true);
    expect(voicePreviewRequested("?voice=other")).toBe(false);
    expect(voicePreviewRequested("")).toBe(false);
  });

  it("keeps the final-transcript fixture editable without artificial provider readiness", () => {
    const state = new PageState();

    expect(applyVoicePreview(state, "?voice=final-transcript")).toBe(true);
    expect(state.providerStatus).toBe("browser");
    expect(state.canSend).toBe(false);
    expect(state.canCompose).toBe(true);
    expect(state.prompt).toBe("");
    expect(state.microphone.status.transcriptionPhase).toBe("ready");
  });
});
