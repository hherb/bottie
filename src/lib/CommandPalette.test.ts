import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import CommandPalette from "./CommandPalette.svelte";
import { commandPaletteItems } from "./command-palette";

describe("CommandPalette", () => {
  it("renders an accessible searchable dialog with shortcuts and disabled reasons", () => {
    const html = render(CommandPalette, {
      props: {
        items: commandPaletteItems({
          busy: true,
          contextOpen: false,
          platform: "MacIntel",
          storageAvailable: true,
        }),
        onclose: vi.fn(),
        onrun: vi.fn(),
      },
    }).body;

    expect(html).toContain('role="dialog"');
    expect(html).toContain('aria-modal="true"');
    expect(html).toContain('aria-label="Search commands"');
    expect(html).toContain('role="listbox"');
    expect(html).toContain("New conversation");
    expect(html).toContain("⌘ N");
    expect(html).toContain("Finish the current response before starting a new conversation.");
    expect(html).toContain('aria-disabled="true"');
  });
});
