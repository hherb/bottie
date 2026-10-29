import { describe, expect, it } from "vitest";

import {
  isKastellanPrompt,
  isPendingKastellanResponse,
  kastellanResponsePresentation,
  kastellanWaitingMeta,
} from "./kastellan";
import type { StoredMessage } from "./storage";

function stored(overrides: Partial<StoredMessage>): StoredMessage {
  return {
    id: "m",
    role: "assistant",
    text: "",
    reasoning: null,
    state: "partial",
    providerId: "kastellan",
    modelId: "localhost",
    providerRun: null,
    rating: null,
    attachments: [],
    generatedAssets: [],
    createdAtMs: 0,
    ...overrides,
  } as StoredMessage;
}

describe("Kastellan prompts", () => {
  it("recognises only the exact prefix, after leading whitespace", () => {
    expect(isKastellanPrompt("@kastellan: do it")).toBe(true);
    expect(isKastellanPrompt("   @kastellan:do it")).toBe(true);
    expect(isKastellanPrompt("@Kastellan: do it")).toBe(false);
    expect(isKastellanPrompt("ask @kastellan: later")).toBe(false);
  });

  it("treats only an unanswered, uninterrupted Kastellan response as pending", () => {
    expect(isPendingKastellanResponse(stored({}))).toBe(true);
    expect(isPendingKastellanResponse(stored({ text: "done" }))).toBe(false);
    expect(isPendingKastellanResponse(stored({ state: "final" }))).toBe(false);
    expect(isPendingKastellanResponse(stored({ providerId: "ollama" }))).toBe(false);
    expect(
      isPendingKastellanResponse(stored({ providerRun: { errorCode: "interrupted" } as StoredMessage["providerRun"] })),
    ).toBe(false);
  });
});

describe("Kastellan response presentation", () => {
  it("names the host while waiting", () => {
    expect(kastellanWaitingMeta("dgx")).toBe("Sent to Kastellan on dgx · the reply will appear here");
    expect(kastellanResponsePresentation(stored({ modelId: "dgx" }))).toEqual({
      meta: "Sent to Kastellan on dgx · the reply will appear here",
      reasoning: undefined,
      model: "Kastellan · dgx",
    });
  });

  it("shows the saved provenance as the footer instead of as reasoning", () => {
    const answered = stored({
      state: "final",
      text: "pong",
      reasoning: "Kastellan task 204 on dgx · 34s",
      modelId: "dgx",
    });
    expect(kastellanResponsePresentation(answered)).toEqual({
      meta: "Kastellan task 204 on dgx · 34s",
      reasoning: undefined,
      model: "Kastellan · dgx",
    });
  });

  it("leaves an interrupted task to the shared interrupted presentation", () => {
    const interrupted = stored({ providerRun: { errorCode: "interrupted" } as StoredMessage["providerRun"] });
    expect(kastellanResponsePresentation(interrupted)?.meta).toBeUndefined();
  });

  it("ignores other providers", () => {
    expect(kastellanResponsePresentation(stored({ providerId: "ollama" }))).toBeNull();
  });
});
