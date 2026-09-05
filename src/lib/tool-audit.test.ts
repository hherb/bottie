import { describe, expect, it } from "vitest";

import {
  pythonExecutionPresentation,
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

  it("presents only exact approved bounded Python execution results", () => {
    const execution = tool({
      toolName: "run_python",
      arguments: { source: "print(42)", purpose: "Calculate exactly." },
      audit: {
        policy: "approval_required",
        approval: { decision: "approved", decidedAtMs: 1_010 },
        outcome: "success",
        durationMs: 18,
      },
      result: {
        output: {
          status: "executed",
          result: { status: "ok", stdout: "42\n", stderr: "", durationMs: 12 },
        },
        isError: false,
        createdAtMs: 1_030,
      },
    });

    expect(pythonExecutionPresentation(execution)).toEqual({
      kind: "executed",
      statusLabel: "Completed",
      stdout: "42\n",
      stderr: "",
      durationLabel: "12 ms",
    });
    expect(
      pythonExecutionPresentation({
        ...execution,
        result: {
          ...execution.result!,
          output: {
            status: "executed",
            result: { status: "ok", stdout: "42\n", stderr: "", durationMs: 12 },
            path: "/private/hidden",
          },
        },
      }),
    ).toEqual({
      kind: "invalid",
      message: "The retained Python result could not be presented safely.",
    });
    expect(
      pythonExecutionPresentation({
        ...execution,
        result: {
          ...execution.result!,
          output: {
            status: "executed",
            result: { status: "ok", stdout: "x".repeat(32 * 1_024 + 1), stderr: "", durationMs: 12 },
          },
        },
      }),
    ).toMatchObject({ kind: "invalid" });
  });

  it("maps closed Python terminal outcomes to stable path-free explanations", () => {
    const python = tool({
      toolName: "run_python",
      arguments: { source: "print(42)", purpose: "Calculate exactly." },
      audit: {
        policy: "approval_required",
        approval: { decision: "approved", decidedAtMs: 1_010 },
        outcome: "execution_failed",
        durationMs: 18,
      },
      result: {
        output: { status: "failed", code: "helper_failed" },
        isError: true,
        createdAtMs: 1_030,
      },
    });

    expect(pythonExecutionPresentation(python)).toEqual({
      kind: "failed",
      statusLabel: "Helper failed",
      message: "The contained Python helper could not complete safely.",
    });
    expect(
      pythonExecutionPresentation({
        ...python,
        audit: {
          policy: "approval_required",
          approval: { decision: "denied", decidedAtMs: 1_010 },
          outcome: "approval_required",
          durationMs: 0,
        },
        result: { output: { status: "denied" }, isError: true, createdAtMs: 1_030 },
      }),
    ).toEqual({
      kind: "denied",
      statusLabel: "Not executed",
      message: "The user denied this Python proposal. No code was run.",
    });
    expect(
      pythonExecutionPresentation({
        ...python,
        audit: {
          policy: "approval_required",
          approval: null,
          outcome: "execution_failed",
          durationMs: 0,
        },
        result: { output: { status: "cancelled" }, isError: true, createdAtMs: 1_030 },
      }),
    ).toEqual({
      kind: "cancelled",
      statusLabel: "Cancelled",
      message: "The Python proposal was cancelled before approval. No code was run.",
    });
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
