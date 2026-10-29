import { describe, expect, it, vi } from "vitest";

import { KASTELLAN_SENDING_META } from "$lib/kastellan";
import type { Message } from "$lib/presentation";

import { KastellanState, type KastellanPage } from "./kastellan-state.svelte";

const native = vi.hoisted(() => ({ startKastellanTask: vi.fn() }));

vi.mock("$lib/kastellan", async (importOriginal) => ({
  ...(await importOriginal<typeof import("$lib/kastellan")>()),
  startKastellanTask: native.startKastellanTask,
}));

const CONTEXT = { conversationId: "open", requestMessageId: "request" };

/** A page double whose timeline is a plain array. */
function page(overrides: Partial<KastellanPage> = {}): KastellanPage & { timeline: Message[] } {
  const timeline: Message[] = [];
  return {
    timeline,
    messages: () => timeline,
    replaceMessages: (messages) => timeline.splice(0, timeline.length, ...messages),
    activeConversationId: () => "open",
    isBusy: () => false,
    reload: vi.fn().mockResolvedValue(null),
    scrollToBottom: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

describe("KastellanState", () => {
  it("shows a waiting response that names the host native code chose", async () => {
    let accept = (_: { runId: string; host: string }) => {};
    native.startKastellanTask.mockReset().mockReturnValue(new Promise((resolve) => (accept = resolve)));
    const target = page();
    const dispatched = new KastellanState(target).dispatch(CONTEXT, "@kastellan: do it");
    await vi.waitFor(() => expect(native.startKastellanTask).toHaveBeenCalledWith(CONTEXT, "@kastellan: do it"));
    expect(target.timeline[0]).toMatchObject({ role: "assistant", content: "", meta: KASTELLAN_SENDING_META });

    accept({ runId: "run", host: "dgx" });
    await dispatched;
    expect(target.timeline[0]).toMatchObject({
      model: "Kastellan · dgx",
      meta: "Sent to Kastellan on dgx · the reply will appear here",
    });
  });

  it("turns a refused dispatch into an error response", async () => {
    native.startKastellanTask.mockReset().mockRejectedValue({
      code: "invalid_request",
      message: "The Kastellan host must be a host name, address, or ssh alias without spaces.",
      retryable: false,
    });
    const target = page();
    await new KastellanState(target).dispatch(CONTEXT, "@kastellan: do it");
    expect(target.timeline[0]).toMatchObject({
      content: "The Kastellan host must be a host name, address, or ssh alias without spaces.",
      error: true,
      meta: undefined,
    });
  });

  it("reloads only the open, idle conversation when a reply is saved", async () => {
    const reloaded: Message[] = [{ id: 1, role: "assistant", content: "late reply" }];
    let busy = false;
    const target = page({ isBusy: () => busy, reload: vi.fn().mockResolvedValue(reloaded) });
    const state = new KastellanState(target);

    await state.applyCompletion({ conversationId: "elsewhere", runId: "run" });
    busy = true;
    await state.applyCompletion({ conversationId: "open", runId: "run" });
    expect(target.reload).not.toHaveBeenCalled();

    busy = false;
    await state.applyCompletion({ conversationId: "open", runId: "run" });
    expect(target.reload).toHaveBeenCalledOnce();
    expect(target.timeline).toEqual(reloaded);
  });

  it("discards a reload that finishes after the user moved on", async () => {
    let active = "open";
    let finishReload = (_: Message[]) => {};
    const reload = vi.fn(() => new Promise<Message[]>((resolve) => (finishReload = resolve)));
    const target = page({ activeConversationId: () => active, reload });
    target.timeline.push({ id: 7, role: "user", content: "a message in the other conversation" });

    const applied = new KastellanState(target).applyCompletion({ conversationId: "open", runId: "run" });
    active = "other";
    finishReload([{ id: 1, role: "assistant", content: "stale" }]);
    await applied;

    expect(target.timeline).toEqual([{ id: 7, role: "user", content: "a message in the other conversation" }]);
  });
});
