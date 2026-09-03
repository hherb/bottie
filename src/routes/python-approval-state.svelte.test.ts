import { describe, expect, it, vi } from "vitest";

import { PythonApprovalState } from "./python-approval-state.svelte";

const pending = {
  requestId: "opaque-native-token",
  phase: "pending" as const,
  source: "print(4)",
  purpose: "Calculate two plus two.",
};

describe("PythonApprovalState", () => {
  it("loads native pending state and sends only its opaque token plus one decision", async () => {
    const get = vi.fn().mockResolvedValue(pending);
    const decide = vi.fn().mockResolvedValue({ ...pending, phase: "approved" });
    const state = new PythonApprovalState({ get, decide });

    await state.initialize();
    await state.decide("approve");

    expect(get).toHaveBeenCalledOnce();
    expect(decide).toHaveBeenCalledWith("opaque-native-token", "approve");
    expect(state.approval?.phase).toBe("approved");
    expect(state.error).toBe("");
  });

  it("keeps the pending review visible with fixed feedback when a decision fails", async () => {
    const state = new PythonApprovalState({
      get: vi.fn().mockResolvedValue(pending),
      decide: vi.fn().mockRejectedValue(new Error("private native detail")),
    });

    await state.initialize();
    await state.decide("deny");

    expect(state.approval).toEqual(pending);
    expect(state.error).toBe("Bottie could not record that decision. Review the request and try again.");
    expect(state.error).not.toContain("private native detail");
    expect(state.busy).toBe(false);
  });
});
