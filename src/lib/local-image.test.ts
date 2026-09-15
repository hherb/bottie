import { describe, expect, it } from "vitest";

import {
  localImageAcquisitionApproval,
  localImageAcquisitionPresentation,
  localImageAvailabilityPresentation,
  localImageWorkerImportApproval,
  localImageWorkerImportPresentation,
  type LocalImageAcquisitionStatus,
  type LocalImageAvailabilityMetadata,
} from "./local-image";

const READY: LocalImageAvailabilityMetadata = {
  modelId: "Qwen/Qwen-Image-2512",
  packageId: "AbstractFramework/qwen-image-2512-4bit",
  runtimeId: "mlx-gen@fca64a283737c68b67a7bfd88d93f7aa9101a95c",
  license: "Apache-2.0",
  sourceRevision: "423f1f5bf708c6e11eb78881ef9738422cea0814",
  expectedDiskBytes: 17_442_350_812,
  workerExpectedDiskBytes: 1_107_880_778,
  requiredMemoryBytes: 29_526_129_448,
  availability: "ready",
};

describe("local image availability presentation", () => {
  it("presents exact package identity and measured byte requirements without claiming hosted 2.0", () => {
    const presentation = localImageAvailabilityPresentation(READY);

    expect(presentation).toEqual({
      state: "ready",
      label: "Ready for local integration",
      detail: "16.2 GiB model · 27.5 GiB measured peak memory",
    });
    expect(`${READY.modelId} ${READY.packageId}`).toContain("Qwen-Image-2512");
    expect(`${READY.modelId} ${READY.packageId}`).not.toContain("2.0");
  });

  it.each([
    ["unsupported_platform", "Unavailable on this operating system"],
    ["unsupported_architecture", "Unavailable on this processor architecture"],
    ["insufficient_memory", "Insufficient physical memory"],
    ["unsupported_hardware", "This exact hardware has not been proved"],
    ["worker_missing", "Local worker is not installed"],
    ["worker_mismatch", "Local worker integrity check failed"],
    ["model_missing", "Local model is not installed"],
    ["model_mismatch", "Local model integrity check failed"],
  ] as const)("maps %s to one fixed fail-closed reason", (availability, label) => {
    expect(localImageAvailabilityPresentation({ ...READY, availability })).toEqual({
      state: "unavailable",
      label,
      detail: "16.2 GiB model · 27.5 GiB measured peak memory",
    });
  });

  it("distinguishes loading and native inspection failure without inventing package state", () => {
    expect(localImageAvailabilityPresentation(null)).toEqual({
      state: "checking",
      label: "Checking local availability…",
      detail: "Native installation inspection is in progress",
    });
    expect(localImageAvailabilityPresentation(null, true)).toEqual({
      state: "unavailable",
      label: "Local availability could not be verified",
      detail: "The Cloud action remains available when configured",
    });
  });
});

describe("local image worker import presentation", () => {
  it("binds approval to the displayed runtime and exact worker bytes", () => {
    expect(localImageWorkerImportApproval({ ...READY, availability: "worker_missing" })).toEqual({
      runtimeId: READY.runtimeId,
      expectedDiskBytes: READY.workerExpectedDiskBytes,
      approved: true,
    });
  });

  it("offers native selection only for missing or mismatched worker bytes", () => {
    expect(localImageWorkerImportPresentation({ ...READY, availability: "worker_missing" }, false)).toEqual({
      action: "import",
      active: false,
      label: "Select and install 1.0 GiB worker",
      detail: "Bottie will copy only the exact reviewed MLX-Gen bundle into its app-owned cache.",
    });
    expect(localImageWorkerImportPresentation({ ...READY, availability: "worker_mismatch" }, true)).toEqual({
      action: "none",
      active: true,
      label: "Verifying and installing worker…",
      detail: "The selected folder stays native and is re-verified before atomic activation.",
    });
    expect(localImageWorkerImportPresentation(READY, false).action).toBe("none");
  });
});

describe("local image acquisition presentation", () => {
  const AWAITING: LocalImageAcquisitionStatus = {
    modelId: READY.modelId,
    packageId: READY.packageId,
    runtimeId: READY.runtimeId,
    license: READY.license,
    sourceRevision: READY.sourceRevision,
    expectedDiskBytes: READY.expectedDiskBytes,
    workerExpectedDiskBytes: READY.workerExpectedDiskBytes,
    requiredMemoryBytes: READY.requiredMemoryBytes,
    availability: "model_missing",
    phase: "awaiting_approval",
    failure: null,
    downloadedFiles: 0,
    totalFiles: 18,
    downloadedBytes: 0,
    verifiedFiles: 0,
  };

  it("builds an affirmative request bound to every displayed package fact", () => {
    expect(localImageAcquisitionApproval(AWAITING)).toEqual({
      modelId: READY.modelId,
      packageId: READY.packageId,
      runtimeId: READY.runtimeId,
      license: READY.license,
      sourceRevision: READY.sourceRevision,
      expectedDiskBytes: READY.expectedDiskBytes,
      workerExpectedDiskBytes: READY.workerExpectedDiskBytes,
      requiredMemoryBytes: READY.requiredMemoryBytes,
      approved: true,
    });
  });

  it("presents initial approval, progress, cancellation, and resumable state with exact totals", () => {
    expect(localImageAcquisitionPresentation(AWAITING)).toEqual({
      action: "install",
      active: false,
      label: "Download and install 16.2 GiB",
      detail: "18 exact files will be verified before activation.",
      percent: 0,
    });
    expect(
      localImageAcquisitionPresentation({
        ...AWAITING,
        phase: "downloading",
        downloadedFiles: 4,
        downloadedBytes: 4_360_587_703,
      }),
    ).toEqual({
      action: "cancel",
      active: true,
      label: "Downloading model… 25%",
      detail: "4 of 18 files · 4.1 of 16.2 GiB retained",
      percent: 25,
    });
    expect(
      localImageAcquisitionPresentation({
        ...AWAITING,
        phase: "paused",
        failure: "cancelled",
        downloadedFiles: 4,
        downloadedBytes: 4_360_587_703,
      }),
    ).toEqual({
      action: "resume",
      active: false,
      label: "Resume retained download",
      detail: "Cancelled · 4.1 of 16.2 GiB retained",
      percent: 25,
    });
  });

  it("keeps native failures fixed and does not offer installation when prerequisites are unavailable", () => {
    expect(localImageAcquisitionPresentation({ ...AWAITING, phase: "failed", failure: "source_mismatch" })).toEqual({
      action: "retry",
      active: false,
      label: "Retry exact package",
      detail: "The approved source changed or returned unexpected metadata.",
      percent: 0,
    });
    expect(localImageAcquisitionPresentation({ ...AWAITING, phase: "unavailable" }).action).toBe("none");
  });
});
