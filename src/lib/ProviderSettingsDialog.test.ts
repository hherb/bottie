import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import { DEFAULT_PROVIDER_SETTINGS } from "./presentation";
import ProviderSettingsDialog from "./ProviderSettingsDialog.svelte";

describe("ProviderSettingsDialog", () => {
  it("shows fixed Brave and Exa credentials plus an explicit search-engine choice", () => {
    const html = render(ProviderSettingsDialog, {
      props: {
        settings: DEFAULT_PROVIDER_SETTINGS,
        appearance: { theme: "dark", density: "comfortable" },
        speech: {
          available: true,
          voices: [{ id: "voice.en-au", name: "Karen", language: "en-AU" }],
          status: {
            phase: "idle",
            selectedVoiceId: "voice.en-au",
            errorCode: null,
            latency: { playbackAcceptedMs: null },
          },
          selectVoice: vi.fn(),
        },
        isGenerating: false,
        onclose: vi.fn(),
        onappearancechange: vi.fn(),
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
    expect(html).toContain("Recent diagnostics");
    expect(html).toContain("Export JSON");
    expect(html).toContain("Structured with secrets and paths redacted");
    expect(html).toContain("Appearance");
    expect(html).toContain("System");
    expect(html).toContain("Comfortable");
    expect(html).toContain("Compact");
    expect(html).toContain("Local speech voice");
    expect(html).toContain('aria-label="Local playback voice"');
    expect(html).toContain("Karen · en-AU");
    expect(html).toContain('aria-describedby="provider-settings-description"');
    expect(html).toContain('id="provider-settings-description"');
    expect(html.match(/Local routes require loopback endpoints\./g)).toHaveLength(1);
    expect(html).not.toContain("Enable web search tool");
  });
});
