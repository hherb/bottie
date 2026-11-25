/** Reactive presentation state for native local-data integrity and guided recovery. */

import { isTauri } from "@tauri-apps/api/core";

import { getStorageRecoveryStatus, storageErrorFromUnknown, type StorageRecoveryStatus } from "$lib/storage";

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

  /**
   * Restores one guided recovery point and confirms that native conversation access resumed. Never rejects:
   * the caller runs it with `void`, so a failed status check is shown on the recovery screen instead (#235).
   */
  async restore(
    history: ConversationState,
    source: "manual" | "automatic",
  ): Promise<Awaited<ReturnType<ConversationState["restoreBackup"]>>> {
    const messages = await history.restoreBackup(source);
    if (messages === null) return null;
    try {
      this.status = await getStorageRecoveryStatus();
    } catch (error) {
      // The data was already replaced, so keep the restore message and add why the screen stayed.
      console.warn("Could not re-check local data after restore", error);
      const reason = storageErrorFromUnknown(error).message;
      history.backupFeedback = `${history.backupFeedback ?? "Restored local data"} · Checking the restored data failed: ${reason}. Relaunch Bottie to check again.`;
      history.backupWarning = true;
      return null;
    }
    return this.status.state === "ready" ? messages : null;
  }
}
