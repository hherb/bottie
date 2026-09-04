import { describe, expect, it } from "vitest";

import type { Message } from "./presentation";
import { memoryCitationsForMessages } from "./memory-provenance";

type ToolFixture = Omit<NonNullable<Message["toolInvocations"]>[number], "audit">;

/** Builds one assistant message with durable native tool activity. */
function assistantWithTools(tools: ToolFixture[]): Message {
  return {
    id: 7,
    storageId: "assistant-7",
    role: "assistant",
    content: "A response grounded in retained context.",
    toolInvocations: tools.map((tool) => ({
      ...tool,
      audit: {
        policy: tool.toolName === "web_search" ? "unregistered" : "safe",
        approval: null,
        outcome: tool.result?.isError ? "unavailable" : tool.result ? "success" : null,
        durationMs: tool.result ? Math.max(0, tool.result.createdAtMs - tool.createdAtMs) : null,
      },
    })),
  };
}

describe("memory provenance", () => {
  it("derives path-free conversation and attachment citations from successful native results", () => {
    const messages = [
      assistantWithTools([
        {
          ordinal: 0,
          toolName: "search_memory",
          arguments: { query: "private query is not presentation" },
          result: {
            output: {
              ok: true,
              result: {
                matches: [
                  {
                    rank: 1,
                    excerpt: "Keep provider traffic inside the Rust core.",
                    provenance: {
                      sourceKind: "message",
                      conversationId: "conversation-source",
                      conversationTitle: "Architecture notes",
                      messageId: "message-source",
                      role: "assistant",
                      createdAtMs: 1_776_000_000_000,
                      chunk: { ordinal: 0, startCharacter: 4, endCharacter: 52 },
                    },
                  },
                ],
              },
            },
            isError: false,
            createdAtMs: 2,
          },
          createdAtMs: 1,
        },
        {
          ordinal: 1,
          toolName: "search_attached_files",
          arguments: { query: "private file query" },
          result: {
            output: {
              ok: true,
              result: {
                matches: [
                  {
                    rank: 1,
                    excerpt: "Use bounded native parsing for retained documents.",
                    provenance: {
                      sourceKind: "attachment",
                      attachmentId: "attachment-source",
                      displayName: "design.md",
                      mimeType: "text/markdown",
                      byteSize: 2_048,
                      extractionFormat: "markdown",
                      characterCount: 48,
                      createdAtMs: 1_776_000_100_000,
                    },
                  },
                ],
              },
            },
            isError: false,
            createdAtMs: 4,
          },
          createdAtMs: 3,
        },
      ]),
    ];

    expect(memoryCitationsForMessages(messages)).toEqual([
      {
        id: "message:conversation-source:message-source",
        kind: "conversation",
        label: "Conversation memory",
        title: "Architecture notes",
        excerpt: "Keep provider traffic inside the Rust core.",
        createdAtMs: 1_776_000_000_000,
      },
      {
        id: "attachment:attachment-source",
        kind: "attachment",
        label: "Attached file",
        title: "design.md",
        excerpt: "Use bounded native parsing for retained documents.",
        createdAtMs: 1_776_000_100_000,
      },
    ]);
  });

  it("uses exact open-memory turns, deduplicates sources, and applies dismissals", () => {
    const messages = [
      assistantWithTools([
        {
          ordinal: 0,
          toolName: "search_memory",
          arguments: {},
          result: {
            output: {
              ok: true,
              result: {
                matches: [
                  {
                    excerpt: "Short ranked excerpt.",
                    provenance: {
                      sourceKind: "message",
                      conversationId: "conversation-source",
                      conversationTitle: "Architecture notes",
                      messageId: "message-source",
                      role: "assistant",
                      createdAtMs: 10,
                    },
                  },
                ],
              },
            },
            isError: false,
            createdAtMs: 2,
          },
          createdAtMs: 1,
        },
        {
          ordinal: 1,
          toolName: "open_memory",
          arguments: {},
          result: {
            output: {
              ok: true,
              result: {
                provenance: {
                  sourceKind: "message",
                  conversationId: "conversation-source",
                  conversationTitle: "Architecture notes",
                  messageId: "message-source",
                },
                turns: [
                  { messageId: "other", role: "user", text: "Nearby turn", createdAtMs: 9, isMatch: false },
                  {
                    messageId: "message-source",
                    role: "assistant",
                    text: "Complete matched turn.",
                    createdAtMs: 10,
                    isMatch: true,
                  },
                ],
              },
            },
            isError: false,
            createdAtMs: 4,
          },
          createdAtMs: 3,
        },
      ]),
    ];

    expect(memoryCitationsForMessages(messages)).toHaveLength(1);
    expect(memoryCitationsForMessages(messages)[0]?.excerpt).toBe("Short ranked excerpt.");
    expect(memoryCitationsForMessages(messages, new Set(["message:conversation-source:message-source"]))).toEqual([]);
  });

  it("ignores failed, unsupported, and malformed tool activity without reflecting it", () => {
    const messages = [
      assistantWithTools([
        {
          ordinal: 0,
          toolName: "search_memory",
          arguments: { query: "/secret/path" },
          result: { output: { ok: false, error: { code: "unavailable" } }, isError: true, createdAtMs: 2 },
          createdAtMs: 1,
        },
        {
          ordinal: 1,
          toolName: "web_search",
          arguments: {},
          result: { output: { ok: true, result: { matches: [] } }, isError: false, createdAtMs: 4 },
          createdAtMs: 3,
        },
        {
          ordinal: 2,
          toolName: "search_memory",
          arguments: {},
          result: {
            output: {
              ok: true,
              result: {
                matches: [
                  {
                    excerpt: "Malformed provenance",
                    provenance: { sourceKind: "message", filePath: "/secret/path" },
                  },
                ],
              },
            },
            isError: false,
            createdAtMs: 6,
          },
          createdAtMs: 5,
        },
      ]),
    ];

    expect(memoryCitationsForMessages(messages)).toEqual([]);
  });
});
