/** Focused frontend boundary and copy for durable conversation memory exclusion. */

import { invoke, isTauri } from "@tauri-apps/api/core";

import type { ConversationSummary, StorageError } from "./storage";

/** Excludes or restores one active or archived conversation in native long-term memory. */
export async function setConversationMemoryExcluded(
  conversationId: string,
  excluded: boolean,
): Promise<ConversationSummary> {
  if (!isTauri()) {
    throw {
      code: "internal",
      message: "Conversation storage is available only in the native Bottie app.",
    } satisfies StorageError;
  }
  return invoke<ConversationSummary>("set_conversation_memory_excluded", { conversationId, excluded });
}

/** Labels the reversible native memory action for one conversation summary. */
export function conversationMemoryActionLabel(conversation: ConversationSummary): string {
  return conversation.memoryExcluded ? "Include in memory" : "Exclude from memory";
}
