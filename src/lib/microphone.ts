import { invoke } from "@tauri-apps/api/core";

import { formatNativeLatency } from "$lib/latency";

/** Path-free capture phase returned by Bottie's native microphone worker. */
export type MicrophonePhase = "idle" | "starting" | "recording" | "captured" | "error";

/** Operating-system microphone authorization state without device or account identity. */
export type MicrophonePermission = "prompt" | "granted" | "denied" | "unavailable";

/** Stable native microphone failure categories safe to show in the WebView. */
export type MicrophoneErrorCode =
  | "permission_denied"
  | "device_unavailable"
  | "selected_device_unavailable"
  | "device_busy"
  | "unsupported_format"
  | "capture_failed";

/** One bounded display label and process-local opaque token without native device identity. */
export type MicrophoneInputDevice = {
  token: string;
  label: string;
  isSystemDefault: boolean;
};

/** Current microphone choices with one Rust-restored opaque selection. */
export type MicrophoneInputDeviceList = {
  devices: MicrophoneInputDevice[];
  selectedToken: string;
  selectionAvailable: boolean;
};

/** Default-only state before native microphone discovery is explicitly requested. */
export const INITIAL_MICROPHONE_DEVICE_LIST: MicrophoneInputDeviceList = {
  devices: [{ token: "system-default", label: "System default", isSystemDefault: true }],
  selectedToken: "system-default",
  selectionAvailable: true,
};

/** Stable path-free classification produced by Bottie's native voice activity detector. */
export type VoiceActivity = "silence" | "speech";

/** One contiguous path-free activity range within the bounded capture. */
export type VoiceSegment = {
  activity: VoiceActivity;
  startMs: number;
  endMs: number;
};

/** Path-free lifecycle of Bottie's native local speech recognizer. */
export type TranscriptionPhase = "idle" | "listening" | "preparing_model" | "transcribing" | "ready" | "error";

/** Stable local-recognition failures without model, cache, or runtime details. */
export type TranscriptionErrorCode = "model_unavailable" | "model_integrity" | "recognition_failed";

/** One bounded local transcript range whose finality is explicit. */
export type TranscriptSegment = {
  text: string;
  startMs: number;
  endMs: number;
  isFinal: boolean;
  isCorrected: boolean;
};

/** Maximum UTF-8 byte length accepted for one session-only transcript correction. */
export const MAX_TRANSCRIPT_TURN_BYTES = 512;

/** Maximum UTF-8 byte length accepted when explicitly composing one local text draft. */
export const MAX_COMPOSER_DRAFT_BYTES = 32 * 1_024;

/** Result of explicitly merging the current visible transcript into the composer. */
export type TranscriptComposerDraftResult =
  { ok: true; draft: string; mode: "inserted" | "appended" } | { ok: false; reason: "unavailable" | "limit_exceeded" };

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
  voiceActivity: VoiceActivity | null;
  voiceSegments: VoiceSegment[];
  transcriptionPhase: TranscriptionPhase;
  transcriptSegments: TranscriptSegment[];
  transcriptionErrorCode: TranscriptionErrorCode | null;
  errorCode: MicrophoneErrorCode | null;
  latency: MicrophoneLatency;
};

/** Native-observable monotonic intervals for the current capture action. */
export type MicrophoneLatency = {
  inputReadyMs: number | null;
  firstTranscriptMs: number | null;
  finalTranscriptMs: number | null;
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
  voiceActivity: null,
  voiceSegments: [],
  transcriptionPhase: "idle",
  transcriptSegments: [],
  transcriptionErrorCode: null,
  errorCode: null,
  latency: { inputReadyMs: null, firstTranscriptMs: null, finalTranscriptMs: null },
};

/** Reads current native capture metadata without opening an input device. */
export async function getMicrophoneStatus(): Promise<MicrophoneStatus> {
  return invoke<MicrophoneStatus>("get_microphone_status");
}

/** Lists bounded microphone labels and process-local opaque tokens without opening an input. */
export async function listMicrophoneInputDevices(): Promise<MicrophoneInputDeviceList> {
  return invoke<MicrophoneInputDeviceList>("list_microphone_input_devices");
}

/** Selects one current opaque microphone token without persisting it. */
export async function selectMicrophoneInputDevice(token: string): Promise<MicrophoneInputDeviceList> {
  return invoke<MicrophoneInputDeviceList>("select_microphone_input_device", { token });
}

/** Requests selected-input capture after the composer's explicit user action. */
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

/** Replaces one final transcript turn in bounded session-only native memory. */
export async function correctMicrophoneTranscript(turnIndex: number, text: string): Promise<MicrophoneStatus> {
  return invoke<MicrophoneStatus>("correct_microphone_transcript", { turnIndex, text });
}

/** Trims one correction and rejects blank or over-limit UTF-8 before native validation. */
export function normalizeTranscriptCorrection(value: string): string | null {
  const normalized = value.trim();
  if (!normalized || new TextEncoder().encode(normalized).byteLength > MAX_TRANSCRIPT_TURN_BYTES) return null;
  return normalized;
}

