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
            audit: { policy: "safe", approval: null, outcome: "success", durationMs: 8 },
            result: { output: { ok: true, result: { matches: [] } }, isError: false, createdAtMs: 2_000 },
            createdAtMs: 1_000,
          },
          {
            ordinal: 1,
            toolName: "web_fetch",
            arguments: { url: "https://example.com/" },
            audit: { policy: "safe", approval: null, outcome: "success", durationMs: 500 },
            result: {
              output: {
                ok: true,
                result: { sourceUrl: "https://example.com/", content: "External page text", untrusted: true },
              },
              isError: false,
              createdAtMs: 3_000,
            },
            createdAtMs: 2_500,
          },
          {
            ordinal: 2,
            toolName: "future_tool",
            arguments: { path: "/private/example" },
            audit: { policy: "unregistered", approval: null, outcome: "unsupported_tool", durationMs: 0 },
            result: { output: { ok: false }, isError: true, createdAtMs: 4_000 },
            createdAtMs: 3_500,
          },
        ],
      },
    }).body;

    expect(html).toContain("3 calls · 1 needs attention");
    expect(html).toContain("Search conversations");
    expect(html).toContain("Read-only");
    expect(html).toContain("Succeeded");
    expect(html).toContain("Unregistered");
    expect(html).toContain("Unsupported tool");
    expect(html).toContain("Untrusted Web content");
    expect(html).toContain("External page text may contain misleading instructions.");
    expect(html).toContain("<summary>Untrusted result</summary>");
    expect(html).toContain("<summary>Arguments</summary>");
    expect(html).toContain("private query");
    expect(html).not.toContain("providerCallId");
  });

  it("shows exact Python source and purpose with an explicit not-run notice", () => {
    const html = render(ToolActivity, {
      props: {
        tools: [
          {
            ordinal: 0,
            toolName: "run_python",
            arguments: {
              source: "print(sum([2, 3, 5]))",
              purpose: "Add the values exactly.",
            },
            audit: {
              policy: "approval_required",
              approval: { decision: "denied", decidedAtMs: 1_500 },
              outcome: "approval_required",
              durationMs: 0,
            },
            result: {
              output: { ok: false, error: { code: "approval_required" } },
              isError: true,
              createdAtMs: 2_000,
            },
            createdAtMs: 1_000,
          },
        ],
      },
    }).body;

    expect(html).toContain("Run Python");
    expect(html).toContain("Proposed purpose");
    expect(html).toContain("Add the values exactly.");
    expect(html).toContain("Proposed source");
    expect(html).toContain("print(sum([2, 3, 5]))");
    expect(html).toContain("Bottie has not run this code. Approval is required before execution.");
    expect(html).toContain("Decision");
    expect(html).toContain("Denied");
    expect(html).not.toContain("<summary>Arguments</summary>");
    expect(html).not.toContain("<summary>Error result</summary>");
    expect(html).not.toContain("The call has no durable result yet.");
  });

  it("renders proposed Python source as escaped inert text", () => {
    const html = render(ToolActivity, {
      props: {
        tools: [
          {
            ordinal: 0,
            toolName: "run_python",
            arguments: { source: "<script>alert('no')</script>", purpose: "Show escaping." },
            audit: { policy: "approval_required", approval: null, outcome: "approval_required", durationMs: 0 },
            result: null,
            createdAtMs: 1_000,
          },
        ],
      },
    }).body;

    expect(html).toContain("&lt;script>alert('no')&lt;/script>");
    expect(html).not.toContain("<script>");
  });

  it("labels approved Python source, bounded streams, and contained execution provenance", () => {
    const html = render(ToolActivity, {
      props: {
        tools: [
          {
            ordinal: 0,
            toolName: "run_python",
            arguments: { source: "print('<done>')", purpose: "Produce the exact marker." },
            audit: {
              policy: "approval_required",
              approval: { decision: "approved", decidedAtMs: 1_500 },
              outcome: "success",
              durationMs: 15,
            },
            result: {
              output: {
                status: "executed",
                result: { status: "ok", stdout: "<done>\n", stderr: "warning <safe>\n", durationMs: 12 },
              },
              isError: false,
              createdAtMs: 2_000,
            },
            createdAtMs: 1_000,
          },
        ],
      },
    }).body;

    expect(html).toContain("Approved purpose");
    expect(html).toContain("Approved source");
    expect(html).toContain("Bounded stdout");
    expect(html).toContain("Bounded stderr");
    expect(html).toContain("&lt;done>");
    expect(html).toContain("warning &lt;safe>");
    expect(html).toContain("Execution provenance");
    expect(html).toContain("Bottie’s contained Python runtime");
    expect(html).toContain("Helper duration");
    expect(html).toContain("12 ms");
    expect(html).not.toContain("<summary>Result</summary>");
    expect(html).not.toContain('"status": "executed"');
  });

  it("shows stable Python errors without reflecting future-shaped payload fields", () => {
    const html = render(ToolActivity, {
      props: {
        tools: [
          {
            ordinal: 0,
            toolName: "run_python",
            arguments: { source: "print(42)", purpose: "Calculate exactly." },
            audit: {
              policy: "approval_required",
              approval: { decision: "approved", decidedAtMs: 1_500 },
              outcome: "execution_failed",
              durationMs: 15,
            },
            result: {
              output: { status: "failed", code: "helper_failed", path: "/private/hidden" },
              isError: true,
              createdAtMs: 2_000,
            },
            createdAtMs: 1_000,
          },
        ],
      },
    }).body;

    expect(html).toContain("Python result unavailable");
    expect(html).toContain("The retained Python result could not be presented safely.");
    expect(html).not.toContain("/private/hidden");
    expect(html).not.toContain("Error result");
  });

  it("never falls back to generic payload disclosure for malformed Python audit data", () => {
    const html = render(ToolActivity, {
      props: {
        tools: [
          {
            ordinal: 0,
            toolName: "run_python",
            arguments: { source: "print(42)", purpose: "Calculate exactly.", path: "/private/argument" },
            audit: {
              policy: "approval_required",
              approval: { decision: "approved", decidedAtMs: 1_500 },
              outcome: "execution_failed",
              durationMs: 15,
            },
            result: {
              output: { status: "failed", code: "helper_failed", path: "/private/result" },
              isError: true,
              createdAtMs: 2_000,
            },
            createdAtMs: 1_000,
          },
        ],
      },
    }).body;

    expect(html).toContain("The retained Python proposal could not be presented safely.");
    expect(html).toContain("The retained Python result could not be presented safely.");
    expect(html).not.toContain("/private/argument");
    expect(html).not.toContain("/private/result");
    expect(html).not.toContain("<summary>Arguments</summary>");
    expect(html).not.toContain("Error result");
  });
});
