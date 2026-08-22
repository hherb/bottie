/** Path-free frontend contract for Bottie's native Trash retention policy. */

import { invoke, isTauri } from "@tauri-apps/api/core";

/** Bounded periods accepted by the native retention command. */
export type ConversationRetentionPeriod = "forever" | "thirty_days" | "ninety_days" | "one_year";

/** Durable native retention state for the built-in local profile. */
export type ConversationRetentionPolicy = {
  period: ConversationRetentionPeriod;
};

/** One user-facing bounded retention choice. */
export type ConversationRetentionOption = {
  value: ConversationRetentionPeriod;
  label: string;
};

/** Complete set of retention periods accepted by Rust. */
export const CONVERSATION_RETENTION_OPTIONS: ConversationRetentionOption[] = [
  { value: "forever", label: "Keep until I forget manually" },
  { value: "thirty_days", label: "Forget after 30 days in Trash" },
  { value: "ninety_days", label: "Forget after 90 days in Trash" },
  { value: "one_year", label: "Forget after 1 year in Trash" },
];

/** Loads the durable retention policy without receiving database or filesystem details. */
export async function getConversationRetentionPolicy(): Promise<ConversationRetentionPolicy> {
  if (!isTauri()) throw new Error("Conversation retention is available only in the native Bottie app.");
  return invoke<ConversationRetentionPolicy>("get_conversation_retention_policy");
}

/** Saves one typed period; expiry is applied only on a later healthy native startup. */
export async function setConversationRetentionPeriod(
  period: ConversationRetentionPeriod,
): Promise<ConversationRetentionPolicy> {
  if (!isTauri()) throw new Error("Conversation retention is available only in the native Bottie app.");
  return invoke<ConversationRetentionPolicy>("set_conversation_retention_period", { period });
}

/** Explains the irreversible effect and retained external data for one draft period. */
export function conversationRetentionDisclosure(period: ConversationRetentionPeriod): string {
  if (period === "forever") {
    return "Bottie keeps conversations in Trash until you choose Forget permanently.";
  }
  const duration = period === "thirty_days" ? "30 days" : period === "ninety_days" ? "90 days" : "1 year";
  return (
    `Bottie keeps conversations in Trash for ${duration}, then permanently removes expired live-store data on the ` +
    "next healthy app launch. This can include conversations already in Trash. Unshared files keep the 24-hour " +
    "safety window; existing exports and backups are unchanged, and the app-owned model cache is retained."
  );
}
