/** Typed frontend boundary for Rust-owned durable conversation storage. */

import { invoke, isTauri } from "@tauri-apps/api/core";

/** Durable message states supported by the initial SQLite schema. */
export type StoredMessageState = "partial" | "final" | "cancelled" | "failed";

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
};

/** One persisted text/reasoning message. */
export type StoredMessage = {
  id: string;
  role: "user" | "assistant";
  text: string;
  reasoning: string | null;
  state: StoredMessageState;
  providerId: string | null;
  modelId: string | null;
  createdAtMs: number;
};

/** One reconstructed durable conversation. */
export type StoredConversation = {
  id: string;
  title: string;
  messages: StoredMessage[];
};

/** Input accepted when appending one immutable message. */
export type NewStoredMessage = {
  conversationId: string;
  role: "user" | "assistant";
  text: string;
  reasoning: string | null;
  state: StoredMessageState;
  providerId: string | null;
  modelId: string | null;
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

/** Appends one immutable text/reasoning message. */
export async function appendConversationMessage(message: NewStoredMessage): Promise<StoredMessage> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<StoredMessage>("append_conversation_message", { message });
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
