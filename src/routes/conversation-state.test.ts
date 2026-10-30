import { describe, expect, it, vi } from "vitest";

import type { StoredConversation } from "$lib/storage";

import { ConversationState } from "./conversation-state.svelte";

const storage = vi.hoisted(() => ({
  loadConversation: vi.fn(),
  clearLastOpenConversation: vi.fn(),
  deleteConversation: vi.fn(),
  setConversationArchived: vi.fn(),
  listConversations: vi.fn(),
  loadLastOpenConversation: vi.fn(),
  backupConversationStore: vi.fn(),
  restoreConversationStore: vi.fn(),
  restoreLatestAutomaticBackup: vi.fn(),
}));

vi.mock("$lib/storage", async (importOriginal) => ({
  ...(await importOriginal<typeof import("$lib/storage")>()),
  ...storage,
}));

/** Builds the smallest stored conversation the state applies. */
function conversation(id: string): StoredConversation {
  return { id, branches: [], currentBranchId: null, attachments: [], messages: [] } as unknown as StoredConversation;
}

describe("ConversationState.open", () => {
  it("keeps the newer selection when an older load resolves last", async () => {
    const pending = new Map<string, (value: StoredConversation) => void>();
    storage.loadConversation.mockImplementation(
      (id: string) => new Promise<StoredConversation>((resolve) => pending.set(id, resolve)),
    );
    const state = new ConversationState();

    const openA = state.open("a");
    const openB = state.open("b");
    pending.get("b")?.(conversation("b"));
    expect(await openB).toEqual([]);
    pending.get("a")?.(conversation("a"));

    expect(await openA).toBeNull();
    expect(state.activeConversationId).toBe("b");
  });

  it("keeps a new chat blank when an earlier load resolves after it", async () => {
    let resolveA: (value: StoredConversation) => void = () => {};
    storage.loadConversation.mockImplementation(
      () => new Promise<StoredConversation>((resolve) => (resolveA = resolve)),
    );
    storage.clearLastOpenConversation.mockResolvedValue(undefined);
    const state = new ConversationState();

    const openA = state.open("a");
    await state.startNew();
    resolveA(conversation("a"));

    expect(await openA).toBeNull();
    expect(state.activeConversationId).toBeNull();
  });
});

describe("ConversationState.openSearchResult", () => {
  it("does not report a superseded load's failure over a newer selection", async () => {
    let rejectA: (error: unknown) => void = () => {};
    storage.loadConversation
      .mockImplementationOnce(() => new Promise<StoredConversation>((_, reject) => (rejectA = reject)))
      .mockResolvedValueOnce(conversation("b"));
    const state = new ConversationState();

    const openA = state.openSearchResult({ conversationId: "a", branchId: null } as never);
    expect(await state.open("b")).toEqual([]);
    rejectA(new Error("load failed"));

    expect(await openA).toBeNull();
    expect(state.storageError).toBeNull();
    expect(state.activeConversationId).toBe("b");
  });
});

describe("closing the active conversation", () => {
  it.each([
    ["delete", (state: ConversationState) => state.delete("a")],
    ["archive", (state: ConversationState) => state.setArchived("a", true)],
  ])("%s discards a reload of it that resolves afterwards", async (_name, close) => {
    storage.loadConversation.mockResolvedValueOnce(conversation("a"));
    storage.deleteConversation.mockResolvedValue(undefined);
    storage.setConversationArchived.mockResolvedValue(undefined);
    storage.listConversations.mockResolvedValue([]);
    const state = new ConversationState();
    await state.open("a");

    let resolveReload: (value: StoredConversation) => void = () => {};
    storage.loadConversation.mockImplementationOnce(
      () => new Promise<StoredConversation>((resolve) => (resolveReload = resolve)),
    );
    const reload = state.open("a");
    expect(await close(state)).toBe(true);
    resolveReload(conversation("a"));

    expect(await reload).toBeNull();
    expect(state.activeConversationId).toBeNull();
  });

  it.each([
    ["delete", (state: ConversationState) => state.delete("a")],
    ["archive", (state: ConversationState) => state.setArchived("a", true)],
  ])("%s keeps a load of another conversation that resolves afterwards", async (_name, close) => {
    storage.loadConversation.mockResolvedValueOnce(conversation("a"));
    storage.deleteConversation.mockResolvedValue(undefined);
    storage.setConversationArchived.mockResolvedValue(undefined);
    storage.listConversations.mockResolvedValue([]);
    const state = new ConversationState();
    await state.open("a");

    let resolveB: (value: StoredConversation) => void = () => {};
    storage.loadConversation.mockImplementationOnce(
      () => new Promise<StoredConversation>((resolve) => (resolveB = resolve)),
    );
    const openB = state.open("b");
    expect(await close(state)).toBe(true);
    resolveB(conversation("b"));

    expect(await openB).toEqual([]);
    expect(state.activeConversationId).toBe("b");
  });
});

describe("ConversationState.restoreBackup", () => {
  it.each([
    ["manual", storage.restoreConversationStore],
    ["automatic", storage.restoreLatestAutomaticBackup],
  ] as const)("asks for a restart after a %s restore when background work stopped", async (source, restore) => {
    restore.mockResolvedValue({
      status: "restored",
      fileName: "backup.sqlite3",
      preservedCopyName: "preserved.sqlite3",
      missingFiles: 0,
      backgroundWorkStopped: true,
    });
    storage.listConversations.mockResolvedValue([]);
    storage.loadLastOpenConversation.mockResolvedValue(null);
    const state = new ConversationState();

    expect(await state.restoreBackup(source)).toEqual([]);

    expect(state.backupFailed).toBe(false);
    expect(state.backupWarning).toBe(true);
    expect(state.backupFeedback).toBe(
      "Restored backup.sqlite3 · preserved current data as preserved.sqlite3 · Restart Bottie to finish processing restored data",
    );
  });

  it("marks a clean restore as neither failed nor a warning", async () => {
    storage.restoreConversationStore.mockResolvedValue({
      status: "restored",
      fileName: "backup.sqlite3",
      preservedCopyName: "preserved.sqlite3",
      missingFiles: 0,
      backgroundWorkStopped: false,
    });
    storage.listConversations.mockResolvedValue([]);
    storage.loadLastOpenConversation.mockResolvedValue(null);
    const state = new ConversationState();

    await state.restoreBackup();

    expect(state.backupFailed).toBe(false);
    expect(state.backupWarning).toBe(false);
  });
});

describe("ConversationState.backup", () => {
  it.each([
    [2, true],
    [0, false],
  ])("marks a backup with %i missing file(s) as warning=%s, never failed", async (missingFiles, warning) => {
    storage.backupConversationStore.mockResolvedValue({ status: "saved", fileName: "backup.sqlite3", missingFiles });
    const state = new ConversationState();

    await state.backup();

    expect(state.backupFailed).toBe(false);
    expect(state.backupWarning).toBe(warning);
  });
});
