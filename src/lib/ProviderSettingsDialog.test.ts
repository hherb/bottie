import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import { DEFAULT_PROVIDER_SETTINGS } from "./presentation";
import ProviderSettingsDialog from "./ProviderSettingsDialog.svelte";

describe("ProviderSettingsDialog", () => {
  it("shows fixed Brave and Exa credentials plus an explicit search-engine choice", () => {
    const html = render(ProviderSettingsDialog, {
      props: {
        settings: DEFAULT_PROVIDER_SETTINGS,
        isGenerating: false,
        onclose: vi.fn(),
        onsaved: vi.fn(),
      },
    }).body;

    expect(html).toContain("Brave Search");
    expect(html).toContain("Exa Search");
    expect(html).toContain("Fixed native HTTPS search route");
    expect(html).toContain('id="brave-api-key"');
    expect(html).toContain('id="exa-api-key"');
    expect(html).toContain('aria-label="Choose web search engine"');
    expect(html).toContain('value="exa"');
    expect(html).toContain("Connection test sends one fixed bounded probe and does not expose search results.");
    expect(html).toContain("Web destination policy");
    expect(html).toContain("Require HTTPS destinations");
    expect(html).toContain('id="web-policy-allowed-domains"');
    expect(html).toContain('id="web-policy-blocked-domains"');
    expect(html).toContain("Private, loopback, special-use, and non-public addresses remain blocked");
    expect(html).not.toContain("Enable web search tool");
  });
});
