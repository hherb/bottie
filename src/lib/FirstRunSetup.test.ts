import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import FirstRunSetup from "./FirstRunSetup.svelte";

describe("FirstRunSetup", () => {
  it("discloses the active cloud route and off-by-default context before completion", () => {
    const html = render(FirstRunSetup, {
      props: {
        providerName: "OpenAI compatible",
        modelName: "GPT test",
        isLocalRoute: false,
        canComplete: true,
        isSaving: false,
        error: "",
        onopensettings: vi.fn(),
        oncomplete: vi.fn(),
      },
    }).body;

    expect(html).toContain("Before your first conversation");
    expect(html).toContain("OpenAI compatible");
    expect(html).toContain("GPT test");
    expect(html).toContain("Cloud route");
    expect(html).toContain("Prompts, delivered images, and explicitly enabled tool results go to this provider");
    expect(html).toContain("Conversations, files, and derived memory stay in Bottie’s local storage");
    expect(html).toContain("Memory and Web start off for every app session");
    expect(html).toContain("Finish setup");
  });

  it("guides an offline first run to provider settings without pretending setup is ready", () => {
    const html = render(FirstRunSetup, {
      props: {
        providerName: null,
        modelName: null,
        isLocalRoute: true,
        canComplete: false,
        isSaving: false,
        error: "",
        onopensettings: vi.fn(),
        oncomplete: vi.fn(),
      },
    }).body;

    expect(html).toContain("No streaming model is ready yet");
    expect(html).toContain("Open provider settings");
    expect(html).toContain("disabled");
  });
});
