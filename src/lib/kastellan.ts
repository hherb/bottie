/**
 * `@kastellan:` requests: tasks handed to a Kastellan agent daemon, answered later in the same conversation.
 *
 * Native code runs one `kastellan-cli session` per request on this computer or over ssh, announces each notebook
 * entry as {@link KASTELLAN_PROGRESS_EVENT}, saves the reply as the durable response, and emits
 * {@link KASTELLAN_TASK_EVENT}. While the task runs, Stop and Correct reach it by run id. This module holds the IPC
 * wrappers and the pure presentation rules.
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
/** Native event emitted for each progress line of a running task; progress is transient, only the reply is saved. */
export const KASTELLAN_PROGRESS_EVENT = "kastellan-task-progress";
/** Most progress lines kept per running task; older lines scroll away. */
export const KASTELLAN_PROGRESS_LIMIT = 50;
/** Footer shown until native code confirms where a request was sent. */
export const KASTELLAN_SENDING_META = "Sending to Kastellan…";
/** Host used when none is configured. */
export const KASTELLAN_DEFAULT_HOST = "localhost";

/** Identity returned when a request is accepted, including where it was sent. */
export type KastellanRun = { runId: string; host: string };
/** Payload of {@link KASTELLAN_TASK_EVENT}. */
export type KastellanTaskFinished = { conversationId: string; runId: string };
/** Payload of {@link KASTELLAN_PROGRESS_EVENT}. */
export type KastellanTaskProgress = { conversationId: string; runId: string; line: string };
/** A Kastellan response still running: the run Stop and Correct address, and its progress so far. */
export type KastellanLive = { runId: string; progress: string[] };
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
): { meta: string | undefined; reasoning: undefined; model: string; live: KastellanLive | undefined } | null {
  if (message.providerId !== KASTELLAN_PROVIDER_ID) return null;
  const host = message.modelId ?? KASTELLAN_DEFAULT_HOST;
  const pending = isPendingKastellanResponse(message);
  const meta = pending ? kastellanWaitingMeta(host) : (message.reasoning?.trim() ?? undefined);
  // A reopened task shows its controls at once; its earlier progress returns with the next progress event.
  const live = pending && message.providerRun ? { runId: message.providerRun.id, progress: [] } : undefined;
  return { meta, reasoning: undefined, model: `Kastellan · ${host}`, live };
}

/** Appends one progress line, keeping the newest {@link KASTELLAN_PROGRESS_LIMIT}. Pure. */
export function appendKastellanProgress(progress: readonly string[], line: string): string[] {
  return [...progress, line].slice(-KASTELLAN_PROGRESS_LIMIT);
}

/** Accepts one prefixed request as a background native run. */
export async function startKastellanTask(context: ProviderRunContext, prompt: string): Promise<KastellanRun> {
  if (!isTauri()) throw new Error("Kastellan requests need the native Bottie app.");
  return invoke<KastellanRun>("start_kastellan_task", { context, prompt });
}

/** Cancels a running request's Kastellan task; the reply is saved as stopped once Kastellan confirms. */
export async function stopKastellanTask(runId: string): Promise<void> {
  return invoke<void>("stop_kastellan_task", { runId });
}

/** Adds an operator correction to a running request; Kastellan's planner reads it on its next turn. */
export async function correctKastellanTask(runId: string, text: string): Promise<void> {
  return invoke<void>("correct_kastellan_task", { runId, text });
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

/** Subscribes to progress lines of running tasks; resolves to undefined when listening is unavailable. */
export async function listenForKastellanProgress(
  onProgress: (event: KastellanTaskProgress) => void,
): Promise<UnlistenFn | undefined> {
  try {
    return await listen<KastellanTaskProgress>(KASTELLAN_PROGRESS_EVENT, (event) => onProgress(event.payload));
  } catch (error) {
    console.warn("Could not listen for Kastellan progress", error);
    return undefined;
  }
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
