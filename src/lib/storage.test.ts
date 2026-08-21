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
import {
  applyAttachmentProcessingUpdate,
  applyAttachmentProcessingUpdateToMessages,
  attachmentFailure,
  attachmentStatusLabel,
} from "./attachment";

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
      extraction: { state: "unsupported", format: null, characterCount: null, pageCount: null, errorCode: null },
      indexing: { state: "unsupported" },
      normalization: { state: "ready", format: "png", width: 2, height: 2, byteSize: 16, errorCode: null },
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
        previewUrl: null,
        extraction: { state: "unsupported", format: null, characterCount: null, pageCount: null, errorCode: null },
        indexing: { state: "unsupported" },
        normalization: { state: "ready", format: "png", width: 2, height: 2, byteSize: 16, errorCode: null },
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
        extraction: { state: "ready", format: "markdown", characterCount: 42, pageCount: null, errorCode: null },
        indexing: { state: "indexable" },
        normalization: {
          state: "unsupported",
          format: null,
          width: null,
          height: null,
          byteSize: null,
          errorCode: null,
        },
      }),
    ).toEqual({
      id: "attachment-1",
      name: "notes.md",
      size: "2 KB",
      kind: "file",
      mimeType: "text/plain",
      previewUrl: null,
      extraction: { state: "ready", format: "markdown", characterCount: 42, pageCount: null, errorCode: null },
      indexing: { state: "indexable" },
      normalization: { state: "unsupported", format: null, width: null, height: null, byteSize: null, errorCode: null },
    });
  });

  it("explains local-only consequences for extraction and normalization failures", () => {
    const document = storedAttachmentToPresentation({
      id: "document",
      displayName: "scanned.pdf",
      mimeType: "application/pdf",
      byteSize: 100,
      extraction: {
        state: "failed",
        format: "pdf",
        characterCount: null,
        pageCount: null,
        errorCode: "pdf_no_text",
      },
      indexing: { state: "blocked" },
      normalization: { state: "unsupported", format: null, width: null, height: null, byteSize: null, errorCode: null },
    });
    const image = {
      ...document,
      kind: "image" as const,
      mimeType: "image/png",
      normalization: {
        state: "failed" as const,
        format: null,
        width: null,
        height: null,
        byteSize: null,
        errorCode: "image_decode_failed",
      },
    };

    expect(attachmentFailure(document)).toEqual({
      title: "PDF has no extractable text",
      detail: "The original file is still stored locally, but its text is unavailable for later indexing.",
    });
    expect(attachmentFailure(image)).toEqual({
      title: "Image could not be decoded",
      detail: "The original file is still stored locally, but Bottie cannot preview or send this image.",
    });
  });

  it("applies a path-free background update only to the matching attachment", () => {
    const pending = storedAttachmentToPresentation({
      id: "attachment-1",
      displayName: "notes.txt",
      mimeType: "text/plain",
      byteSize: 12,
      extraction: { state: "pending", format: null, characterCount: null, pageCount: null, errorCode: null },
      indexing: { state: "waiting_for_extraction" },
      normalization: { state: "unsupported", format: null, width: null, height: null, byteSize: null, errorCode: null },
    });
    const untouched = { ...pending, id: "attachment-2", name: "other.txt" };
    const updated = {
      id: "attachment-1",
      displayName: "notes.txt",
      mimeType: "text/plain",
      byteSize: 12,
      extraction: {
        state: "ready" as const,
        format: "plain_text" as const,
        characterCount: 12,
        pageCount: null,
        errorCode: null,
      },
      indexing: { state: "indexable" as const },
      normalization: {
        state: "unsupported" as const,
        format: null,
        width: null,
        height: null,
        byteSize: null,
        errorCode: null,
      },
    };

    expect(applyAttachmentProcessingUpdate([pending, untouched], updated)).toEqual([
      storedAttachmentToPresentation(updated),
      untouched,
    ]);
    expect(applyAttachmentProcessingUpdate([untouched], updated)).toEqual([untouched]);
    expect(
      applyAttachmentProcessingUpdateToMessages(
        [
          { id: 1, role: "user", content: "Attached", attachments: [pending] },
          { id: 2, role: "assistant", content: "Unchanged" },
        ],
        updated,
      ),
    ).toEqual([
      { id: 1, role: "user", content: "Attached", attachments: [storedAttachmentToPresentation(updated)] },
      { id: 2, role: "assistant", content: "Unchanged" },
    ]);
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

  it("describes path-free image normalization without receiving derivative bytes", () => {
    expect(
      attachmentStatusLabel({
        state: "ready",
        format: "png",
        width: 1_200,
        height: 800,
        byteSize: 320_000,
        errorCode: null,
      }),
    ).toBe("PNG normalized locally · 1200 × 800");
    expect(
      attachmentStatusLabel({
        state: "failed",
        format: null,
        width: null,
        height: null,
        byteSize: null,
        errorCode: "image_dimension_limit_exceeded",
      }),
    ).toBe("Image dimensions exceed local limit");
    expect(
      attachmentStatusLabel(
        {
          state: "unsupported",
          format: null,
          width: null,
          height: null,
          byteSize: null,
          errorCode: null,
        },
        {
          state: "ready",
          format: "markdown",
          characterCount: 42,
          pageCount: null,
          errorCode: null,
        },
        { state: "indexable" },
      ),
    ).toBe("Markdown ready locally · Ready for indexing");
  });
});
