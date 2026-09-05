/** Pure presentation helpers for native-owned durable tool audit records. */

import type { StoredToolInvocation } from "./storage";

/** Calm UI state derived from one structured native audit record. */
export type ToolAuditPresentation = {
  status: "pending" | "complete" | "blocked" | "error";
  statusLabel: string;
  policyLabel: string;
  approvalLabel: string | null;
  outcomeLabel: string;
  durationLabel: string | null;
};

const TOOL_DISPLAY_NAMES: Record<string, string> = {
  search_memory: "Search conversations",
  open_memory: "Open conversation context",
  search_attached_files: "Search attached files",
  run_python: "Run Python",
};

const MAX_PYTHON_SOURCE_BYTES = 32 * 1_024;
const MAX_PYTHON_PURPOSE_CHARACTERS = 512;
const MAX_PYTHON_STREAM_BYTES = 32 * 1_024;
const INVALID_PYTHON_RESULT_MESSAGE = "The retained Python result could not be presented safely.";

const PYTHON_EXECUTION_STATUS_LABELS = {
  ok: "Completed",
  python_error: "Python error",
  timed_out: "Timed out",
  output_limit: "Output limit reached",
  resource_limit: "Resource limit reached",
  invalid_request: "Request rejected",
  internal_error: "Execution failed",
} as const;

const PYTHON_FAILURES = {
  approval_failed: {
    statusLabel: "Approval failed",
    message: "Bottie could not complete the Python approval safely.",
    outcome: "approval_required",
    approval: null,
  },
  invalid_request: {
    statusLabel: "Request rejected",
    message: "The approved Python request no longer matched the reviewed proposal.",
    outcome: "invalid_arguments",
    approval: "approved",
  },
  helper_failed: {
    statusLabel: "Helper failed",
    message: "The contained Python helper could not complete safely.",
    outcome: "execution_failed",
    approval: "approved",
  },
  invalid_result: {
    statusLabel: "Result rejected",
    message: "The contained Python helper returned a result Bottie could not accept safely.",
    outcome: "execution_failed",
    approval: "approved",
  },
} as const;

/** Exact source and purpose safe to show in the dedicated Python proposal review. */
export type PythonToolReview = {
  source: string;
  purpose: string;
};

/** Closed user-visible interpretation of one retained Python terminal payload. */
export type PythonExecutionPresentation =
  | {
      kind: "executed";
      statusLabel: string;
      stdout: string;
      stderr: string;
      durationLabel: string;
    }
  | {
      kind: "denied" | "cancelled" | "failed";
      statusLabel: string;
      message: string;
    }
  | {
      kind: "invalid";
      message: string;
    };

/** Accepts only the native Python tool's closed, bounded source-and-purpose argument shape. */
export function pythonToolReview(tool: StoredToolInvocation): PythonToolReview | null {
  if (tool.toolName !== "run_python" || !isRecord(tool.arguments)) return null;
  const keys = Object.keys(tool.arguments);
  if (keys.length !== 2 || !keys.includes("source") || !keys.includes("purpose")) return null;
  const source = requiredString(tool.arguments.source);
  const purpose = requiredString(tool.arguments.purpose);
  if (
    !source ||
    !purpose ||
    source.includes("\0") ||
    purpose.includes("\0") ||
    new TextEncoder().encode(source).byteLength > MAX_PYTHON_SOURCE_BYTES ||
    Array.from(purpose).length > MAX_PYTHON_PURPOSE_CHARACTERS
  ) {
    return null;
  }
  return { source, purpose };
}

