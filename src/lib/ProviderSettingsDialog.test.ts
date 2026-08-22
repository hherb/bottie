import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import { DEFAULT_PROVIDER_SETTINGS } from "./presentation";
import ProviderSettingsDialog from "./ProviderSettingsDialog.svelte";

describe("ProviderSettingsDialog", () => {
  it("shows Brave as a fixed native web-search credential without a model-tool claim", () => {
    const html = render(ProviderSettingsDialog, {
      props: {
        settings: DEFAULT_PROVIDER_SETTINGS,
        isGenerating: false,
        onclose: vi.fn(),
        onsaved: vi.fn(),
      },
    }).body;

    expect(html).toContain("Brave Search");
    expect(html).toContain("Fixed native HTTPS search route");
    expect(html).toContain('id="brave-api-key"');
    expect(html).toContain("Connection test sends one fixed bounded probe and does not expose search results.");
    expect(html).not.toContain("Enable web search tool");
  });
});
