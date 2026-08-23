/** Pure presentation helpers for native-owned durable tool audit records. */

import type { StoredToolInvocation } from "./storage";

/** Calm UI state derived from one structured native audit record. */
export type ToolAuditPresentation = {
  status: "pending" | "complete" | "blocked" | "error";
  statusLabel: string;
  policyLabel: string;
  outcomeLabel: string;
  durationLabel: string | null;
};

const TOOL_DISPLAY_NAMES: Record<string, string> = {
  search_memory: "Search conversations",
  open_memory: "Open conversation context",
  search_attached_files: "Search attached files",
};

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

/** Accepts one required non-empty string from the native fetched-page result. */
function requiredString(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}
