import { render } from "svelte/server";
import { describe, expect, it } from "vitest";

import MemoryIndexControl from "./MemoryIndexControl.svelte";

describe("MemoryIndexControl", () => {
  it("keeps the derived-only reindex policy explicit in browser presentation", () => {
    const html = render(MemoryIndexControl, { props: { disabled: false } }).body;

    expect(html).toContain("Semantic memory index");
    expect(html).toContain("Reindex memory");
    expect(html).toContain("Source content and the app-owned model cache are retained.");
    expect(html).toMatch(/<button[^>]*disabled/);
    expect(html).not.toMatch(/\/(Users|home|var|tmp)\//);
  });
});
