/**
 * `@kastellan:` requests: tasks handed to a Kastellan agent daemon, answered later in the same conversation.
 *
 * Native code runs Kastellan's operator CLI on this computer or over ssh, saves the reply as the durable response, and
 * emits {@link KASTELLAN_TASK_EVENT}. This module holds the IPC wrappers and the pure presentation rules.
 */

import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { StoredMessage, ProviderRunContext } from "./storage";

/** Prompt prefix that routes a message to Kastellan instead of the selected model. */
export const KASTELLAN_PREFIX = "@kastellan:";
/** Provider identity native code records on Kastellan runs; the run's model identity is the host. */
export const KASTELLAN_PROVIDER_ID = "kastellan";
/** Native event emitted once a Kastellan run is durably closed. */
export const KASTELLAN_TASK_EVENT = "kastellan-task-finished";
/** Footer shown until native code confirms where a request was sent. */
export const KASTELLAN_SENDING_META = "Sending to Kastellan…";
/** Host used when none is configured. */
export const KASTELLAN_DEFAULT_HOST = "localhost";

/** Identity returned when a request is accepted, including where it was sent. */
export type KastellanRun = { runId: string; host: string };
/** Payload of {@link KASTELLAN_TASK_EVENT}. */
export type KastellanTaskFinished = { conversationId: string; runId: string };
/** Effective settings, including defaults. */
export type KastellanSettings = { host: string; cliPath: string | null; remote: boolean };

/** Whether a submitted prompt is addressed to Kastellan. */
export function isKastellanPrompt(prompt: string): boolean {
  return prompt.trimStart().startsWith(KASTELLAN_PREFIX);
}

/** Footer shown while a task is with Kastellan. */
export function kastellanWaitingMeta(host: string): string {
  return `Sent to Kastellan on ${host} · the reply will appear here`;
}

/** Whether a reopened response is a Kastellan task that has not answered yet. */
export function isPendingKastellanResponse(message: StoredMessage): boolean {
  return (
    message.providerId === KASTELLAN_PROVIDER_ID &&
    message.state === "partial" &&
    !message.providerRun?.errorCode &&
    message.text.length === 0
  );
}

/**
 * Presentation overrides for a reopened Kastellan response, or null for any other message.
 *
 * Native code stores the task provenance in the reasoning block; it is shown as the footer instead, because it is not
 * model reasoning.
 */
export function kastellanResponsePresentation(
  message: StoredMessage,
): { meta: string | undefined; reasoning: undefined; model: string } | null {
  if (message.providerId !== KASTELLAN_PROVIDER_ID) return null;
  const host = message.modelId ?? KASTELLAN_DEFAULT_HOST;
  const meta = isPendingKastellanResponse(message)
    ? kastellanWaitingMeta(host)
    : (message.reasoning?.trim() ?? undefined);
  return { meta, reasoning: undefined, model: `Kastellan · ${host}` };
}

/** Accepts one prefixed request as a background native run. */
export async function startKastellanTask(context: ProviderRunContext, prompt: string): Promise<KastellanRun> {
  if (!isTauri()) throw new Error("Kastellan requests need the native Bottie app.");
  return invoke<KastellanRun>("start_kastellan_task", { context, prompt });
}

/** Reads the effective Kastellan settings. */
export async function getKastellanSettings(): Promise<KastellanSettings> {
  return invoke<KastellanSettings>("get_kastellan_settings");
}

/** Saves the host and optional CLI path; blank values restore the defaults. */
export async function updateKastellanSettings(host: string, cliPath: string): Promise<KastellanSettings> {
  return invoke<KastellanSettings>("update_kastellan_settings", { host, cliPath });
}

/** Tests unsaved drafts without submitting a task; resolves to a success message. */
export async function testKastellanConnection(host: string, cliPath: string): Promise<string> {
  return invoke<string>("test_kastellan_connection", { host, cliPath });
}

/** Subscribes to background completions; resolves to undefined when listening is unavailable. */
export async function listenForKastellanTasks(
  onFinished: (event: KastellanTaskFinished) => void,
): Promise<UnlistenFn | undefined> {
  try {
    return await listen<KastellanTaskFinished>(KASTELLAN_TASK_EVENT, (event) => onFinished(event.payload));
  } catch (error) {
    console.warn("Could not listen for Kastellan replies", error);
    return undefined;
  }
}
