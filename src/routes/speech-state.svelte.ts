/** Reactive controller for bounded Rust-owned local text-to-speech. */

import { isTauri } from "@tauri-apps/api/core";

import {
  assistantSpeechText,
  getSpeechStatus,
  INITIAL_SPEECH_STATUS,
  listSpeechVoices,
  selectSpeechVoice,
  speechTextWithinLimit,
  speakText,
  stopSpeech,
  type SpeechStatus,
  type SpeechVoice,
} from "$lib/speech";

const STATUS_POLL_INTERVAL_MS = 180;

/** Owns local voice discovery, playback actions, and path-free polling state. */
export class SpeechState {
  status = $state<SpeechStatus>({ ...INITIAL_SPEECH_STATUS });
  voices = $state<SpeechVoice[]>([]);
  activeMessageId = $state<number | null>(null);
  available = $state(isTauri());

  private readonly nativeAvailable = isTauri();
  private pollTimer?: ReturnType<typeof setInterval>;
  private pollInFlight = false;

  /** Lazily discovers local voices without starting playback. */
  async initialize(): Promise<void> {
    if (!this.nativeAvailable) return;
    try {
      const [voices, status] = await Promise.all([listSpeechVoices(), getSpeechStatus()]);
      this.voices = voices;
      this.status = status;
    } catch {
      this.failClosed("unavailable");
    }
  }

  /** Selects one engine-provided voice for the current process lifetime. */
  async selectVoice(voiceId: string): Promise<void> {
    if (!this.nativeAvailable || !this.voices.some((voice) => voice.id === voiceId)) return;
    if (this.status.phase === "speaking") await this.stop();
    try {
      this.status = await selectSpeechVoice(voiceId);
    } catch {
      this.failClosed("playback_failed");
    }
  }

  /** Plays the visible text of one completed assistant response after explicit user action. */
  async speak(messageId: number, markdown: string): Promise<void> {
    if (!this.nativeAvailable || this.voices.length === 0) return;
    const text = assistantSpeechText(markdown);
    if (!speechTextWithinLimit(text)) return;
    try {
      this.status = await speakText(text);
      this.activeMessageId = this.status.phase === "speaking" ? messageId : null;
      if (this.activeMessageId !== null) this.startPolling();
    } catch {
      this.failClosed("playback_failed");
    }
  }

  /** Stops only Bottie's local speech and clears the active response marker. */
  async stop(): Promise<boolean> {
    if (!this.nativeAvailable) return false;
    try {
      this.status = await stopSpeech();
    } catch {
      this.failClosed("playback_failed");
    }
    this.activeMessageId = null;
    this.stopPolling();
    return this.status.phase === "idle";
  }

  /** Stops local polling and playback when the page is unmounted. */
  dispose(): void {
    this.stopPolling();
    if (this.nativeAvailable && this.status.phase === "speaking") void stopSpeech();
  }

  /** Applies a deterministic disconnected browser fixture for presentation review. */
  applyPreview(voices: SpeechVoice[], status: SpeechStatus): void {
    if (this.nativeAvailable) return;
    this.available = true;
    this.voices = voices;
    this.status = status;
    this.activeMessageId = status.phase === "speaking" ? 2 : null;
  }

  private async refresh(): Promise<void> {
    if (this.pollInFlight) return;
    this.pollInFlight = true;
    try {
      this.status = await getSpeechStatus();
      if (this.status.phase !== "speaking") {
        this.activeMessageId = null;
        this.stopPolling();
      }
    } catch {
      this.failClosed("playback_failed");
    } finally {
      this.pollInFlight = false;
    }
  }

  private startPolling(): void {
    if (this.pollTimer) return;
    this.pollTimer = setInterval(() => void this.refresh(), STATUS_POLL_INTERVAL_MS);
  }

  private stopPolling(): void {
    if (this.pollTimer) clearInterval(this.pollTimer);
    this.pollTimer = undefined;
  }

  private failClosed(errorCode: "unavailable" | "playback_failed"): void {
    this.status = { ...this.status, phase: "error", errorCode };
    this.activeMessageId = null;
    this.stopPolling();
  }
}
