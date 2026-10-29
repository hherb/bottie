import { describe, expect, it, vi } from "vitest";

import { PageState } from "./page-state.svelte";

const native = vi.hoisted(() => ({
  startKastellanTask: vi.fn(),
  startChat: vi.fn(),
}));

vi.mock("$lib/kastellan", async (importOriginal) => ({
  ...(await importOriginal<typeof import("$lib/kastellan")>()),
  startKastellanTask: native.startKastellanTask,
}));

vi.mock("$lib/inference", async (importOriginal) => ({
  ...(await importOriginal<typeof import("$lib/inference")>()),
  startChat: native.startChat,
}));

const CONTEXT = { conversationId: "conversation", requestMessageId: "request" };

/** A page with no usable model, which a Kastellan request must not need. */
function offlinePage(prompt = ""): PageState {
  native.startKastellanTask.mockReset().mockResolvedValue({ runId: "run", host: "dgx" });
  native.startChat.mockReset();
  const state = new PageState();
  state.messages = [];
  state.prompt = prompt;
  state.attachment.items = [];
  state.providerStatus = "offline";
  vi.spyOn(state.history, "persistUserMessage").mockResolvedValue(CONTEXT);
  return state;
}

describe("PageState Kastellan routing", () => {
  it("dispatches a prefixed prompt to Kastellan without a model and without holding the composer", async () => {
    const state = offlinePage("@kastellan: summarise my day");

    await state.sendMessage();

    expect(native.startKastellanTask).toHaveBeenCalledWith(CONTEXT, "@kastellan: summarise my day");
    expect(native.startChat).not.toHaveBeenCalled();
    expect(state.isGenerating).toBe(false);
    expect(state.prompt).toBe("");
    const [request, reply] = state.messages;
    expect(request).toMatchObject({ role: "user", storageId: "request" });
    expect(reply).toMatchObject({ role: "assistant", content: "", model: "Kastellan · dgx" });
  });

  it("still requires a usable model for ordinary prompts", async () => {
    const state = offlinePage("hello");

    await state.sendMessage();

    expect(state.history.persistUserMessage).not.toHaveBeenCalled();
    expect(native.startKastellanTask).not.toHaveBeenCalled();
  });

  it("sends a regenerated Kastellan request back to Kastellan, not to the selected model", async () => {
    const state = offlinePage();
    state.messages = [
      { id: 1, storageId: "request", role: "user", content: "@kastellan: do it" },
      { id: 2, storageId: "reply", role: "assistant", content: "done" },
    ];
    vi.spyOn(state.history, "branchFromUserMessage").mockResolvedValue({
      messages: [{ id: 3, storageId: "request-2", role: "user", content: "@kastellan: do it" }],
      context: { conversationId: "conversation", requestMessageId: "request-2" },
    });

    await state.regenerateResponse(2);

    expect(native.startKastellanTask).toHaveBeenCalledWith(
      { conversationId: "conversation", requestMessageId: "request-2" },
      "@kastellan: do it",
    );
    expect(native.startChat).not.toHaveBeenCalled();
  });

  it("reloads the open conversation through history once the page is idle", async () => {
    const state = offlinePage();
    state.history.activeConversationId = "conversation";
    const reloaded = [{ id: 9, role: "assistant" as const, content: "late reply" }];
    const refresh = vi.spyOn(state.history, "refreshAfterGeneration").mockResolvedValue(reloaded);

    state.isGenerating = true;
    await state.kastellan.applyCompletion({ conversationId: "conversation", runId: "run" });
    expect(refresh).not.toHaveBeenCalled();

    state.isGenerating = false;
    await state.kastellan.applyCompletion({ conversationId: "conversation", runId: "run" });
    expect(state.messages).toEqual(reloaded);
  });
});