/** Reports whether one path-free status has a current, non-empty final transcript. */
export function canUseTranscriptAsText(status: MicrophoneStatus): boolean {
  return transcriptTextForDraft(status) !== null;
}

/** Builds an editable unsent draft without mutating or consuming native capture state. */
export function buildTranscriptComposerDraft(
  currentDraft: string,
  status: MicrophoneStatus,
): TranscriptComposerDraftResult {
  const transcript = transcriptTextForDraft(status);
  if (transcript === null) return { ok: false, reason: "unavailable" };
  const mode = currentDraft.length > 0 ? "appended" : "inserted";
  const draft = mode === "appended" ? `${currentDraft}\n\n${transcript}` : transcript;
  if (new TextEncoder().encode(draft).byteLength > MAX_COMPOSER_DRAFT_BYTES) {
    return { ok: false, reason: "limit_exceeded" };
  }
  return { ok: true, draft, mode };
}

/** Returns ordered visible final turns without requesting hidden native transcript state. */
function transcriptTextForDraft(status: MicrophoneStatus): string | null {
  if (
    status.phase !== "captured" ||
    status.transcriptionPhase !== "ready" ||
    status.transcriptSegments.length === 0 ||
    status.transcriptSegments.some((segment) => !segment.isFinal)
  ) {
    return null;
  }
  const transcript = status.transcriptSegments.map((segment) => segment.text).join("\n");
  return transcript.trim().length > 0 ? transcript : null;
}

/** Formats a native millisecond duration as a stable minutes-and-seconds label. */
export function formatMicrophoneDuration(durationMs: number): string {
  const totalSeconds = Math.max(0, Math.floor(durationMs / 1_000));
  return `${Math.floor(totalSeconds / 60)}:${String(totalSeconds % 60).padStart(2, "0")}`;
}

/** Returns one calm trust-boundary or failure message from path-free native state. */
export function microphoneFeedback(status: MicrophoneStatus): string {
  if (status.phase === "starting") return "Waiting for microphone permission…";
  if (status.transcriptionPhase === "preparing_model") {
    return "Preparing the local speech model · audio remains only in native memory";
  }
  if (status.transcriptionPhase === "error") {
    if (status.transcriptionErrorCode === "model_integrity") {
      return "The local speech model failed its integrity check. Discard this capture and try again.";
    }
    if (status.transcriptionErrorCode === "model_unavailable") {
      return "The local speech model is unavailable. Check your connection, then record again.";
    }
    return "Local speech recognition failed. The captured audio remains only in native memory.";
  }
  if (status.phase === "recording") {
    const activity = status.voiceActivity === "speech" ? "Speech detected" : "Listening for speech";
    return [
      "Recording locally",
      activity,
      `${formatMicrophoneDuration(status.durationMs)} of ${formatMicrophoneDuration(status.maxDurationMs)}`,
    ].join(" · ");
  }
  if (status.phase === "captured") {
    if (status.transcriptionPhase === "ready" && status.transcriptSegments.length > 0) {
      return "Transcript ready locally · audio held only in native memory";
    }
    if (status.transcriptionPhase === "ready") {
      return "No speech was transcribed · audio held only in native memory";
    }
    const speechMs = status.voiceSegments
      .filter((segment) => segment.activity === "speech")
      .reduce((total, segment) => total + Math.max(0, segment.endMs - segment.startMs), 0);
    return [
      "Voice capture",
      formatMicrophoneDuration(status.durationMs),
      `${formatMicrophoneDuration(speechMs)} speech`,
      "held only in native memory",
    ].join(" · ");
  }
  if (status.errorCode === "permission_denied") {
    return "Microphone access was denied. Allow Bottie in system privacy settings, then try again.";
  }
  if (status.errorCode === "device_unavailable") {
    return "No microphone is available. Connect or enable an input device, then try again.";
  }
  if (status.errorCode === "selected_device_unavailable") {
    return "The selected microphone is no longer available. Choose another microphone or System default, then try again.";
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

/** Describes only Rust-observed capture endpoints without implying physical audio latency. */
export function microphoneLatencyFeedback(status: MicrophoneStatus): string | null {
  if (status.phase === "idle" || status.phase === "error") return null;
  const input = status.latency.inputReadyMs === null ? "measuring" : formatNativeLatency(status.latency.inputReadyMs);
  const parts = [`Native timing`, `Input ready after Record: ${input}`];

  if (status.phase !== "starting") {
    const firstTranscript =
      status.latency.firstTranscriptMs === null
        ? status.transcriptionPhase === "ready"
          ? "none available"
          : "waiting"
        : formatNativeLatency(status.latency.firstTranscriptMs);
    parts.push(`First transcript after Record: ${firstTranscript}`);
  }
  if (status.phase === "captured") {
    const finalTranscript =
      status.latency.finalTranscriptMs === null
        ? status.transcriptionPhase === "error"
          ? "not available"
          : "processing"
        : formatNativeLatency(status.latency.finalTranscriptMs);
    parts.push(`Final transcript after capture stopped: ${finalTranscript}`);
  }
  return parts.join(" · ");
}
