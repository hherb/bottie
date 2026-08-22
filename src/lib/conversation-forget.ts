/** Focused frontend boundary and copy for permanent conversation forgetting. */

import { invoke, isTauri } from "@tauri-apps/api/core";

import type { StorageError } from "./storage";

/** User-facing policy shown before the irreversible native action. */
export const FORGET_CONVERSATION_CONFIRMATION =
  "Permanently delete this conversation, its message memory, and its file links? " +
  "Unshared file bytes are cleaned after the 24-hour safety window. Existing exports and backups are unchanged.";

/** Permanently deletes one already-trashed conversation through the narrow native boundary. */
export async function forgetConversation(conversationId: string): Promise<void> {
  if (!isTauri()) {
    throw {
      code: "internal",
      message: "Conversation storage is available only in the native Bottie app.",
    } satisfies StorageError;
  }
  return invoke<void>("forget_conversation", { conversationId });
}

/** Returns the explicit irreversible action label used only in Trash. */
export function forgetConversationActionLabel(): string {
  return "Forget permanently";
}
