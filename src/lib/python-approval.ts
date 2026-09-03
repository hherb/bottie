/** Typed path-free WebView contract for one Rust-owned Python approval decision. */

import { invoke } from "@tauri-apps/api/core";

/** Exact decision accepted once for the current native Python proposal. */
export type PythonApprovalDecision = "approve" | "deny";

/** User-visible lifecycle without provider call identity or native paths. */
export type PythonApprovalPhase = "pending" | "approved" | "denied";

/** Complete bounded proposal plus one process-local opaque decision token. */
export type PythonApprovalStatus = {
  requestId: string;
  phase: PythonApprovalPhase;
  source: string;
  purpose: string;
};

/** Returns the current process-local Python proposal, when one awaits or has received a decision. */
export async function getPythonApproval(): Promise<PythonApprovalStatus | null> {
  return invoke<PythonApprovalStatus | null>("get_python_approval");
}

/** Submits only the opaque native token and one closed explicit decision. */
export async function decidePythonApproval(
  requestId: string,
  decision: PythonApprovalDecision,
): Promise<PythonApprovalStatus> {
  return invoke<PythonApprovalStatus>("decide_python_approval", { request: { requestId, decision } });
}
