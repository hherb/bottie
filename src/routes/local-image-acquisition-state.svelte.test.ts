import { describe, expect, it, vi } from "vitest";

import type { LocalImageAcquisitionStatus } from "$lib/local-image";

import { LocalImageAcquisitionState } from "./local-image-acquisition-state.svelte";

const awaiting: LocalImageAcquisitionStatus = {
  modelId: "Qwen/Qwen-Image-2512",
  packageId: "AbstractFramework/qwen-image-2512-4bit",
  runtimeId: "mlx-gen@fca64a283737c68b67a7bfd88d93f7aa9101a95c",
  license: "Apache-2.0",
  sourceRevision: "423f1f5bf708c6e11eb78881ef9738422cea0814",
  expectedDiskBytes: 17_442_350_812,
  requiredMemoryBytes: 29_526_129_448,
  availability: "model_missing",
  phase: "awaiting_approval",
  failure: null,
  downloadedFiles: 0,
  totalFiles: 18,
  downloadedBytes: 0,
  verifiedFiles: 0,
};

function dependencies(overrides: Record<string, unknown> = {}) {
  return {
    isNative: () => true,
    get: vi.fn().mockResolvedValue(awaiting),
    start: vi.fn().mockResolvedValue({ ...awaiting, phase: "downloading" }),
    cancel: vi.fn().mockResolvedValue({ ...awaiting, phase: "cancelling" }),
    listen: vi.fn().mockResolvedValue(vi.fn()),
    ...overrides,
  };
}

describe("LocalImageAcquisitionState", () => {
  it("subscribes before its read and sends the exact affirmative disclosure", async () => {
    const api = dependencies();
    const state = new LocalImageAcquisitionState(api);

    await state.initialize(vi.fn());
    await state.start();

    expect(api.listen).toHaveBeenCalledOnce();
    expect(api.get).toHaveBeenCalledOnce();
    expect(api.start).toHaveBeenCalledWith({
      modelId: awaiting.modelId,
      packageId: awaiting.packageId,
      runtimeId: awaiting.runtimeId,
      license: awaiting.license,
      sourceRevision: awaiting.sourceRevision,
      expectedDiskBytes: awaiting.expectedDiskBytes,
      requiredMemoryBytes: awaiting.requiredMemoryBytes,
      approved: true,
    });
    expect(state.status?.phase).toBe("downloading");
  });

  it("keeps a new event from being overwritten by a stale startup read and publishes readiness", async () => {
    let publish = (_status: LocalImageAcquisitionStatus): void => {};
    let resolveGet = (_status: LocalImageAcquisitionStatus): void => {};
    const onStatus = vi.fn();
    const api = dependencies({
      get: vi.fn().mockImplementation(
        () =>
          new Promise<LocalImageAcquisitionStatus>((resolve) => {
            resolveGet = resolve;
          }),
      ),
      listen: vi.fn().mockImplementation(async (onStatus) => {
        publish = onStatus;
        return vi.fn();
      }),
    });
    const state = new LocalImageAcquisitionState(api);

    const initialization = state.initialize(onStatus);
    await Promise.resolve();
    publish({
      ...awaiting,
      phase: "ready",
      availability: "ready",
      downloadedFiles: 18,
      downloadedBytes: awaiting.expectedDiskBytes,
    });
    resolveGet(awaiting);
    await initialization;

    expect(state.status?.phase).toBe("ready");
    expect(onStatus).toHaveBeenCalledOnce();
    expect(onStatus).toHaveBeenCalledWith(expect.objectContaining({ phase: "ready", availability: "ready" }));
  });

  it("keeps an observed native event authoritative when the startup read then fails", async () => {
    let publish = (_status: LocalImageAcquisitionStatus): void => {};
    const api = dependencies({
      get: vi.fn().mockRejectedValue(new Error("stale read failed")),
      listen: vi.fn().mockImplementation(async (onStatus) => {
        publish = onStatus;
        return vi.fn();
      }),
    });
    const state = new LocalImageAcquisitionState(api);

    const initialization = state.initialize(vi.fn());
    await Promise.resolve();
    publish({ ...awaiting, phase: "downloading", downloadedBytes: 1 });
    await initialization;

    expect(state.status?.phase).toBe("downloading");
    expect(state.failed).toBe(false);
    expect(state.feedback).toBe("");
  });

  it("maps rejected native detail to fixed feedback and releases its listener", async () => {
    const unlisten = vi.fn();
    const api = dependencies({
      start: vi.fn().mockRejectedValue(new Error("/private/cache/secret")),
      listen: vi.fn().mockResolvedValue(unlisten),
    });
    const state = new LocalImageAcquisitionState(api);

    await state.initialize(vi.fn());
    await state.start();
    state.dispose();

    expect(state.feedback).toBe("The local image model acquisition could not continue safely.");
    expect(state.feedback).not.toContain("/private/cache");
    expect(unlisten).toHaveBeenCalledOnce();
  });
});
