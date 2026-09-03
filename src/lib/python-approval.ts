/** Typed path-free WebView contract for one Rust-owned Python approval decision. */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Fixed native event carrying a newly published review or cancellation removal. */
export const PYTHON_APPROVAL_EVENT = "python-approval-changed";

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

/** Listens for bounded native approval lifecycle updates published after WebView startup. */
export async function listenForPythonApproval(
  onApproval: (approval: PythonApprovalStatus | null) => void,
): Promise<UnlistenFn> {
  return listen<PythonApprovalStatus | null>(PYTHON_APPROVAL_EVENT, (event) => onApproval(event.payload));
}

/** Submits only the opaque native token and one closed explicit decision. */
export async function decidePythonApproval(
  requestId: string,
  decision: PythonApprovalDecision,
): Promise<PythonApprovalStatus> {
  return invoke<PythonApprovalStatus>("decide_python_approval", { request: { requestId, decision } });
}
