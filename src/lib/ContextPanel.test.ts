import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import type { Attachment } from "./presentation";
import ContextPanel from "./ContextPanel.svelte";

/** Builds one path-free document fixture for scope-label rendering. */
function attachment(id: string, name: string): Attachment {
  return {
    id,
    name,
    size: "1 KB",
    kind: "file",
    mimeType: "text/plain",
    previewUrl: null,
    extraction: { state: "ready", format: "plain_text", characterCount: 12, pageCount: null, errorCode: null },
    indexing: { state: "indexable" },
    normalization: { state: "unsupported", format: null, width: null, height: null, byteSize: null, errorCode: null },
  };
}

describe("ContextPanel", () => {
  it("distinguishes next-message, conversation, and message attachment ownership", () => {
    const draft = attachment("draft", "draft.txt");
    const conversation = attachment("conversation", "conversation.txt");
    const message = attachment("message", "message.txt");
    const html = render(ContextPanel, {
      props: {
        open: true,
        attachments: [draft],
        conversationAttachments: [conversation],
        messageAttachments: [{ messageId: "message-1", attachment: message }],
        canKeepInConversation: true,
        selectedModel: undefined,
        selectedProviderEndpoint: "127.0.0.1:11434",
        providerStatus: "available",
        isLocalRoute: true,
        isAddingAttachments: false,
        attachmentFeedback: null,
        attachmentFailed: false,
        attachmentActionsDisabled: false,
        onclose: vi.fn(),
        onadd: vi.fn(),
        onremove: vi.fn(),
        onkeep: vi.fn(),
        onremoveconversation: vi.fn(),
        onremovemessage: vi.fn(),
      },
    }).body;

    expect(html).toContain("Next message · 1 KB");
    expect(html).toContain("Conversation · 1 KB");
    expect(html).toContain("Message · 1 KB");
    expect(html).toContain('aria-label="Keep draft.txt in conversation"');
    expect(html).toContain('aria-label="Remove conversation.txt from conversation"');
    expect(html).toContain('aria-label="Remove message.txt from message"');
  });

  it("shows ready image thumbnails and explicit extraction failures", () => {
    const image = {
      ...attachment("image", "diagram.png"),
      kind: "image" as const,
      mimeType: "image/png",
      previewUrl: "bottie-attachment://localhost/image",
      extraction: {
        state: "unsupported" as const,
        format: null,
        characterCount: null,
        pageCount: null,
        errorCode: null,
      },
      indexing: { state: "unsupported" as const },
      normalization: {
        state: "ready" as const,
        format: "png" as const,
        width: 320,
        height: 180,
        byteSize: 4_096,
        errorCode: null,
      },
    };
    const failed = {
      ...attachment("failed", "scan.pdf"),
      mimeType: "application/pdf",
      extraction: {
        state: "failed" as const,
        format: "pdf" as const,
        characterCount: null,
        pageCount: null,
        errorCode: "pdf_no_text",
      },
      indexing: { state: "blocked" as const },
    };
    const html = render(ContextPanel, {
      props: {
        open: true,
        attachments: [image, failed],
        conversationAttachments: [],
        messageAttachments: [],
        canKeepInConversation: true,
        selectedModel: undefined,
        selectedProviderEndpoint: "127.0.0.1:11434",
        providerStatus: "available",
        isLocalRoute: true,
        isAddingAttachments: false,
        attachmentFeedback: null,
        attachmentFailed: false,
        attachmentActionsDisabled: false,
        onclose: vi.fn(),
        onadd: vi.fn(),
        onremove: vi.fn(),
        onkeep: vi.fn(),
        onremoveconversation: vi.fn(),
        onremovemessage: vi.fn(),
      },
    }).body;

    expect(html).toContain('src="bottie-attachment://localhost/image"');
    expect(html).toContain("PDF has no extractable text");
    expect(html).toContain("its text is unavailable for later indexing");
    expect(html).toContain('class="attachment-row failed"');
  });
});
