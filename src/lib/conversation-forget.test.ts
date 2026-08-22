import { describe, expect, it } from "vitest";

import { FORGET_CONVERSATION_CONFIRMATION, forgetConversationActionLabel } from "./conversation-forget";

describe("conversation forget policy", () => {
  it("uses explicit irreversible copy and names retained external copies", () => {
    expect(forgetConversationActionLabel()).toBe("Forget permanently");
    expect(FORGET_CONVERSATION_CONFIRMATION).toContain("24-hour safety window");
    expect(FORGET_CONVERSATION_CONFIRMATION).toContain("Existing exports and backups are unchanged");
  });
});
