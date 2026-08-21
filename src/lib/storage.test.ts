import { describe, expect, it } from "vitest";

import {
  activeConversationDateGroups,
  canBatchExportConversations,
  conversationExportFeedback,
  conversationsForLifecycle,
  attachmentExtractionLabel,
  formatToolPayload,
  mergeIngestedAttachments,
  storedAttachmentToPresentation,
  type IngestedAttachment,
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

  it("merges native attachment metadata without repeating the same content", () => {
    const first: IngestedAttachment = {
      id: "attachment-1",
      displayName: "diagram.png",
      mimeType: "image/png",
      byteSize: 16,
      sha256: "abc123",
      extraction: { state: "unsupported", format: null, characterCount: null, pageCount: null, errorCode: null },
      duplicate: false,
    };
    const duplicate = { ...first, duplicate: true };

    expect(mergeIngestedAttachments([], [first])).toEqual([
      {
        id: "attachment-1",
        name: "diagram.png",
        size: "16 B",
        kind: "image",
        mimeType: "image/png",
        sha256: "abc123",
        extraction: { state: "unsupported", format: null, characterCount: null, pageCount: null, errorCode: null },
      },
    ]);
    expect(mergeIngestedAttachments(mergeIngestedAttachments([], [first]), [duplicate])).toHaveLength(1);
  });

  it("maps reopened attachment metadata into path-free message presentation", () => {
    expect(
      storedAttachmentToPresentation({
        id: "attachment-1",
        displayName: "notes.md",
        mimeType: "text/plain",
        byteSize: 2_048,
        sha256: "abc123",
        extraction: { state: "ready", format: "markdown", characterCount: 42, pageCount: null, errorCode: null },
      }),
    ).toEqual({
      id: "attachment-1",
      name: "notes.md",
      size: "2 KB",
      kind: "file",
      mimeType: "text/plain",
      sha256: "abc123",
      extraction: { state: "ready", format: "markdown", characterCount: 42, pageCount: null, errorCode: null },
    });
  });

  it("describes native extraction states without receiving extracted content", () => {
    expect(
      attachmentExtractionLabel({
        state: "ready",
        format: "markdown",
        characterCount: 42,
        pageCount: null,
        errorCode: null,
      }),
    ).toBe("Markdown ready locally");
    expect(
      attachmentExtractionLabel({
        state: "ready",
        format: "plain_text",
        characterCount: 12,
        pageCount: null,
        errorCode: null,
      }),
    ).toBe("Text ready locally");
    expect(
      attachmentExtractionLabel({
        state: "unsupported",
        format: null,
        characterCount: null,
        pageCount: null,
        errorCode: null,
      }),
    ).toBe("No text extraction");
    expect(
      attachmentExtractionLabel({
        state: "ready",
        format: "pdf",
        characterCount: 42,
        pageCount: 2,
        errorCode: null,
      }),
    ).toBe("PDF text ready locally · 2 pages");
    expect(
      attachmentExtractionLabel({
        state: "ready",
        format: "docx",
        characterCount: 64,
        pageCount: null,
        errorCode: null,
      }),
    ).toBe("DOCX text ready locally");
    expect(
      attachmentExtractionLabel({
        state: "failed",
        format: null,
        characterCount: null,
        pageCount: null,
        errorCode: "content_too_large",
      }),
    ).toBe("Text too large to extract");
    expect(
      attachmentExtractionLabel({
        state: "failed",
        format: null,
        characterCount: null,
        pageCount: null,
        errorCode: "pdf_page_limit_exceeded",
      }),
    ).toBe("PDF has too many pages");
    expect(
      attachmentExtractionLabel({
        state: "failed",
        format: null,
        characterCount: null,
        pageCount: null,
        errorCode: "docx_archive_limit_exceeded",
      }),
    ).toBe("DOCX archive is too complex");
  });
});
