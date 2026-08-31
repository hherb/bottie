/** Reactive controller for Bottie's path-free native microphone state. */

import { isTauri } from "@tauri-apps/api/core";

import {
  correctMicrophoneTranscript,
  discardMicrophoneCapture,
  getMicrophoneStatus,
  INITIAL_MICROPHONE_DEVICE_LIST,
  INITIAL_MICROPHONE_STATUS,
  listMicrophoneInputDevices,
  selectMicrophoneInputDevice,
  startMicrophoneCapture,
  stopMicrophoneCapture,
  type MicrophoneInputDeviceList,
  type MicrophoneStatus,
} from "$lib/microphone";

const STATUS_POLL_INTERVAL_MS = 150;

/** Owns native microphone actions and bounded status polling for the composer. */
export class MicrophoneState {
  status = $state<MicrophoneStatus>({ ...INITIAL_MICROPHONE_STATUS });
  deviceList = $state<MicrophoneInputDeviceList>({
    ...INITIAL_MICROPHONE_DEVICE_LIST,
    devices: [...INITIAL_MICROPHONE_DEVICE_LIST.devices],
  });
  devicesLoaded = $state(false);
  deviceListFailed = $state(false);
  sendAudio = $state(false);
  retainAudio = $state(false);
  readonly available = isTauri();

  private pollTimer?: ReturnType<typeof setInterval>;
  private pollInFlight = false;
  private deviceRequestInFlight = false;

  /** Reads status without requesting permission or opening an input device. */
  async initialize(): Promise<void> {
    if (!this.available) return;
    await this.refresh();
    if (this.isActive) this.startPolling();
  }

  /** Lazily discovers bounded microphone choices after the user's explicit action. */
  async loadDevices(): Promise<void> {
    if (!this.available || this.isActive || this.deviceRequestInFlight) return;
    this.deviceRequestInFlight = true;
    this.deviceListFailed = false;
    try {
      this.deviceList = await listMicrophoneInputDevices();
      this.devicesLoaded = true;
    } catch {
      this.deviceListFailed = true;
    } finally {
      this.deviceRequestInFlight = false;
    }
  }

  /** Selects one current opaque microphone token for this app session only. */
  async selectDevice(token: string): Promise<void> {
    if (!this.available || this.isActive || this.deviceRequestInFlight) return;
    this.deviceRequestInFlight = true;
    this.deviceListFailed = false;
    try {
      this.deviceList = await selectMicrophoneInputDevice(token);
      this.devicesLoaded = true;
    } catch {
      this.deviceListFailed = true;
    } finally {
      this.deviceRequestInFlight = false;
    }
  }

  /** Whether permission or native audio capture currently owns the microphone action. */
  get isActive(): boolean {
    return (
      this.status.phase === "starting" ||
      this.status.phase === "recording" ||
      this.status.transcriptionPhase === "preparing_model" ||
      this.status.transcriptionPhase === "transcribing"
    );
  }

  /** Begins capture only from the composer's explicit user action. */
  async start(): Promise<void> {
    if (!this.available || this.isActive) return;
    this.resetDeliveryChoices();
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
    this.resetDeliveryChoices();
  }

  /** Toggles explicit provider delivery for the stopped capture. */
  toggleSendAudio(available: boolean): void {
    if (this.status.phase !== "captured" || (!available && !this.sendAudio)) return;
    this.sendAudio = !this.sendAudio;
  }

  /** Toggles optional durable local retention independently from provider delivery. */
  toggleRetainAudio(): void {
    if (this.status.phase !== "captured") return;
    this.retainAudio = !this.retainAudio;
  }

  /** Refreshes native state after accepted delivery consumed the session capture. */
  async refreshAfterSubmission(): Promise<void> {
    await this.refresh();
    if (this.status.phase !== "captured") this.resetDeliveryChoices();
  }

  /** Replaces one final voice turn while keeping the corrected text in native session memory only. */
  async correct(turnIndex: number, text: string): Promise<void> {
    if (!this.available || this.status.transcriptionPhase !== "ready") return;
    try {
      this.status = await correctMicrophoneTranscript(turnIndex, text);
    } catch {
      await this.refresh();
    }
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

  private resetDeliveryChoices(): void {
    this.sendAudio = false;
    this.retainAudio = false;
  }
}
