/** Deterministic large-history fixtures used only by Bottie's opt-in performance budgets. */

import type { Message } from "./presentation";
import type { ConversationSummary, StoredMessage } from "./storage";

/** Number of navigation rows in the large-history fixture. */
export const PERFORMANCE_CONVERSATION_COUNT = 2_000;

/** Number of turns in the selected long-lineage fixture. */
export const PERFORMANCE_MESSAGE_COUNT = 600;

const FIXTURE_NOW_MS = Date.UTC(2026, 7, 24, 12);
const ACTIVE_CONVERSATION_COUNT = 1_500;
const MESSAGE_PARAGRAPH =
  "This deterministic paragraph exercises the same Markdown and wrapping path as an ordinary retained answer.";

/** Builds a stable active/Archived navigation history in native recency order. */
export function performanceConversations(): ConversationSummary[] {
  return Array.from({ length: PERFORMANCE_CONVERSATION_COUNT }, (_, index) => ({
    id: `performance-conversation-${index}`,
    title: `Performance conversation ${index.toString().padStart(4, "0")}`,
    updatedAtMs: FIXTURE_NOW_MS - index * 60_000,
    lifecycle: index < ACTIVE_CONVERSATION_COUNT ? "active" : "archived",
    memoryExcluded: index % 29 === 0,
  }));
}

/** Builds path-free durable turns for measuring conversation switching reconstruction. */
export function performanceStoredMessages(): StoredMessage[] {
  return Array.from({ length: PERFORMANCE_MESSAGE_COUNT }, (_, index) => ({
    id: `performance-message-${index}`,
    role: index % 2 === 0 ? "user" : "assistant",
    text: index % 2 === 0 ? `Question ${index}\n\n${MESSAGE_PARAGRAPH}` : `## Answer ${index}\n\n${MESSAGE_PARAGRAPH}`,
    reasoning: null,
    state: "final",
    providerId: index % 2 === 0 ? null : "ollama",
    modelId: index % 2 === 0 ? null : "fixture-model",
    providerRun: null,
    rating: null,
    attachments: [],
    createdAtMs: FIXTURE_NOW_MS + index,
  }));
}

/** Builds the corresponding presentation turns for full long-lineage rendering. */
export function performanceMessages(): Message[] {
  return performanceStoredMessages().map((message, index) => ({
    id: index + 1,
    storageId: message.id,
    role: message.role,
    content: message.text,
    model: message.role === "assistant" ? "fixture-model · ollama" : undefined,
  }));
}
