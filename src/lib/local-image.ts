/** Typed path-free presentation for Rust-owned local-image availability. */

import { invoke, isTauri } from "@tauri-apps/api/core";

/** Closed fail-closed readiness states emitted by the native local-image service. */
export type LocalImageAvailability =
  | "unsupported_platform"
  | "unsupported_architecture"
  | "insufficient_memory"
  | "unsupported_hardware"
  | "worker_missing"
  | "worker_mismatch"
  | "model_missing"
  | "model_mismatch"
  | "ready";

/** Exact selected-package disclosure without native paths, hashes, or hardware identity. */
export type LocalImageAvailabilityMetadata = {
  modelId: string;
  packageId: string;
  runtimeId: string;
  license: string;
  sourceRevision: string;
  expectedDiskBytes: number;
  requiredMemoryBytes: number;
  availability: LocalImageAvailability;
};

/** Calm fixed presentation derived from one native readiness response. */
export type LocalImageAvailabilityPresentation = {
  state: "checking" | "ready" | "unavailable";
  label: string;
  detail: string;
};

const GIB_BYTES = 1_024 ** 3;

const UNAVAILABLE_LABELS: Record<Exclude<LocalImageAvailability, "ready">, string> = {
  unsupported_platform: "Unavailable on this operating system",
  unsupported_architecture: "Unavailable on this processor architecture",
  insufficient_memory: "Insufficient physical memory",
  unsupported_hardware: "This exact hardware has not been proved",
  worker_missing: "Local worker is not installed",
  worker_mismatch: "Local worker integrity check failed",
  model_missing: "Local model is not installed",
  model_mismatch: "Local model integrity check failed",
};

/** Reads the selected local-image package and installation readiness from native code. */
export async function getLocalImageAvailability(): Promise<LocalImageAvailabilityMetadata> {
  if (!isTauri()) throw new Error("Native local-image availability is unavailable in browser preview.");
  return invoke<LocalImageAvailabilityMetadata>("get_local_image_availability");
}

/** Maps native state to fixed copy while preserving exact byte requirements. */
export function localImageAvailabilityPresentation(
  metadata: LocalImageAvailabilityMetadata | null,
  failed = false,
): LocalImageAvailabilityPresentation {
  if (!metadata) {
    return failed
      ? {
          state: "unavailable",
          label: "Local availability could not be verified",
          detail: "The Cloud action remains available when configured",
        }
      : {
          state: "checking",
          label: "Checking local availability…",
          detail: "Native installation inspection is in progress",
        };
  }
  const disk = formatGib(metadata.expectedDiskBytes);
  const memory = formatGib(metadata.requiredMemoryBytes);
  const detail = `${disk} model · ${memory} measured peak memory`;
  if (metadata.availability === "ready") {
    return { state: "ready", label: "Ready for local integration", detail };
  }
  return {
    state: "unavailable",
    label: UNAVAILABLE_LABELS[metadata.availability],
    detail,
  };
}

function formatGib(bytes: number): string {
  return `${(bytes / GIB_BYTES).toFixed(1)} GiB`;
}
