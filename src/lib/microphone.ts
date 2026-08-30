import { invoke } from "@tauri-apps/api/core";

/** Path-free capture phase returned by Bottie's native microphone worker. */
export type MicrophonePhase = "idle" | "starting" | "recording" | "captured" | "error";

/** Operating-system microphone authorization state without device or account identity. */
export type MicrophonePermission = "prompt" | "granted" | "denied" | "unavailable";

/** Stable native microphone failure categories safe to show in the WebView. */
export type MicrophoneErrorCode =
  "permission_denied" | "device_unavailable" | "device_busy" | "unsupported_format" | "capture_failed";

/** Bounded native capture metadata that deliberately omits audio samples and device identity. */
export type MicrophoneStatus = {
  phase: MicrophonePhase;
  permission: MicrophonePermission;
  durationMs: number;
  maxDurationMs: number;
  sampleRateHz: number | null;
  channels: number | null;
  retainedByteSize: number;
  inputLevel: number;
  errorCode: MicrophoneErrorCode | null;
};

/** Initial disconnected-preview state before a native microphone request exists. */
export const INITIAL_MICROPHONE_STATUS: MicrophoneStatus = {
  phase: "idle",
  permission: "prompt",
  durationMs: 0,
  maxDurationMs: 60_000,
  sampleRateHz: null,
  channels: null,
  retainedByteSize: 0,
  inputLevel: 0,
  errorCode: null,
};

/** Reads current native capture metadata without opening an input device. */
export async function getMicrophoneStatus(): Promise<MicrophoneStatus> {
  return invoke<MicrophoneStatus>("get_microphone_status");
}

/** Requests default-input capture after the composer's explicit user action. */
export async function startMicrophoneCapture(): Promise<MicrophoneStatus> {
  return invoke<MicrophoneStatus>("start_microphone_capture");
}

/** Stops native input while retaining its bounded session-only PCM buffer. */
export async function stopMicrophoneCapture(): Promise<MicrophoneStatus> {
  return invoke<MicrophoneStatus>("stop_microphone_capture");
}

/** Clears the native session-only PCM buffer without returning any sample. */
export async function discardMicrophoneCapture(): Promise<MicrophoneStatus> {
  return invoke<MicrophoneStatus>("discard_microphone_capture");
}

/** Formats a native millisecond duration as a stable minutes-and-seconds label. */
export function formatMicrophoneDuration(durationMs: number): string {
  const totalSeconds = Math.max(0, Math.floor(durationMs / 1_000));
  return `${Math.floor(totalSeconds / 60)}:${String(totalSeconds % 60).padStart(2, "0")}`;
}

/** Returns one calm trust-boundary or failure message from path-free native state. */
export function microphoneFeedback(status: MicrophoneStatus): string {
  if (status.phase === "starting") return "Waiting for microphone permission…";
  if (status.phase === "recording") {
    return [
      "Recording locally",
      `${formatMicrophoneDuration(status.durationMs)} of ${formatMicrophoneDuration(status.maxDurationMs)}`,
    ].join(" · ");
  }
  if (status.phase === "captured") {
    return `Voice capture · ${formatMicrophoneDuration(status.durationMs)} · held only in native memory`;
  }
  if (status.errorCode === "permission_denied") {
    return "Microphone access was denied. Allow Bottie in system privacy settings, then try again.";
  }
  if (status.errorCode === "device_unavailable") {
    return "No microphone is available. Connect or enable an input device, then try again.";
  }
  if (status.errorCode === "device_busy") {
    return "The microphone is busy in another application. Close it there, then try again.";
  }
  if (status.errorCode === "unsupported_format") {
    return "This microphone format is not supported yet. Try another input device.";
  }
  if (status.phase === "error") return "Voice capture stopped unexpectedly. Discard it and try again.";
  return "Microphone access is requested only when you choose Record voice.";
}
