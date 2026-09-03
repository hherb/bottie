/** Development-only fixture for the approval-required Python proposal review. */

import type { Message } from "$lib/presentation";
import type { PageState } from "./page-state.svelte";

const PYTHON_APPROVAL_PREVIEW_VALUE = "approval-review";

/** Reports whether a query explicitly requests the bounded Python review fixture. */
export function pythonApprovalPreviewRequested(search: string): boolean {
  return new URLSearchParams(search).get("python") === PYTHON_APPROVAL_PREVIEW_VALUE;
}

/** Applies one inert, path-free Python proposal to the disconnected browser preview. */
export function applyPythonApprovalPreview(state: PageState, search: string): boolean {
  if (!pythonApprovalPreviewRequested(search)) return false;
  const messages: Message[] = [
    {
      id: 1,
      role: "user",
      content: "Calculate the median and spread of these values: 7, 11, 13, 18, 21.",
    },
    {
      id: 2,
      role: "assistant",
      featured: true,
      model: "Python approval preview",
      content: "I prepared a bounded Python proposal for review. No code has run.",
      toolInvocations: [
        {
          ordinal: 0,
          toolName: "run_python",
          arguments: {
            source: [
              "from statistics import median, pstdev",
              "values = [7, 11, 13, 18, 21]",
              "print({'median': median(values), 'spread': pstdev(values)})",
            ].join("\n"),
            purpose: "Calculate the requested median and population standard deviation exactly.",
          },
          audit: { policy: "approval_required", outcome: "approval_required", durationMs: 0 },
          result: {
            output: {
              ok: false,
              error: { code: "approval_required", message: "Approval is required before Python can run." },
            },
            isError: true,
            createdAtMs: 1_776_000_001_000,
          },
          createdAtMs: 1_776_000_000_000,
        },
      ],
    },
  ];
  state.messages = messages;
  return true;
}
