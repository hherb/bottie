/** Typed frontend boundary for Rust-owned durable conversation storage. */

import { invoke, isTauri } from "@tauri-apps/api/core";

import type { Usage } from "./inference";

/** Durable message states supported by the initial SQLite schema. */
export type StoredMessageState = "partial" | "final" | "cancelled" | "failed";

/** Local quality rating attached to one durable assistant response. */
export type ResponseRating = "good" | "poor";

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
  code: "invalid_request" | "not_found" | "recovery_required" | "internal";
  message: string;
};

/** Path-redacted native state for startup integrity and automatic recovery. */
export type StorageRecoveryStatus = {
  state: "ready" | "recovery_required";
  automaticBackupCount: number;
  latestAutomaticBackupAtMs: number | null;
};

/** One conversation row used by navigation. */
export type ConversationSummary = {
  id: string;
  title: string;
  updatedAtMs: number;
  lifecycle: ConversationLifecycle;
};

/** One native-ranked conversation search result that opens the branch containing its match. */
export type ConversationSearchResult = {
  conversationId: string;
  title: string;
  snippet: string;
  branchId: string;
  updatedAtMs: number;
  lifecycle: Exclude<ConversationLifecycle, "deleted">;
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
  rating: ResponseRating | null;
  createdAtMs: number;
};

/** One selectable durable conversation branch. */
export type ConversationBranch = {
  id: string;
  name: string;
};

/** One reconstructed durable conversation. */
export type StoredConversation = {
  id: string;
  title: string;
  currentBranchId: string;
  branches: ConversationBranch[];
  messages: StoredMessage[];
};

/** Native result for one atomic edit-and-regenerate branch creation. */
export type ForkedConversation = {
  conversation: StoredConversation;
  requestMessageId: string;
};

/** Native Save-dialog result that deliberately omits the selected filesystem path. */
export type ConversationExportOutcome = {
  status: "saved" | "cancelled";
  fileName: string | null;
};

/** Native backup result that deliberately omits the selected filesystem path. */
export type BackupOutcome = {
  status: "saved" | "cancelled";
  fileName: string | null;
};

/** Native restore result that returns leaf filenames but never filesystem paths. */
export type RestoreOutcome = {
  status: "restored" | "cancelled";
  fileName: string | null;
  preservedCopyName: string | null;
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

/** Searches active and archived conversation titles and user-visible message text. */
export async function searchConversations(query: string): Promise<ConversationSearchResult[]> {
  if (!isTauri()) return [];
  return invoke<ConversationSearchResult[]>("search_conversations", { query });
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

/** Saves the selected visible lineage as Markdown without exposing its destination path. */
export async function exportConversationMarkdown(conversationId: string): Promise<ConversationExportOutcome> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ConversationExportOutcome>("export_conversation_markdown", { conversationId });
}

/** Saves a complete SQLite snapshot without exposing its destination path. */
export async function backupConversationStore(): Promise<BackupOutcome> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<BackupOutcome>("backup_conversation_store");
}

/** Restores a validated SQLite snapshot after a native-owned safety backup and confirmation. */
export async function restoreConversationStore(): Promise<RestoreOutcome> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<RestoreOutcome>("restore_conversation_store");
}

/** Reads corruption state and verified automatic-backup availability without receiving native paths. */
export async function getStorageRecoveryStatus(): Promise<StorageRecoveryStatus> {
  if (!isTauri()) {
    return { state: "ready", automaticBackupCount: 0, latestAutomaticBackupAtMs: null };
  }
  return invoke<StorageRecoveryStatus>("get_storage_recovery_status");
}

/** Restores the newest verified app-private automatic backup through native confirmation. */
export async function restoreLatestAutomaticBackup(): Promise<RestoreOutcome> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<RestoreOutcome>("restore_latest_automatic_backup");
}

/** Loads the exact conversation selected by the local profile, when present. */
export async function loadLastOpenConversation(): Promise<StoredConversation | null> {
  if (!isTauri()) return null;
  return invoke<StoredConversation | null>("load_last_open_conversation");
}

/** Records an intentional blank new-chat view for the local profile. */
export async function clearLastOpenConversation(): Promise<void> {
  if (!isTauri()) return;
  return invoke<void>("clear_last_open_conversation");
}

/** Appends one final user message through the narrow native storage command. */
export async function appendConversationMessage(conversationId: string, text: string): Promise<StoredMessage> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<StoredMessage>("append_conversation_message", { conversationId, text });
}

/** Forks one visible user message onto a newly selected durable branch. */
export async function branchConversationMessage(
  conversationId: string,
  messageId: string,
  text: string,
): Promise<ForkedConversation> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ForkedConversation>("branch_conversation_message", { conversationId, messageId, text });
}

/** Selects one durable branch and reconstructs its visible message lineage. */
export async function selectConversationBranch(conversationId: string, branchId: string): Promise<StoredConversation> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<StoredConversation>("select_conversation_branch", { conversationId, branchId });
}

/** Sets or clears the local quality rating for one durable assistant response. */
export async function rateConversationResponse(
  conversationId: string,
  messageId: string,
  rating: ResponseRating | null,
): Promise<ResponseRating | null> {
  if (!isTauri()) throw unavailableInBrowser();
  return invoke<ResponseRating | null>("rate_conversation_response", { conversationId, messageId, rating });
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
