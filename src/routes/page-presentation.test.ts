import { describe, expect, it } from "vitest";

import type { Attachment, Message } from "$lib/presentation";

import {
  inferenceStages,
  messageAttachmentAssociations,
  nextRequestAttachments,
  selectedProviderEndpoint,
} from "./page-presentation";

const attachment: Attachment = {
  id: "attachment-1",
  name: "context.png",
  size: "1 KB",
  kind: "image",
  mimeType: "image/png",
  previewUrl: null,
  extraction: { state: "unsupported", format: null, characterCount: null, pageCount: null, errorCode: null },
  indexing: { state: "unsupported" },
  normalization: { state: "ready", format: "png", width: 1, height: 1, byteSize: 68, errorCode: null },
};

describe("page presentation", () => {
  it("keeps draft, conversation, and message scope identities explicit", () => {
    const messages: Message[] = [
      { id: 1, storageId: "message-1", role: "user", content: "Use context", attachments: [attachment] },
      { id: 2, role: "assistant", content: "Done" },
    ];

    expect(messageAttachmentAssociations(messages)).toEqual([{ messageId: "message-1", attachment }]);
    expect(nextRequestAttachments([attachment], [{ ...attachment, id: "conversation-attachment" }])).toEqual([
      attachment,
      { ...attachment, id: "conversation-attachment" },
    ]);
  });

  it("builds provider route labels without exposing protocol noise", () => {
    const settings = {
      omlxBaseUrl: "http://127.0.0.1:8000/",
      ollamaBaseUrl: "http://127.0.0.1:11434/",
      openaiBaseUrl: "https://api.openai.com/v1/",
      anthropicBaseUrl: "https://api.anthropic.com/v1/",
      lastProviderId: null,
      lastModelId: null,
    } as const;

    expect(selectedProviderEndpoint("ollama", settings)).toBe("127.0.0.1:11434");
    expect(inferenceStages(false, "Cloud provider", "low")).toEqual([
      { icon: "shield", label: "Cloud route confirmed", detail: "Rust → Cloud provider" },
      { icon: "sparkles", label: "Streaming response", detail: "Low reasoning" },
    ]);
  });
});
