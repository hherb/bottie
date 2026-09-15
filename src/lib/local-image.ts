/** Typed path-free presentation for Rust-owned local-image availability. */

import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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
  workerExpectedDiskBytes: number;
  requiredMemoryBytes: number;
  availability: LocalImageAvailability;
};

/** Closed native lifecycle for explicit installation of the selected local model. */
export type LocalImageAcquisitionPhase =
  "unavailable" | "awaiting_approval" | "downloading" | "cancelling" | "paused" | "verifying" | "ready" | "failed";

/** Stable path-free failure emitted by native acquisition orchestration. */
export type LocalImageAcquisitionFailure =
  "cancelled" | "interrupted" | "timeout" | "transport" | "source_mismatch" | "integrity" | "internal";

/** Complete path-free status for the one reviewed model package. */
export type LocalImageAcquisitionStatus = LocalImageAvailabilityMetadata & {
  phase: LocalImageAcquisitionPhase;
  failure: LocalImageAcquisitionFailure | null;
  downloadedFiles: number;
  totalFiles: number;
  downloadedBytes: number;
  verifiedFiles: number;
};

/** Exact affirmative disclosure sent only from the explicit install or resume control. */
export type LocalImageAcquisitionApproval = Omit<
  LocalImageAcquisitionStatus,
  "availability" | "phase" | "failure" | "downloadedFiles" | "totalFiles" | "downloadedBytes" | "verifiedFiles"
> & { approved: true };

/** Fixed native command rejection without paths, URLs, hashes, or response data. */
export type LocalImageAcquisitionError = {
  code: "unavailable" | "approval_mismatch" | "already_active" | "not_active" | "invalid_state";
  message: string;
};

/** Exact affirmative disclosure sent only from the native worker-import control. */
export type LocalImageWorkerImportApproval = {
  runtimeId: string;
  expectedDiskBytes: number;
  approved: true;
};

/** Path-free result from the Rust-owned folder picker and transactional copy. */
export type LocalImageWorkerImportOutcome = {
  imported: boolean;
  availability: LocalImageAvailabilityMetadata;
};

/** Calm explicit worker import copy derived from native availability only. */
export type LocalImageWorkerImportPresentation = {
  action: "none" | "import";
  active: boolean;
  label: string;
  detail: string;
};

/** Calm progress and action copy derived only from the closed native status. */
export type LocalImageAcquisitionPresentation = {
  action: "none" | "install" | "resume" | "retry" | "cancel";
  active: boolean;
  label: string;
  detail: string;
  percent: number;
};

/** Calm fixed presentation derived from one native readiness response. */
export type LocalImageAvailabilityPresentation = {
  state: "checking" | "ready" | "unavailable";
  label: string;
  detail: string;
};

const GIB_BYTES = 1_024 ** 3;
const LOCAL_IMAGE_ACQUISITION_EVENT = "local-image-acquisition-changed";

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

/** Opens the native folder picker and imports only the exact approved worker bundle. */
export async function importLocalImageWorker(
  approval: LocalImageWorkerImportApproval,
): Promise<LocalImageWorkerImportOutcome> {
  if (!isTauri()) throw new Error("Native local-image worker import is unavailable in browser preview.");
  return invoke<LocalImageWorkerImportOutcome>("import_local_image_worker", { approval });
}

/** Reads current exact resumable acquisition status without creating cache state. */
export async function getLocalImageAcquisitionStatus(): Promise<LocalImageAcquisitionStatus> {
  if (!isTauri()) throw new Error("Native local-image acquisition is unavailable in browser preview.");
  return invoke<LocalImageAcquisitionStatus>("get_local_image_acquisition_status");
}

/** Starts one exact package acquisition after the explicit disclosure action. */
export async function startLocalImageAcquisition(
  approval: LocalImageAcquisitionApproval,
): Promise<LocalImageAcquisitionStatus> {
  if (!isTauri()) throw new Error("Native local-image acquisition is unavailable in browser preview.");
  return invoke<LocalImageAcquisitionStatus>("start_local_image_acquisition", { approval });
}

/** Cancels the active package transfer while retaining only exact synced partial bytes. */
export async function cancelLocalImageAcquisition(): Promise<LocalImageAcquisitionStatus> {
  if (!isTauri()) throw new Error("Native local-image acquisition is unavailable in browser preview.");
  return invoke<LocalImageAcquisitionStatus>("cancel_local_image_acquisition");
}

/** Subscribes to complete path-free acquisition snapshots from native orchestration. */
export async function listenForLocalImageAcquisition(
  onStatus: (status: LocalImageAcquisitionStatus) => void,
): Promise<UnlistenFn> {
  return listen<LocalImageAcquisitionStatus>(LOCAL_IMAGE_ACQUISITION_EVENT, (event) => onStatus(event.payload));
}

