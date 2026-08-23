import { render } from "svelte/server";
import { describe, expect, it } from "vitest";

import LocalmailSettingsControl from "./LocalmailSettingsControl.svelte";

describe("LocalmailSettingsControl", () => {
  it("explains the bounded trust and bearer-token onboarding flow", () => {
    const html = render(LocalmailSettingsControl, { props: { disabled: false } }).body;

    expect(html).toContain("Localmail archive");
    expect(html).toContain("HTTPS origin");
    expect(html).toContain("Inspect certificate");
    expect(html).toContain("Confirm certificate trust");
    expect(html).toContain("Bearer token");
    expect(html).toContain("operating-system credential vault");
    expect(html).toContain("No email is read during setup");
    expect(html).not.toContain("search_email");
    expect(html).not.toContain("open_email");
  });
});
