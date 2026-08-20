import { describe, expect, it } from "vitest";

import {
  activeConversationDateGroups,
  canBatchExportConversations,
  conversationExportFeedback,
  conversationsForLifecycle,
  formatToolPayload,
  type ConversationSummary,
} from "./storage";

const conversations: ConversationSummary[] = [
  { id: "active", title: "Active", updatedAtMs: 3, lifecycle: "active" },
  { id: "archived", title: "Archived", updatedAtMs: 2, lifecycle: "archived" },
  { id: "deleted", title: "Deleted", updatedAtMs: 1, lifecycle: "deleted" },
];

describe("conversation storage presentation helpers", () => {
  it("selects one lifecycle group without changing native ordering", () => {
    expect(conversationsForLifecycle(conversations, "archived")).toEqual([conversations[1]]);
    expect(conversationsForLifecycle(conversations, "active")).toEqual([conversations[0]]);
  });

  it("enables batch export only when active or archived conversations exist", () => {
    expect(canBatchExportConversations(conversations)).toBe(true);
    expect(canBatchExportConversations([conversations[2]])).toBe(false);
    expect(canBatchExportConversations([])).toBe(false);
  });

  it("groups active conversations by local calendar recency", () => {
    const now = new Date(2026, 7, 19, 12).getTime();
    const atLocalNoon = (daysAgo: number) => new Date(2026, 7, 19 - daysAgo, 12).getTime();
    const active = [
      { id: "today", title: "Today", updatedAtMs: atLocalNoon(0), lifecycle: "active" as const },
      { id: "yesterday", title: "Yesterday", updatedAtMs: atLocalNoon(1), lifecycle: "active" as const },
      { id: "week", title: "This week", updatedAtMs: atLocalNoon(6), lifecycle: "active" as const },
      { id: "older", title: "Older", updatedAtMs: atLocalNoon(8), lifecycle: "active" as const },
    ];

    expect(activeConversationDateGroups([...active, conversations[1]], now)).toEqual([
      { label: "Today", conversations: [active[0]] },
      { label: "Yesterday", conversations: [active[1]] },
      { label: "Previous 7 days", conversations: [active[2]] },
      { label: "Older", conversations: [active[3]] },
    ]);
  });

  it("labels successful selected and batch exports without needing a native path", () => {
    expect(conversationExportFeedback("markdown", "bottie-notes.md")).toBe("Saved bottie-notes.md");
    expect(conversationExportFeedback("json", null)).toBe("Saved JSON export");
    expect(conversationExportFeedback("batch-json", null)).toBe("Saved all conversations");
  });

  it("formats retained tool payloads as readable inert JSON", () => {
    expect(formatToolPayload({ query: "release", count: 2 })).toBe('{\n  "query": "release",\n  "count": 2\n}');
    expect(formatToolPayload("plain result")).toBe('"plain result"');
  });
});