/** Echoes every displayed package fact in one affirmative native approval request. */
export function localImageAcquisitionApproval(status: LocalImageAcquisitionStatus): LocalImageAcquisitionApproval {
  return {
    modelId: status.modelId,
    packageId: status.packageId,
    runtimeId: status.runtimeId,
    license: status.license,
    sourceRevision: status.sourceRevision,
    expectedDiskBytes: status.expectedDiskBytes,
    workerExpectedDiskBytes: status.workerExpectedDiskBytes,
    requiredMemoryBytes: status.requiredMemoryBytes,
    approved: true,
  };
}

/** Binds one affirmative worker-import action to the exact displayed runtime and bundle size. */
export function localImageWorkerImportApproval(
  metadata: LocalImageAvailabilityMetadata,
): LocalImageWorkerImportApproval {
  return {
    runtimeId: metadata.runtimeId,
    expectedDiskBytes: metadata.workerExpectedDiskBytes,
    approved: true,
  };
}

/** Offers worker import only while the exact promoted worker is missing or mismatched. */
export function localImageWorkerImportPresentation(
  metadata: LocalImageAvailabilityMetadata | null,
  active: boolean,
): LocalImageWorkerImportPresentation {
  if (active) {
    return {
      action: "none",
      active: true,
      label: "Verifying and installing worker…",
      detail: "The selected folder stays native and is re-verified before atomic activation.",
    };
  }
  if (metadata && ["worker_missing", "worker_mismatch"].includes(metadata.availability)) {
    return {
      action: "import",
      active: false,
      label: `Select and install ${formatGib(metadata.workerExpectedDiskBytes)} worker`,
      detail: "Bottie will copy only the exact reviewed MLX-Gen bundle into its app-owned cache.",
    };
  }
  return { action: "none", active: false, label: "", detail: "" };
}

/** Derives bounded install, resume, cancel, and failure presentation from native status. */
export function localImageAcquisitionPresentation(
  status: LocalImageAcquisitionStatus,
): LocalImageAcquisitionPresentation {
  const percent = boundedPercent(status.downloadedBytes, status.expectedDiskBytes);
  const retained = `${formatGibValue(status.downloadedBytes)} of ${formatGib(status.expectedDiskBytes)} retained`;
  switch (status.phase) {
    case "awaiting_approval":
      return {
        action: "install",
        active: false,
        label: `Download and install ${formatGib(status.expectedDiskBytes)}`,
        detail: `${status.totalFiles} exact files will be verified before activation.`,
        percent,
      };
    case "downloading":
      return {
        action: "cancel",
        active: true,
        label: `Downloading model… ${percent}%`,
        detail: `${status.downloadedFiles} of ${status.totalFiles} files · ${retained}`,
        percent,
      };
    case "cancelling":
      return { action: "none", active: true, label: "Cancelling download…", detail: retained, percent };
    case "paused":
      return {
        action: "resume",
        active: false,
        label: "Resume retained download",
        detail: `${failureLabel(status.failure)} · ${retained}`,
        percent,
      };
    case "verifying":
      return {
        action: "none",
        active: true,
        label: "Verifying exact model files…",
        detail: "Activation remains unavailable until atomic promotion completes.",
        percent: 100,
      };
    case "ready":
      return {
        action: "none",
        active: false,
        label: "Local model is ready",
        detail: `${status.verifiedFiles} exact files verified.`,
        percent: 100,
      };
    case "failed":
      return {
        action: "retry",
        active: false,
        label: "Retry exact package",
        detail: failureDetail(status.failure),
        percent,
      };
    case "unavailable":
      return {
        action: "none",
        active: false,
        label: "Installation prerequisites are unavailable",
        detail: "The exact hardware and worker must pass native verification first.",
        percent,
      };
  }
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
  return `${formatGibValue(bytes)} GiB`;
}

function formatGibValue(bytes: number): string {
  return (bytes / GIB_BYTES).toFixed(1);
}

function boundedPercent(completed: number, total: number): number {
  if (!Number.isFinite(completed) || !Number.isFinite(total) || total <= 0) return 0;
  return Math.max(0, Math.min(100, Math.floor((completed / total) * 100)));
}

function failureLabel(failure: LocalImageAcquisitionFailure | null): string {
  if (failure === "cancelled") return "Cancelled";
  if (failure === "timeout") return "Timed out";
  if (failure === "transport") return "Connection interrupted";
  if (failure === "interrupted") return "Download interrupted";
  return "Ready to resume";
}

function failureDetail(failure: LocalImageAcquisitionFailure | null): string {
  if (failure === "source_mismatch") return "The approved source changed or returned unexpected metadata.";
  if (failure === "integrity") return "Exact cache integrity verification failed closed.";
  return "The model acquisition could not continue safely.";
}
