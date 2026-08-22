import { describe, expect, it } from "vitest";

import {
  CONVERSATION_RETENTION_OPTIONS,
  conversationRetentionDisclosure,
  type ConversationRetentionPeriod,
} from "./conversation-retention";

describe("conversation retention presentation", () => {
  it("offers only bounded native retention periods with manual retention first", () => {
    expect(CONVERSATION_RETENTION_OPTIONS.map((option) => option.value)).toEqual([
      "forever",
      "thirty_days",
      "ninety_days",
      "one_year",
    ] satisfies ConversationRetentionPeriod[]);
    expect(CONVERSATION_RETENTION_OPTIONS[0]?.label).toBe("Keep until I forget manually");
  });

  it("makes startup deletion and external-copy limits explicit", () => {
    const disclosure = conversationRetentionDisclosure("thirty_days");

    expect(disclosure).toContain("next healthy app launch");
    expect(disclosure).toContain("Trash for 30 days");
    expect(disclosure).toContain("exports and backups are unchanged");
    expect(disclosure).toContain("model cache is retained");
    expect(disclosure).not.toMatch(/\/(Users|home|var|tmp)\//);
  });
});
