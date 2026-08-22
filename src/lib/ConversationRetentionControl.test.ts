import { render } from "svelte/server";
import { describe, expect, it } from "vitest";

import ConversationRetentionControl from "./ConversationRetentionControl.svelte";

describe("ConversationRetentionControl", () => {
  it("renders the opt-in destructive policy explicitly in browser presentation", () => {
    const html = render(ConversationRetentionControl, { props: { disabled: false } }).body;

    expect(html).toContain("Trash retention");
    expect(html).toContain("Keep until I forget manually");
    expect(html).toContain("Save retention");
    expect(html).toContain("Automatic forget leaves existing exports");
    expect(html).toContain("backups unchanged");
    expect(html).toMatch(/<button[^>]*disabled/);
    expect(html).not.toMatch(/\/(Users|home|var|tmp)\//);
  });
});
