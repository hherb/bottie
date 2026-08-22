import { render } from "svelte/server";
import { describe, expect, it } from "vitest";

import ToolActivity from "./ToolActivity.svelte";

describe("ToolActivity", () => {
  it("renders audit summaries before inert payload disclosures", () => {
    const html = render(ToolActivity, {
      props: {
        tools: [
          {
            ordinal: 0,
            toolName: "search_memory",
            arguments: { query: "private query" },
            audit: { policy: "safe", outcome: "success", durationMs: 8 },
            result: { output: { ok: true, result: { matches: [] } }, isError: false, createdAtMs: 2_000 },
            createdAtMs: 1_000,
          },
          {
            ordinal: 1,
            toolName: "future_tool",
            arguments: { path: "/private/example" },
            audit: { policy: "unregistered", outcome: "unsupported_tool", durationMs: 0 },
            result: { output: { ok: false }, isError: true, createdAtMs: 3_000 },
            createdAtMs: 2_500,
          },
        ],
      },
    }).body;

    expect(html).toContain("2 calls · 1 needs attention");
    expect(html).toContain("Search conversations");
    expect(html).toContain("Read-only");
    expect(html).toContain("Succeeded");
    expect(html).toContain("Unregistered");
    expect(html).toContain("Unsupported tool");
    expect(html).toContain("<summary>Arguments</summary>");
    expect(html).toContain("private query");
    expect(html).not.toContain("providerCallId");
  });
});
