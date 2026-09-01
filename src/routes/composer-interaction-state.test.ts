import { describe, expect, it, vi } from "vitest";

import { ComposerInteractionState } from "./composer-interaction-state";

describe("ComposerInteractionState", () => {
  it("focuses the updated transcript draft with the caret at its end", async () => {
    const state = new ComposerInteractionState();
    const composer = {
      focus: vi.fn(),
      scrollHeight: 74,
      setSelectionRange: vi.fn(),
      style: { height: "" },
      value: "Existing draft\n\nCopied transcript",
    } as unknown as HTMLTextAreaElement;
    state.setComposer(composer);

    await state.focusDraftAfterUpdate();

    expect(composer.style.height).toBe("74px");
    expect(composer.focus).toHaveBeenCalledOnce();
    expect(composer.setSelectionRange).toHaveBeenCalledWith(composer.value.length, composer.value.length);
  });
});
