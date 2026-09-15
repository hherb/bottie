import { describe, expect, it, vi } from "vitest";

import type { LocalImageAvailabilityMetadata } from "$lib/local-image";

import { LocalImageWorkerImportState } from "./local-image-worker-import-state.svelte";

const missing: LocalImageAvailabilityMetadata = {
  modelId: "Qwen/Qwen-Image-2512",
  packageId: "AbstractFramework/qwen-image-2512-4bit",
  runtimeId: "mlx-gen@fca64a283737c68b67a7bfd88d93f7aa9101a95c",
  license: "Apache-2.0",
  sourceRevision: "423f1f5bf708c6e11eb78881ef9738422cea0814",
  expectedDiskBytes: 17_442_350_812,
  workerExpectedDiskBytes: 1_107_880_778,
  requiredMemoryBytes: 29_526_129_448,
  availability: "worker_missing",
};

describe("LocalImageWorkerImportState", () => {
  it("sends exact approval and refreshes model acquisition after promotion", async () => {
    const availability = { ...missing, availability: "model_missing" as const };
    const invokeImport = vi.fn().mockResolvedValue({ imported: true, availability });
    const refresh = vi.fn().mockResolvedValue(undefined);
    const apply = vi.fn();
    const state = new LocalImageWorkerImportState({ importWorker: invokeImport });

    await state.start(missing, apply, refresh);

    expect(invokeImport).toHaveBeenCalledWith({
      runtimeId: missing.runtimeId,
      expectedDiskBytes: missing.workerExpectedDiskBytes,
      approved: true,
    });
    expect(apply).toHaveBeenCalledWith(availability);
    expect(refresh).toHaveBeenCalledOnce();
    expect(state.feedback).toBe("Exact worker installed in Bottie’s private cache.");
    expect(state.active).toBe(false);
  });

  it("keeps cancellation and integrity failure path-free", async () => {
    const cancelled = new LocalImageWorkerImportState({
      importWorker: vi.fn().mockResolvedValue({ imported: false, availability: missing }),
    });
    await cancelled.start(missing, vi.fn(), vi.fn());
    expect(cancelled.feedback).toBe("Worker selection cancelled.");

    const failed = new LocalImageWorkerImportState({
      importWorker: vi.fn().mockRejectedValue("integrity"),
    });
    await failed.start(missing, vi.fn(), vi.fn());
    expect(failed.feedback).toBe("The selected folder did not match the exact reviewed worker bundle.");
    expect(failed.feedback).not.toContain("/");
  });

  it("does nothing unless worker readiness permits import", async () => {
    const invokeImport = vi.fn();
    const state = new LocalImageWorkerImportState({ importWorker: invokeImport });

    await state.start({ ...missing, availability: "ready" }, vi.fn(), vi.fn());
    expect(invokeImport).not.toHaveBeenCalled();
  });
});
