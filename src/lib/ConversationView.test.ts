import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import ConversationView from "./ConversationView.svelte";

describe("ConversationView", () => {
  it("labels a durable failed response without replacing its stable content or retry action", () => {
    const html = render(ConversationView, {
      props: {
        messages: [
          {
            id: 1,
            storageId: "response-1",
            role: "assistant",
            content: "Generation failed before any response was saved.",
            meta: "Generation failed · saved partial response",
            error: true,
            retryable: true,
          },
        ],
        providerStatus: "available",
        providerError: null,
        selectedModel: undefined,
        activeStage: -1,
        inferenceStages: [],
        isGenerating: false,
        canGenerate: true,
        branches: [],
        currentBranchId: null,
        onretry: vi.fn(),
        onselectbranch: vi.fn(),
        oneditmessage: vi.fn(),
        onregenerate: vi.fn(),
        onretryresponse: vi.fn(),
        onrateresponse: vi.fn(),
        onremoveattachment: vi.fn(),
        onscrollready: vi.fn(),
      },
    }).body;

    expect(html).toContain('class="message-state error-state"');
    expect(html).toContain("Response needs attention");
    expect(html).toContain("Generation failed before any response was saved.");
    expect(html).toContain('aria-label="Retry response"');
  });
});
