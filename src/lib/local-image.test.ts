import { describe, expect, it } from "vitest";

import { localImageAvailabilityPresentation, type LocalImageAvailabilityMetadata } from "./local-image";

const READY: LocalImageAvailabilityMetadata = {
  modelId: "Qwen/Qwen-Image-2512",
  packageId: "AbstractFramework/qwen-image-2512-4bit",
  runtimeId: "mlx-gen@fca64a283737c68b67a7bfd88d93f7aa9101a95c",
  license: "Apache-2.0",
  sourceRevision: "423f1f5bf708c6e11eb78881ef9738422cea0814",
  expectedDiskBytes: 17_442_350_812,
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
