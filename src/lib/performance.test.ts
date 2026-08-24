import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import { storedMessageToPresentation } from "../routes/conversation-presentation";
import ConversationView from "./ConversationView.svelte";
import Sidebar from "./Sidebar.svelte";
import {
  PERFORMANCE_CONVERSATION_COUNT,
  PERFORMANCE_MESSAGE_COUNT,
  performanceConversations,
  performanceMessages,
  performanceStoredMessages,
} from "./performance-fixtures";

const FRONTEND_SWITCH_BUDGET_MS = 50;
const FRONTEND_SIDEBAR_RENDER_BUDGET_MS = 1_500;
const FRONTEND_CONVERSATION_RENDER_BUDGET_MS = 2_000;
const MEASUREMENT_RUNS = 3;

/** Measures the fastest warmed run so scheduler noise does not make budgets flaky. */
function fastestDuration(operation: () => void): number {
  operation();
  let fastest = Number.POSITIVE_INFINITY;
  for (let run = 0; run < MEASUREMENT_RUNS; run += 1) {
    const startedAt = performance.now();
    operation();
    fastest = Math.min(fastest, performance.now() - startedAt);
  }
  return fastest;
}

/** Returns inert callbacks shared by the large navigation render. */
function sidebarCallbacks() {
  return {
    onclose: vi.fn(),
    onnewchat: vi.fn(),
    onselectconversation: vi.fn(),
    onsearch: vi.fn(),
    onselectsearchresult: vi.fn(),
    onrenameconversation: vi.fn(),
    onarchiveconversation: vi.fn(),
    onmemoryexclusion: vi.fn(),
    ondeleteconversation: vi.fn(),
    onrestoreconversation: vi.fn(),
    onforgetconversation: vi.fn(),
    onopensettings: vi.fn(),
  };
}

describe.runIf(import.meta.env.MODE === "performance")("frontend performance budgets", () => {
  it("reconstructs a long selected lineage within the switching budget", () => {
    const stored = performanceStoredMessages();
    let mapped = 0;
    const duration = fastestDuration(() => {
      mapped = stored.map(storedMessageToPresentation).length;
    });

    console.info(`performance budget: switch=${duration.toFixed(2)}ms`);
    expect(mapped).toBe(PERFORMANCE_MESSAGE_COUNT);
    expect(duration).toBeLessThan(FRONTEND_SWITCH_BUDGET_MS);
  });

  it("renders a large active and Archived history within the navigation budget", () => {
    const conversations = performanceConversations();
    let html = "";
    const duration = fastestDuration(() => {
      html = render(Sidebar, {
        props: {
          mobileOpen: false,
          runtimeVersion: "performance",
          conversations,
          activeConversationId: conversations[0].id,
          storageError: null,
          searchQuery: "",
          searchResults: [],
          isSearching: false,
          isGenerating: false,
          newChatShortcut: "⌘ N",
          searchShortcut: "⌘ ⇧ F",
          ...sidebarCallbacks(),
        },
      }).body;
    });

    console.info(`performance budget: sidebar-render=${duration.toFixed(2)}ms`);
    expect(html).toContain(
      `Performance conversation ${(PERFORMANCE_CONVERSATION_COUNT - 1).toString().padStart(4, "0")}`,
    );
    expect(duration).toBeLessThan(FRONTEND_SIDEBAR_RENDER_BUDGET_MS);
  });

  it("renders a long selected lineage within the conversation budget", () => {
    const messages = performanceMessages();
    let html = "";
    const duration = fastestDuration(() => {
      html = render(ConversationView, {
        props: {
          messages,
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
    });

    console.info(`performance budget: conversation-render=${duration.toFixed(2)}ms`);
    expect(html).toContain(`Answer ${PERFORMANCE_MESSAGE_COUNT - 1}`);
    expect(duration).toBeLessThan(FRONTEND_CONVERSATION_RENDER_BUDGET_MS);
  });
});
