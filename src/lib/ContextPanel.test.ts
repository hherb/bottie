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
    sha256: id,
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
});
