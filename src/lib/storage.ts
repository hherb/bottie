/** Typed frontend boundary for Rust-owned durable conversation storage. */

import { invoke, isTauri } from "@tauri-apps/api/core";

import type { Usage } from "./inference";

/** Durable message states supported by the initial SQLite schema. */
export type StoredMessageState = "partial" | "final" | "cancelled" | "failed";

/** Durable identity linking a native generation to its persisted user request. */
export type ProviderRunContext = {
  conversationId: string;
  requestMessageId: string;
};

/** Persisted native generation provenance linked to one assistant response. */
export type StoredProviderRun = {
  id: string;
  state: "running" | "completed" | "cancelled" | "failed";
  reasoningEffort: "off" | "low";
  startedAtMs: number;
  completedAtMs: number | null;
  errorCode: string | null;
  usage: Usage | null;
};

/** Stable storage failure returned by native commands. */
export type StorageError = {
  code: "invalid_request" | "not_found" | "internal";
  message: string;
};

/** One conversation row used by navigation. */
export type ConversationSummary = {
  id: string;
  title: string;
  updatedAtMs: number;
  lifecycle: ConversationLifecycle;
};

/** Recoverable lifecycle state used by conversation navigation. */
export type ConversationLifecycle = "active" | "archived" | "deleted";

/** One non-empty date bucket used by active conversation navigation. */
export type ConversationDateGroup = {
  label: "Today" | "Yesterday" | "Previous 7 days" | "Older";
  conversations: ConversationSummary[];
};

const MILLISECONDS_PER_DAY = 86_400_000;
const PREVIOUS_DAYS_LIMIT = 7;

/** One persisted text/reasoning message. */
export type StoredMessage = {
  id: string;
  role: "user" | "assistant";
  text: string;
  reasoning: string | null;
  state: StoredMessageState;
  providerId: string | null;
  modelId: string | null;
  providerRun: StoredProviderRun | null;
  createdAtMs: number;
};

/** One reconstructed durable conversation. */
export type StoredConversation = {
  id: string;
  title: string;
  messages: StoredMessage[];
};

/** Produces the stable browser-preview storage failure. */
function unavailableInBrowser(): StorageError {
  return {
    code: "internal",
    message: "Conversation storage is available only in the native Bottie app.",
  };
}

/** Lists recent conversations for the local profile. */
export async function listConversations(): Promise<ConversationSummary[]> {
  if (!isTauri()) return [];
  return invoke<ConversationSummary[]>("list_conversations");
}

/** Creates one empty durable conversation. */
export async function createConversation(title: string): Promise<StoredConversation> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<StoredConversation>("create_conversation", { title });
}

/** Loads one conversation and its ordered messages. */
export async function loadConversation(conversationId: string): Promise<StoredConversation> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<StoredConversation>("load_conversation", { conversationId });
}

/** Appends one final user message through the narrow native storage command. */
export async function appendConversationMessage(conversationId: string, text: string): Promise<StoredMessage> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<StoredMessage>("append_conversation_message", { conversationId, text });
}

/** Renames one active or archived conversation. */
export async function renameConversation(conversationId: string, title: string): Promise<ConversationSummary> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ConversationSummary>("rename_conversation", { conversationId, title });
}

/** Moves one conversation into or out of the archive. */
export async function setConversationArchived(conversationId: string, archived: boolean): Promise<ConversationSummary> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ConversationSummary>("set_conversation_archived", { conversationId, archived });
}

/** Moves one conversation to recoverable trash. */
export async function deleteConversation(conversationId: string): Promise<ConversationSummary> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ConversationSummary>("delete_conversation", { conversationId });
}

/** Restores one trashed conversation to the active recent list. */
export async function restoreConversation(conversationId: string): Promise<ConversationSummary> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ConversationSummary>("restore_conversation", { conversationId });
}

/** Selects one lifecycle group while preserving the native store's ordering. */
export function conversationsForLifecycle(
  conversations: ConversationSummary[],
  lifecycle: ConversationLifecycle,
): ConversationSummary[] {
  return conversations.filter((conversation) => conversation.lifecycle === lifecycle);
}

/** Groups active conversations by local calendar recency while preserving native ordering. */
export function activeConversationDateGroups(
  conversations: ConversationSummary[],
  nowMs = Date.now(),
): ConversationDateGroup[] {
  const groups = new Map<ConversationDateGroup["label"], ConversationSummary[]>();
  const today = localCalendarDay(nowMs);
  for (const conversation of conversationsForLifecycle(conversations, "active")) {
    const ageDays = today - localCalendarDay(conversation.updatedAtMs);
    const label = dateGroupLabel(ageDays);
    const group = groups.get(label);
    if (group) group.push(conversation);
    else groups.set(label, [conversation]);
  }
  return (["Today", "Yesterday", "Previous 7 days", "Older"] as const)
    .map((label) => ({ label, conversations: groups.get(label) ?? [] }))
    .filter((group) => group.conversations.length > 0);
}

/** Converts a timestamp's local calendar date into a timezone-independent day number. */
function localCalendarDay(timestampMs: number): number {
  const date = new Date(timestampMs);
  return Math.floor(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / MILLISECONDS_PER_DAY);
}

/** Selects the stable navigation label for a calendar-day age. */
function dateGroupLabel(ageDays: number): ConversationDateGroup["label"] {
  if (ageDays <= 0) return "Today";
  if (ageDays === 1) return "Yesterday";
  if (ageDays <= PREVIOUS_DAYS_LIMIT) return "Previous 7 days";
  return "Older";
}

/** Converts an unknown native error into Bottie's stable storage error shape. */
export function storageErrorFromUnknown(error: unknown): StorageError {
  if (typeof error === "object" && error !== null && "message" in error) {
    const candidate = error as Partial<StorageError>;
    if (typeof candidate.message === "string") {
      return { code: candidate.code ?? "internal", message: candidate.message };
    }
  }
  return {
    code: "internal",
    message: typeof error === "string" ? error : "Bottie could not access local conversation history.",
  };
}