/** Parses only the exact bounded Python audit shapes emitted by the native orchestration seam. */
export function pythonExecutionPresentation(tool: StoredToolInvocation): PythonExecutionPresentation | null {
  if (tool.toolName !== "run_python" || !tool.result) return null;
  if (!pythonToolReview(tool)) return invalidPythonResult();
  const output = tool.result.output;
  if (!isRecord(output) || typeof output.status !== "string") return invalidPythonResult();
  if (output.status === "executed") return executedPythonPresentation(tool, output);
  if (output.status === "denied") {
    if (!exactKeys(output, ["status"]) || !pythonAuditMatches(tool, true, "approval_required", "denied")) {
      return invalidPythonResult();
    }
    return {
      kind: "denied",
      statusLabel: "Not executed",
      message: "The user denied this Python proposal. No code was run.",
    };
  }
  if (output.status === "cancelled") {
    const approval = tool.audit.approval?.decision ?? null;
    if (
      !exactKeys(output, ["status"]) ||
      !pythonAuditMatches(tool, true, "execution_failed", approval) ||
      (approval !== null && approval !== "approved")
    ) {
      return invalidPythonResult();
    }
    return {
      kind: "cancelled",
      statusLabel: "Cancelled",
      message:
        approval === "approved"
          ? "The approved Python execution was cancelled."
          : "The Python proposal was cancelled before approval. No code was run.",
    };
  }
  if (output.status === "failed") return failedPythonPresentation(tool, output);
  return invalidPythonResult();
}

/** Parses the closed executed payload without reflecting unexpected result fields. */
function executedPythonPresentation(
  tool: StoredToolInvocation,
  output: Record<string, unknown>,
): PythonExecutionPresentation {
  if (
    !exactKeys(output, ["status", "result"]) ||
    !pythonAuditMatches(tool, false, "success", "approved") ||
    !isRecord(output.result) ||
    !exactKeys(output.result, ["status", "stdout", "stderr", "durationMs"])
  ) {
    return invalidPythonResult();
  }
  const status = output.result.status;
  const stdout = output.result.stdout;
  const stderr = output.result.stderr;
  const durationMs = output.result.durationMs;
  if (
    typeof status !== "string" ||
    !(status in PYTHON_EXECUTION_STATUS_LABELS) ||
    typeof stdout !== "string" ||
    typeof stderr !== "string" ||
    new TextEncoder().encode(stdout).byteLength > MAX_PYTHON_STREAM_BYTES ||
    new TextEncoder().encode(stderr).byteLength > MAX_PYTHON_STREAM_BYTES ||
    typeof durationMs !== "number" ||
    !Number.isSafeInteger(durationMs) ||
    durationMs < 0
  ) {
    return invalidPythonResult();
  }
  return {
    kind: "executed",
    statusLabel: PYTHON_EXECUTION_STATUS_LABELS[status as keyof typeof PYTHON_EXECUTION_STATUS_LABELS],
    stdout,
    stderr,
    durationLabel: durationLabel(durationMs) ?? "0 ms",
  };
}

/** Maps one exact native failure code to its fixed, path-free explanation. */
function failedPythonPresentation(
  tool: StoredToolInvocation,
  output: Record<string, unknown>,
): PythonExecutionPresentation {
  if (!exactKeys(output, ["status", "code"]) || typeof output.code !== "string" || !(output.code in PYTHON_FAILURES)) {
    return invalidPythonResult();
  }
  const failure = PYTHON_FAILURES[output.code as keyof typeof PYTHON_FAILURES];
  if (!pythonAuditMatches(tool, true, failure.outcome, failure.approval)) return invalidPythonResult();
  return { kind: "failed", statusLabel: failure.statusLabel, message: failure.message };
}

/** Requires the generic audit metadata to agree with the Python-specific terminal payload. */
function pythonAuditMatches(
  tool: StoredToolInvocation,
  isError: boolean,
  outcome: NonNullable<StoredToolInvocation["audit"]["outcome"]>,
  approval: "approved" | "denied" | null,
): boolean {
  return (
    tool.audit.policy === "approval_required" &&
    tool.result?.isError === isError &&
    tool.audit.outcome === outcome &&
    (tool.audit.approval?.decision ?? null) === approval
  );
}

/** Returns the one fixed fallback for malformed or future-shaped Python audit data. */
function invalidPythonResult(): PythonExecutionPresentation {
  return { kind: "invalid", message: INVALID_PYTHON_RESULT_MESSAGE };
}

