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
    const listen = vi.fn().mockResolvedValue(vi.fn());
    const state = new PythonApprovalState({ get, decide, listen });

    await state.initialize();
    await state.decide("approve");

    expect(get).toHaveBeenCalledOnce();
    expect(listen).toHaveBeenCalledOnce();
    expect(decide).toHaveBeenCalledWith("opaque-native-token", "approve");
    expect(state.approval?.phase).toBe("approved");
    expect(state.error).toBe("");
  });

  it("keeps the pending review visible with fixed feedback when a decision fails", async () => {
    const state = new PythonApprovalState({
      get: vi.fn().mockResolvedValue(pending),
      decide: vi.fn().mockRejectedValue(new Error("private native detail")),
      listen: vi.fn().mockResolvedValue(vi.fn()),
    });

    await state.initialize();
    await state.decide("deny");

    expect(state.approval).toEqual(pending);
    expect(state.error).toBe("Bottie could not record that decision. Review the request and try again.");
    expect(state.error).not.toContain("private native detail");
    expect(state.busy).toBe(false);
  });

  it("shows a generation-time published review and releases its listener", async () => {
    let publish = (_approval: typeof pending | null): void => {};
    const unlisten = vi.fn();
    const state = new PythonApprovalState({
      get: vi.fn().mockResolvedValue(null),
      decide: vi.fn(),
      listen: vi.fn().mockImplementation(async (onApproval) => {
        publish = onApproval;
        return unlisten;
      }),
    });

    await state.initialize();
    publish(pending);
    expect(state.approval).toEqual(pending);
    expect(state.error).toBe("");

    publish(null);
    expect(state.approval).toBeNull();
    state.dispose();
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("does not let a stale startup read overwrite a newly published review", async () => {
    let resolveGet = (_approval: typeof pending | null): void => {};
    let publish = (_approval: typeof pending | null): void => {};
    const state = new PythonApprovalState({
      get: vi.fn().mockImplementation(
        () =>
          new Promise<typeof pending | null>((resolve) => {
            resolveGet = resolve;
          }),
      ),
      decide: vi.fn(),
      listen: vi.fn().mockImplementation(async (onApproval) => {
        publish = onApproval;
        return vi.fn();
      }),
    });

    const initialization = state.initialize();
    await Promise.resolve();
    publish(pending);
    resolveGet(null);
    await initialization;

    expect(state.approval).toEqual(pending);
  });
});
