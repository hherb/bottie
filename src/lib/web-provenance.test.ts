import { describe, expect, it } from "vitest";

import type { Message } from "./presentation";
import { webSourcesForMessages } from "./web-provenance";

type ToolFixture = Omit<NonNullable<Message["toolInvocations"]>[number], "audit">;

/** Builds one assistant message with durable safe native tool activity. */
function assistantWithTools(tools: ToolFixture[]): Message {
  return {
    id: 8,
    storageId: "assistant-8",
    role: "assistant",
    content: "A response grounded in web research.",
    toolInvocations: tools.map((tool) => ({
      ...tool,
      audit: {
        policy: "safe",
        outcome: tool.result?.isError ? "unavailable" : tool.result ? "success" : null,
        durationMs: tool.result ? Math.max(0, tool.result.createdAtMs - tool.createdAtMs) : null,
      },
    })),
  };
}

describe("web provenance", () => {
  it("derives path-free search and fetch sources from exact successful native envelopes", () => {
    const messages = [
      assistantWithTools([
        {
          ordinal: 0,
          toolName: "web_search",
          arguments: { query: "private query is not presentation" },
          result: {
            output: {
              ok: true,
              result: {
                providerId: "brave",
                results: [
                  {
                    title: "Rust release notes",
                    url: "https://blog.rust-lang.org/releases/1.90/#details",
                    snippet: "The current stable Rust release.",
                    publishedAt: "2026-08-20",
                  },
                  {
                    title: "Cargo guide",
                    url: "https://doc.rust-lang.org/cargo/guide/",
                    snippet: "Cargo package guidance.",
                    publishedAt: null,
                  },
                ],
              },
            },
            isError: false,
            createdAtMs: 2,
          },
          createdAtMs: 1,
        },
      ]),
    ];

    expect(webSourcesForMessages(messages)).toEqual([
      {
        id: "web:https://blog.rust-lang.org/releases/1.90/",
        kind: "search",
        label: "Search result",
        untrusted: false,
        title: "Rust release notes",
        url: "https://blog.rust-lang.org/releases/1.90/",
        host: "blog.rust-lang.org",
        excerpt: "The current stable Rust release.",
        publishedAt: "2026-08-20",
      },
      {
        id: "web:https://doc.rust-lang.org/cargo/guide/",
        kind: "search",
        label: "Search result",
        untrusted: false,
        title: "Cargo guide",
        url: "https://doc.rust-lang.org/cargo/guide/",
        host: "doc.rust-lang.org",
        excerpt: "Cargo package guidance.",
        publishedAt: null,
      },
    ]);
  });

  it("prefers the later fetched page for one URL and applies session-local dismissals", () => {
    const messages = [
      assistantWithTools([
        {
          ordinal: 0,
          toolName: "web_search",
          arguments: {},
          result: {
            output: {
              ok: true,
              result: {
                providerId: "exa",
                results: [
                  {
                    title: "Search title",
                    url: "https://example.com/article",
                    snippet: "Short search excerpt.",
                    publishedAt: null,
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
          toolName: "web_fetch",
          arguments: {},
          result: {
            output: {
              ok: true,
              result: {
                sourceUrl: "https://example.com/article",
                title: "Fetched article",
                publishedAt: "2026-08-21",
                content: "The complete bounded page excerpt.",
                untrusted: true,
              },
            },
            isError: false,
            createdAtMs: 4,
          },
          createdAtMs: 3,
        },
      ]),
    ];

    expect(webSourcesForMessages(messages)).toEqual([
      {
        id: "web:https://example.com/article",
        kind: "fetch",
        label: "Fetched page",
        untrusted: true,
        title: "Fetched article",
        url: "https://example.com/article",
        host: "example.com",
        excerpt: "The complete bounded page excerpt.",
        publishedAt: "2026-08-21",
      },
    ]);
    expect(webSourcesForMessages(messages, new Set(["web:https://example.com/article"]))).toEqual([]);
  });

  it("ignores failures, malformed results, unsafe URLs, and fetches without the untrusted marker", () => {
    const messages = [
      assistantWithTools([
        {
          ordinal: 0,
          toolName: "web_search",
          arguments: { query: "/secret/path" },
          result: { output: { ok: false, error: { code: "unavailable" } }, isError: true, createdAtMs: 2 },
          createdAtMs: 1,
        },
        {
          ordinal: 1,
          toolName: "web_search",
          arguments: {},
          result: {
            output: {
              ok: true,
              result: {
                providerId: "fixture",
                results: [
                  { title: "Local", url: "http://127.0.0.1/private", snippet: "/secret/path" },
                  { title: "Router", url: "http://router.local/private", snippet: "/secret/path" },
                  { title: "Onion", url: "https://private.onion/path", snippet: "/secret/path" },
                  { title: "Credential", url: "https://user:pass@example.com/path", snippet: "/secret/path" },
                  { title: "Port", url: "https://example.com:8443/path", snippet: "/secret/path" },
                  { title: "File", url: "file:///secret/path", snippet: "/secret/path" },
                  { title: "Missing URL", snippet: "/secret/path" },
                ],
              },
            },
            isError: false,
            createdAtMs: 4,
          },
          createdAtMs: 3,
        },
        {
          ordinal: 2,
          toolName: "web_fetch",
          arguments: {},
          result: {
            output: {
              ok: true,
              result: {
                sourceUrl: "https://example.com/private",
                title: "Missing trust marker",
                content: "/secret/path",
              },
            },
            isError: false,
            createdAtMs: 6,
          },
          createdAtMs: 5,
        },
      ]),
    ];

    expect(webSourcesForMessages(messages)).toEqual([]);
  });
});
