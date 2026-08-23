import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import AppearancePreferences from "./AppearancePreferences.svelte";

describe("AppearancePreferences", () => {
  it("renders labelled keyboard-focusable theme and density choices", () => {
    const html = render(AppearancePreferences, {
      props: {
        appearance: { theme: "dark", density: "comfortable" },
        onchange: vi.fn(),
      },
    }).body;

    expect(html).toContain("Appearance");
    expect(html).toContain('aria-label="Theme"');
    expect(html).toContain('value="system"');
    expect(html).toContain('value="light"');
    expect(html).toContain('value="dark"');
    expect(html).toContain('aria-label="Density"');
    expect(html).toContain('value="comfortable"');
    expect(html).toContain('value="compact"');
    expect(html).toContain("Applied immediately and stored only on this device.");
  });
});
