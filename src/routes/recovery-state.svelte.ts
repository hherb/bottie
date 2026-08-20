/** Reactive presentation state for native local-data integrity and guided recovery. */

import { isTauri } from "@tauri-apps/api/core";

import { getStorageRecoveryStatus, type StorageRecoveryStatus } from "$lib/storage";

import type { ConversationState } from "./conversation-state.svelte";

/** Owns startup integrity status and coordinates restoration before normal conversation access resumes. */
export class RecoveryState {
  status = $state<StorageRecoveryStatus | null>(
    isTauri() ? null : { state: "ready", automaticBackupCount: 0, latestAutomaticBackupAtMs: null },
  );

  /** Loads path-redacted native integrity state and reports whether normal startup may continue. */
  async initialize(): Promise<boolean> {
    try {
      this.status = await getStorageRecoveryStatus();
      return this.status.state === "ready";
    } catch (error) {
      console.warn("Could not read local data recovery status", error);
      return true;
    }
  }

  /** Restores one guided recovery point and confirms that native conversation access resumed. */
  async restore(
    history: ConversationState,
    source: "manual" | "automatic",
  ): Promise<Awaited<ReturnType<ConversationState["restoreBackup"]>>> {
    const messages = await history.restoreBackup(source);
    if (messages === null) return null;
    this.status = await getStorageRecoveryStatus();
    return this.status.state === "ready" ? messages : null;
  }
}
