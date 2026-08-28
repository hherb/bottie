import { render } from "svelte/server";
import { describe, expect, it } from "vitest";

import UpdateControl from "./UpdateControl.svelte";

describe("UpdateControl", () => {
  it("renders an explicit native check with calm trust-boundary disclosure", () => {
    const html = render(UpdateControl).body;

    expect(html).toContain("Application updates");
    expect(html).toContain("Check for updates");
    expect(html).toContain("Bottie checks only its fixed HTTPS GitHub release manifest");
    expect(html).toContain("Nothing downloads or installs until you choose it");
    expect(html).not.toContain("latest.json");
    expect(html).not.toContain("signature");
  });
});
