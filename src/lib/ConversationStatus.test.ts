import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import ConversationStatus from "./ConversationStatus.svelte";

describe("ConversationStatus", () => {
  it("invites an available native model to start a private conversation", () => {
    const html = render(ConversationStatus, {
      props: {
        empty: true,
        providerStatus: "available",
        providerError: null,
        providerName: "Ollama",
        onretry: vi.fn(),
      },
    }).body;

    expect(html).toContain('class="conversation-empty-state"');
    expect(html).toContain("Ready when you are");
    expect(html).toContain("Ollama is connected");
    expect(html).not.toContain("Retry connection");
  });

  it("keeps checking, browser-preview, and offline guidance distinct", () => {
    const checking = render(ConversationStatus, {
      props: {
        empty: true,
        providerStatus: "checking",
        providerError: null,
        providerName: null,
        onretry: vi.fn(),
      },
    }).body;
    const browser = render(ConversationStatus, {
      props: {
        empty: true,
        providerStatus: "browser",
        providerError: {
          code: "unavailable",
          message: "Native inference is unavailable in the browser preview.",
          retryable: false,
        },
        providerName: null,
        onretry: vi.fn(),
      },
    }).body;
    const offline = render(ConversationStatus, {
      props: {
        empty: true,
        providerStatus: "offline",
        providerError: {
          code: "unavailable",
          message: "Ollama is not reachable.",
          diagnostic: "Check that Ollama is running, then retry.",
          retryable: true,
        },
        providerName: "Ollama",
        onretry: vi.fn(),
      },
    }).body;

    expect(checking).toContain("Connecting to your provider");
    expect(checking).toContain('aria-live="polite"');
    expect(browser).toContain("Open the Bottie desktop app to begin");
    expect(browser).not.toContain("Retry connection");
    expect(offline).toContain("Ollama is not reachable.");
    expect(offline).toContain("Check that Ollama is running, then retry.");
    expect(offline).toContain("Retry connection");
  });

  it("shows only the compact connection banner above an existing conversation", () => {
    const html = render(ConversationStatus, {
      props: {
        empty: false,
        providerStatus: "offline",
        providerError: {
          code: "timeout",
          message: "The provider timed out.",
          retryable: true,
        },
        providerName: "Ollama",
        onretry: vi.fn(),
      },
    }).body;

    expect(html).toContain('class="provider-banner offline"');
    expect(html).toContain("The provider timed out.");
    expect(html).not.toContain("conversation-empty-state");
  });
});
