import { beforeEach, describe, expect, it, vi } from "vitest";

const storage = vi.hoisted(() => ({ getStorageRecoveryStatus: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true }));
vi.mock("$lib/storage", async (importOriginal) => ({
  ...(await importOriginal<typeof import("$lib/storage")>()),
  getStorageRecoveryStatus: storage.getStorageRecoveryStatus,
}));

import type { ConversationState } from "./conversation-state.svelte";
import { RecoveryState } from "./recovery-state.svelte";

/** The slice of ConversationState that `RecoveryState.restore` reads and writes. */
function history(): ConversationState {
  return {
    restoreBackup: vi.fn().mockResolvedValue([]),
    backupFeedback: "Restored backup.sqlite3 · preserved current data as copy.sqlite3",
    backupWarning: false,
    restoreReloadFailed: false,
  } as unknown as ConversationState;
}

describe("RecoveryState.restore", () => {
  beforeEach(() => {
    storage.getStorageRecoveryStatus.mockReset();
  });

  it("returns the restored messages once the store reads as ready", async () => {
    storage.getStorageRecoveryStatus.mockResolvedValue({
      state: "ready",
      automaticBackupCount: 1,
      latestAutomaticBackupAtMs: 1,
    });
    const recovery = new RecoveryState();

    await expect(recovery.restore(history(), "automatic")).resolves.toEqual([]);
    expect(recovery.status?.state).toBe("ready");
    expect(recovery.recheckFailed).toBe(false);
  });

  it("keeps the screen and warns when the post-restore check fails, instead of rejecting (#235)", async () => {
    storage.getStorageRecoveryStatus.mockRejectedValue(new Error("bridge down."));
    vi.spyOn(console, "warn").mockImplementation(() => {});
    const recovery = new RecoveryState();
    const conversations = history();

    await expect(recovery.restore(conversations, "manual")).resolves.toBeNull();
    expect(conversations.backupWarning).toBe(true);
    expect(conversations.backupFeedback).toContain("Restored backup.sqlite3");
    expect(conversations.backupFeedback).toContain("Checking the restored data failed: bridge down. Relaunch");
    expect(recovery.recheckFailed).toBe(true);
  });

  it("locks restore when the data was replaced but reloading it failed (#254)", async () => {
    const conversations = history();
    vi.mocked(conversations.restoreBackup).mockImplementation(async () => {
      conversations.restoreReloadFailed = true;
      return null;
    });
    const recovery = new RecoveryState();

    await expect(recovery.restore(conversations, "manual")).resolves.toBeNull();
    expect(recovery.recheckFailed).toBe(true);
    expect(storage.getStorageRecoveryStatus).not.toHaveBeenCalled();
  });

  it("leaves restore enabled when the restore stopped before replacing data", async () => {
    const conversations = history();
    vi.mocked(conversations.restoreBackup).mockResolvedValue(null);
    const recovery = new RecoveryState();

    await expect(recovery.restore(conversations, "manual")).resolves.toBeNull();
    expect(recovery.recheckFailed).toBe(false);
  });
});
