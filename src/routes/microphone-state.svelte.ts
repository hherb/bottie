/** Reactive controller for Bottie's path-free native microphone state. */

import { isTauri } from "@tauri-apps/api/core";

import {
  discardMicrophoneCapture,
  getMicrophoneStatus,
  INITIAL_MICROPHONE_STATUS,
  startMicrophoneCapture,
  stopMicrophoneCapture,
  type MicrophoneStatus,
} from "$lib/microphone";

const STATUS_POLL_INTERVAL_MS = 150;

/** Owns native microphone actions and bounded status polling for the composer. */
export class MicrophoneState {
  status = $state<MicrophoneStatus>({ ...INITIAL_MICROPHONE_STATUS });
  readonly available = isTauri();

  private pollTimer?: ReturnType<typeof setInterval>;
  private pollInFlight = false;

  /** Reads status without requesting permission or opening an input device. */
  async initialize(): Promise<void> {
    if (!this.available) return;
    await this.refresh();
    if (this.isActive) this.startPolling();
  }

  /** Whether permission or native audio capture currently owns the microphone action. */
  get isActive(): boolean {
    return this.status.phase === "starting" || this.status.phase === "recording";
  }

  /** Begins capture only from the composer's explicit user action. */
  async start(): Promise<void> {
    if (!this.available || this.isActive) return;
    try {
      this.status = await startMicrophoneCapture();
      this.startPolling();
    } catch {
      this.failClosed();
    }
  }

  /** Stops active input while retaining the bounded samples only in native memory. */
  async stop(): Promise<void> {
    if (!this.available || !this.isActive) return;
    try {
      this.status = await stopMicrophoneCapture();
      this.startPolling();
    } catch {
      this.failClosed();
    }
  }

  /** Discards every retained native sample and stops status polling. */
  async discard(): Promise<void> {
    if (!this.available) return;
    try {
      this.status = await discardMicrophoneCapture();
    } catch {
      this.failClosed();
    }
    this.stopPolling();
  }

  /** Releases the local timer; native samples remain governed by the explicit discard action or process lifetime. */
  dispose(): void {
    this.stopPolling();
  }

  private async refresh(): Promise<void> {
    if (this.pollInFlight) return;
    this.pollInFlight = true;
    try {
      this.status = await getMicrophoneStatus();
      if (!this.isActive) this.stopPolling();
    } catch {
      this.failClosed();
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

  private failClosed(): void {
    this.status = { ...INITIAL_MICROPHONE_STATUS, phase: "error", errorCode: "capture_failed" };
    this.stopPolling();
  }
}
