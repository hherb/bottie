import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";
import GenerationSettingsControl from "./GenerationSettingsControl.svelte";

describe("GenerationSettingsControl", () => {
  it("shows saved budgets with integer bounds and explains the independent call limit", () => {
    const html = render(GenerationSettingsControl, {
      props: {
        limits: { maxToolRounds: 16, maxToolCalls: 32, maxOutputTokens: 12_288 },
        disabled: false,
        onchange: vi.fn(),
      },
    }).body;
    expect(html).toContain("Generation limits");
    expect(html).toContain('value="16"');
    expect(html).toContain('value="32"');
    expect(html).toContain('value="12288"');
    expect(html).toContain('max="64"');
    expect(html).toContain('max="128"');
    expect(html).toContain('max="65536"');
    expect(html).toContain("Both the round and total call limits apply");
    expect(html).toContain("survive restart");
  });
});
