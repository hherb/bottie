import { describe, expect, it } from "vitest";

import { PageState } from "./page-state.svelte";
import { applyPythonApprovalPreview, pythonApprovalPreviewRequested } from "./python-preview";

describe("Python approval preview", () => {
  it("enables only the explicit bounded review fixture", () => {
    expect(pythonApprovalPreviewRequested("?python=approval-review")).toBe(true);
    expect(pythonApprovalPreviewRequested("?python=execute")).toBe(false);
    expect(pythonApprovalPreviewRequested("")).toBe(false);
  });

  it("adds one interactive proposal without enabling native inference", async () => {
    const state = new PageState();

    expect(applyPythonApprovalPreview(state, "?python=approval-review")).toBe(true);
    expect(state.providerStatus).toBe("browser");
    expect(state.messages).toHaveLength(2);
    expect(state.messages[1]?.toolInvocations).toHaveLength(1);
    expect(state.messages[1]?.toolInvocations?.[0]).toMatchObject({
      toolName: "run_python",
      audit: { policy: "approval_required", approval: null, outcome: "approval_required", durationMs: 0 },
    });
    expect(state.pythonApproval.approval).toMatchObject({
      requestId: "development-preview-token",
      phase: "pending",
    });
    await state.pythonApproval.decide("approve");
    expect(state.pythonApproval.approval?.phase).toBe("approved");
  });
});
