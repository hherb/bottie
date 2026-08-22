import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import { conversationMemoryActionLabel } from "./conversation-memory";
import type { ConversationSummary } from "./storage";
import ConversationGroup from "./ConversationGroup.svelte";

/** Renders one conversation navigation group with inert management callbacks. */
function renderedGroup(conversation: ConversationSummary): string {
  return render(ConversationGroup, {
    props: {
      label: "Today",
      conversations: [conversation],
      activeConversationId: conversation.id,
      disabled: false,
      onselect: vi.fn(),
      onrename: vi.fn(),
      onarchive: vi.fn(),
      onmemoryexclusion: vi.fn(),
      ondelete: vi.fn(),
      onrestore: vi.fn(),
    },
  }).body;
}

describe("ConversationGroup", () => {
  it("shows durable memory exclusion and the exact reversible action", () => {
    const included = renderedGroup({
      id: "included",
      title: "Included",
      updatedAtMs: 2,
      lifecycle: "active",
      memoryExcluded: false,
    });
    const excluded = renderedGroup({
      id: "excluded",
      title: "Excluded",
      updatedAtMs: 1,
      lifecycle: "archived",
      memoryExcluded: true,
    });

    expect(
      conversationMemoryActionLabel({
        id: "included",
        title: "Included",
        updatedAtMs: 2,
        lifecycle: "active",
        memoryExcluded: false,
      }),
    ).toBe("Exclude from memory");
    expect(included).not.toContain("Memory off");
    expect(
      conversationMemoryActionLabel({
        id: "excluded",
        title: "Excluded",
        updatedAtMs: 1,
        lifecycle: "archived",
        memoryExcluded: true,
      }),
    ).toBe("Include in memory");
    expect(excluded).toContain("Memory off");
  });
});
