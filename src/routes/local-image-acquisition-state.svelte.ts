/** Reactive path-free state for the native local image-model acquisition coordinator. */

import { isTauri } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";

import {
  cancelLocalImageAcquisition,
  getLocalImageAcquisitionStatus,
  listenForLocalImageAcquisition,
  localImageAcquisitionApproval,
  startLocalImageAcquisition,
  type LocalImageAcquisitionError,
  type LocalImageAcquisitionStatus,
} from "$lib/local-image";

type LocalImageAcquisitionDependencies = {
  isNative: () => boolean;
  get: typeof getLocalImageAcquisitionStatus;
  start: typeof startLocalImageAcquisition;
  cancel: typeof cancelLocalImageAcquisition;
  listen: typeof listenForLocalImageAcquisition;
};

const DEFAULT_DEPENDENCIES: LocalImageAcquisitionDependencies = {
  isNative: isTauri,
  get: getLocalImageAcquisitionStatus,
  start: startLocalImageAcquisition,
  cancel: cancelLocalImageAcquisition,
  listen: listenForLocalImageAcquisition,
};

/** Owns acquisition event listening and exact explicit install, resume, and cancel actions. */
export class LocalImageAcquisitionState {
  status = $state<LocalImageAcquisitionStatus | null>(null);
  feedback = $state("");
  failed = $state(false);

  private stopListening: UnlistenFn | null = null;
  private onStatus: ((status: LocalImageAcquisitionStatus) => void) | null = null;

  constructor(private readonly dependencies: LocalImageAcquisitionDependencies = DEFAULT_DEPENDENCIES) {
    this.failed = !dependencies.isNative();
  }

  /** Subscribes before reading the current native snapshot so an active transition is not missed. */
  async initialize(onStatus: (status: LocalImageAcquisitionStatus) => void): Promise<void> {
    if (!this.dependencies.isNative()) return;
    this.onStatus = onStatus;
    let eventObserved = false;
    try {
      this.stopListening = await this.dependencies.listen((status) => {
        eventObserved = true;
        this.apply(status);
      });
      const status = await this.dependencies.get();
      if (!eventObserved) this.apply(status);
      this.failed = false;
    } catch (error) {
      if (eventObserved) return;
      this.failed = true;
      this.feedback = errorMessage(error);
      console.warn("Could not read local image acquisition status", error);
    }
  }

  /** Starts or resumes only the exact package currently disclosed by the native status. */
  async start(): Promise<void> {
    if (!this.status || !["awaiting_approval", "paused", "failed"].includes(this.status.phase)) return;
    this.feedback = "";
    try {
      this.apply(await this.dependencies.start(localImageAcquisitionApproval(this.status)));
    } catch (error) {
      this.feedback = errorMessage(error);
    }
  }

  /** Requests cancellation of only the currently active native package transfer. */
  async cancel(): Promise<void> {
    if (!this.status || this.status.phase !== "downloading") return;
    this.feedback = "";
    try {
      this.apply(await this.dependencies.cancel());
    } catch (error) {
      this.feedback = errorMessage(error);
    }
  }

  /** Releases the fixed native event subscription when the page is unmounted. */
  dispose(): void {
    this.stopListening?.();
    this.stopListening = null;
    this.onStatus = null;
  }

  private apply(status: LocalImageAcquisitionStatus): void {
    this.status = status;
    this.failed = false;
    this.onStatus?.(status);
  }
}

function errorMessage(error: unknown): string {
  const candidate = error as Partial<LocalImageAcquisitionError> | null;
  if (candidate?.code === "unavailable") return "The exact local worker and hardware prerequisites are not ready.";
  if (candidate?.code === "approval_mismatch") return "Review the current package disclosure before installing.";
  if (candidate?.code === "already_active") return "A local image model download is already active.";
  if (candidate?.code === "not_active") return "No local image model download is active.";
  return "The local image model acquisition could not continue safely.";
}
