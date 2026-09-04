import { describe, expect, it } from "vitest";

import {
  pythonToolReview,
  toolActivitySummary,
  toolAuditPresentation,
  toolDisplayName,
  untrustedWebResult,
} from "./tool-audit";
import type { StoredToolInvocation } from "./storage";

/** Builds one durable tool record for pure audit-presentation tests. */
function tool(overrides: Partial<StoredToolInvocation> = {}): StoredToolInvocation {
  return {
    ordinal: 0,
    toolName: "search_memory",
    arguments: { query: "release" },
    result: {
      output: { ok: true, result: { matches: [] } },
      isError: false,
      createdAtMs: 1_030,
    },
    audit: {
      policy: "safe",
      approval: null,
      outcome: "success",
      durationMs: 30,
    },
    createdAtMs: 1_000,
    ...overrides,
  };
}

describe("tool audit presentation", () => {
  it("uses calm product labels for the closed native memory tools", () => {
    expect(toolDisplayName("search_memory")).toBe("Search conversations");
    expect(toolDisplayName("open_memory")).toBe("Open conversation context");
    expect(toolDisplayName("search_attached_files")).toBe("Search attached files");
    expect(toolDisplayName("run_python")).toBe("Run Python");
    expect(toolDisplayName("future_tool")).toBe("future_tool");
  });

  it("accepts only the exact bounded Python source and purpose review shape", () => {
    const source = "print(sum([2, 3, 5]))";
    const purpose = "Add the values exactly.";

    expect(pythonToolReview(tool({ toolName: "run_python", arguments: { source, purpose } }))).toEqual({
      source,
      purpose,
    });
    expect(pythonToolReview(tool({ toolName: "other", arguments: { source, purpose } }))).toBeNull();
    expect(
      pythonToolReview(tool({ toolName: "run_python", arguments: { source, purpose, network: true } })),
    ).toBeNull();
    expect(pythonToolReview(tool({ toolName: "run_python", arguments: { source: " ", purpose } }))).toBeNull();
    expect(
      pythonToolReview(tool({ toolName: "run_python", arguments: { source, purpose: "x".repeat(513) } })),
    ).toBeNull();
    expect(
      pythonToolReview(tool({ toolName: "run_python", arguments: { source: "é".repeat(16_385), purpose } })),
    ).toBeNull();
  });

  it("recognizes only exact successful native fetch envelopes as explicitly untrusted", () => {
    const fetch = tool({
      toolName: "web_fetch",
      result: {
        output: { ok: true, result: { sourceUrl: "https://example.com/", content: "Page text", untrusted: true } },
        isError: false,
        createdAtMs: 1_030,
      },
    });

    expect(untrustedWebResult(fetch)).toBe(true);
    expect(untrustedWebResult({ ...fetch, toolName: "web_search" })).toBe(false);
    expect(untrustedWebResult({ ...fetch, result: { ...fetch.result!, isError: true } })).toBe(false);
    expect(
      untrustedWebResult({
        ...fetch,
        result: { ...fetch.result!, output: { ok: true, result: { untrusted: true } } },
      }),
    ).toBe(false);
    expect(
      untrustedWebResult({
        ...fetch,
        result: { ...fetch.result!, output: { ok: true, result: { content: "Missing marker" } } },
      }),
    ).toBe(false);
  });

  it("presents successful read-only execution without exposing engine detail", () => {
    expect(toolAuditPresentation(tool())).toEqual({
      status: "complete",
      statusLabel: "Complete",
      policyLabel: "Read-only",
      approvalLabel: null,
      outcomeLabel: "Succeeded",
      durationLabel: "30 ms",
    });
  });

  it("distinguishes blocked, failed, legacy, and pending records", () => {
    expect(
      toolAuditPresentation(
        tool({
          result: { output: { ok: false }, isError: true, createdAtMs: 2 },
          audit: { policy: "approval_required", approval: null, outcome: "approval_required", durationMs: 0 },
        }),
      ),
    ).toMatchObject({ status: "blocked", statusLabel: "Blocked", policyLabel: "Approval required" });
    expect(
      toolAuditPresentation(
        tool({
          result: { output: { ok: false }, isError: true, createdAtMs: 2 },
          audit: { policy: "safe", approval: null, outcome: "execution_failed", durationMs: 1_250 },
        }),
      ),
    ).toMatchObject({ status: "error", outcomeLabel: "Execution failed", durationLabel: "1.3 s" });
    expect(
      toolAuditPresentation(
        tool({ audit: { policy: "legacy", approval: null, outcome: "legacy_error", durationMs: null } }),
      ),
    ).toMatchObject({ policyLabel: "Legacy record", outcomeLabel: "Legacy error", durationLabel: null });
    expect(
      toolAuditPresentation(
        tool({ result: null, audit: { policy: "safe", approval: null, outcome: null, durationMs: null } }),
      ),
    ).toMatchObject({ status: "pending", statusLabel: "Pending", outcomeLabel: "Awaiting result" });
  });

  it("summarizes the call count and exceptional outcomes", () => {
    const failed = tool({
      ordinal: 1,
      result: { output: { ok: false }, isError: true, createdAtMs: 2 },
      audit: { policy: "safe", approval: null, outcome: "unavailable", durationMs: 4 },
    });
    expect(toolActivitySummary([tool(), failed])).toBe("2 calls · 1 needs attention");
    expect(toolActivitySummary([tool()])).toBe("1 call");
  });
});
