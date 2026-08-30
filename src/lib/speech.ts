/** Typed path-free WebView contract for Rust-owned local text-to-speech. */

import { invoke } from "@tauri-apps/api/core";
import MarkdownIt from "markdown-it";

/** Exact UTF-8 ceiling enforced again by the native speech controller. */
export const MAX_SPEECH_TEXT_BYTES = 32 * 1_024;

/** Local speech lifecycle without utterance text or backend detail. */
export type SpeechPhase = "idle" | "speaking" | "error";

/** Stable native speech failure categories. */
export type SpeechErrorCode = "unavailable" | "playback_failed";

/** Bounded local voice metadata supplied by the native speech engine. */
export type SpeechVoice = {
  id: string;
  name: string;
  language: string;
};

/** Current local playback state without message or utterance content. */
export type SpeechStatus = {
  phase: SpeechPhase;
  selectedVoiceId: string | null;
  errorCode: SpeechErrorCode | null;
};

/** Initial state before native voice discovery is explicitly requested. */
export const INITIAL_SPEECH_STATUS: SpeechStatus = {
  phase: "idle",
  selectedVoiceId: null,
  errorCode: null,
};

type SpeechToken = {
  type: string;
  content: string;
  children: SpeechToken[] | null;
};

const speechMarkdown = new MarkdownIt({
  breaks: false,
  html: false,
  linkify: true,
  typographer: false,
});

/** Adds visible text from parser-owned Markdown tokens without link destinations or markup. */
function collectSpeechText(tokens: SpeechToken[], parts: string[]): void {
  for (const token of tokens) {
    if (["text", "code_inline", "code_block", "fence", "image"].includes(token.type)) {
      parts.push(token.content);
    } else if (["softbreak", "hardbreak"].includes(token.type)) {
      parts.push(" ");
    } else if (token.children) {
      collectSpeechText(token.children, parts);
    }
  }
}

/** Derives normalized visible assistant text for local speech from safe Markdown tokens. */
export function assistantSpeechText(source: string): string {
  const parts: string[] = [];
  collectSpeechText(speechMarkdown.parse(source, {}) as SpeechToken[], parts);
  return parts
    .join(" ")
    .replaceAll(/\s+/g, " ")
    .replaceAll(/\s+([,.;:!?])/g, "$1")
    .trim();
}

/** Returns whether one non-empty utterance fits the native UTF-8 ceiling. */
export function speechTextWithinLimit(text: string): boolean {
  const normalized = text.replaceAll(/\s+/g, " ").trim();
  return normalized.length > 0 && new TextEncoder().encode(normalized).byteLength <= MAX_SPEECH_TEXT_BYTES;
}

/** Returns current path-free local speech state. */
export async function getSpeechStatus(): Promise<SpeechStatus> {
  return invoke<SpeechStatus>("get_speech_status");
}

/** Lazily lists bounded native voices. */
export async function listSpeechVoices(): Promise<SpeechVoice[]> {
  return invoke<SpeechVoice[]>("list_speech_voices");
}

/** Selects one exact local voice for this process lifetime. */
export async function selectSpeechVoice(voiceId: string): Promise<SpeechStatus> {
  return invoke<SpeechStatus>("select_speech_voice", { voiceId });
}

/** Plays one bounded text payload through Bottie's selected local voice. */
export async function speakText(text: string): Promise<SpeechStatus> {
  return invoke<SpeechStatus>("speak_text", { text });
}

/** Stops only Bottie's current local speech. */
export async function stopSpeech(): Promise<SpeechStatus> {
  return invoke<SpeechStatus>("stop_speech");
}

/** Builds calm user-facing local playback feedback from fixed path-free state. */
export function speechFeedback(status: SpeechStatus, voiceCount: number): string {
  if (status.phase === "speaking") return "Playing locally · use Stop to end playback";
  if (status.errorCode === "unavailable") return "Local voices are unavailable on this device.";
  if (status.errorCode === "playback_failed") return "Local playback failed. Choose another voice and try again.";
  return `${voiceCount} local ${voiceCount === 1 ? "voice" : "voices"} available · playback stays on this device`;
}