const OUTCOME_LABELS: Record<NonNullable<StoredToolInvocation["audit"]["outcome"]>, string> = {
  success: "Succeeded",
  unsupported_tool: "Unsupported tool",
  invalid_arguments: "Invalid arguments",
  approval_required: "Approval required",
  unavailable: "Source unavailable",
  execution_failed: "Execution failed",
  output_too_large: "Output too large",
  legacy_error: "Legacy error",
};

/** Maps Bottie's closed native tool names to short user-facing labels. */
export function toolDisplayName(toolName: string): string {
  return TOOL_DISPLAY_NAMES[toolName] ?? toolName;
}

/** Recognizes the exact durable envelope used for explicitly untrusted fetched page text. */
export function untrustedWebResult(tool: StoredToolInvocation): boolean {
  if (tool.toolName !== "web_fetch" || !tool.result || tool.result.isError) return false;
  const output = tool.result.output;
  if (!isRecord(output) || output.ok !== true || !isRecord(output.result)) return false;
  return (
    output.result.untrusted === true &&
    requiredString(output.result.sourceUrl) !== null &&
    requiredString(output.result.content) !== null
  );
}

/** Builds one bounded presentation summary without inspecting argument or result payloads. */
export function toolAuditPresentation(tool: StoredToolInvocation): ToolAuditPresentation {
  const outcome = tool.audit.outcome;
  const status = auditStatus(outcome);
  return {
    status,
    statusLabel: status === "complete" ? "Complete" : status === "blocked" ? "Blocked" : titleCase(status),
    policyLabel: policyLabel(tool.audit.policy),
    approvalLabel:
      tool.audit.approval?.decision === "approved"
        ? "Approved once"
        : tool.audit.approval?.decision === "denied"
          ? "Denied"
          : null,
    outcomeLabel: outcome ? OUTCOME_LABELS[outcome] : "Awaiting result",
    durationLabel: durationLabel(tool.audit.durationMs),
  };
}

/** Summarizes one selected response's ordered call count and exceptional outcomes. */
export function toolActivitySummary(tools: StoredToolInvocation[]): string {
  const attentionCount = tools.filter((tool) => {
    const status = auditStatus(tool.audit.outcome);
    return status === "blocked" || status === "error";
  }).length;
  const calls = `${tools.length} ${tools.length === 1 ? "call" : "calls"}`;
  return attentionCount > 0 ? `${calls} · ${attentionCount} needs attention` : calls;
}

/** Formats one native wall-clock audit timestamp for the user's current locale. */
export function toolAuditTime(createdAtMs: number): string {
  return new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit", second: "2-digit" }).format(
    new Date(createdAtMs),
  );
}

/** Maps a stable outcome to its small visual state. */
function auditStatus(outcome: StoredToolInvocation["audit"]["outcome"]): ToolAuditPresentation["status"] {
  if (outcome === null) return "pending";
  if (outcome === "success") return "complete";
  if (outcome === "approval_required") return "blocked";
  return "error";
}

/** Labels the recorded policy classification without implying arbitrary read access is safe. */
function policyLabel(policy: StoredToolInvocation["audit"]["policy"]): string {
  switch (policy) {
    case "safe":
      return "Read-only";
    case "approval_required":
      return "Approval required";
    case "unregistered":
      return "Unregistered";
    case "legacy":
      return "Legacy record";
  }
}

/** Formats short native execution durations without false precision. */
function durationLabel(durationMs: number | null): string | null {
  if (durationMs === null) return null;
  if (durationMs < 1_000) return `${durationMs} ms`;
  return `${(durationMs / 1_000).toFixed(1)} s`;
}

/** Uppercases one stable lowercase status label. */
function titleCase(value: string): string {
  return `${value.charAt(0).toUpperCase()}${value.slice(1)}`;
}

/** Narrows an unknown durable payload into a non-array object. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Requires one native payload object to contain exactly the expected closed field set. */
function exactKeys(value: Record<string, unknown>, expected: string[]): boolean {
  const keys = Object.keys(value);
  return keys.length === expected.length && expected.every((key) => keys.includes(key));
}

/** Accepts one required non-empty string from the native fetched-page result. */
function requiredString(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}
