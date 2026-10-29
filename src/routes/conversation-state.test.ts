import { describe, expect, it, vi } from "vitest";

import type { StoredConversation } from "$lib/storage";

import { ConversationState } from "./conversation-state.svelte";

const storage = vi.hoisted(() => ({ loadConversation: vi.fn(), clearLastOpenConversation: vi.fn() }));

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
