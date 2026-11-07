/** Typed path-free WebView contract for Rust-owned hands-free voice conversation mode. */

import { invoke } from "@tauri-apps/api/core";

import { buildTranscriptComposerDraft, type MicrophoneStatus } from "$lib/microphone";

/** User-visible phase of the one native hands-free conversation session. */
export type VoicePhase = "idle" | "listening" | "finalizing" | "thinking" | "speaking";

/** Path-free progress of the streamed spoken reply. */
export type VoiceSpeechProgress = "idle" | "buffering" | "speaking" | "interrupted" | "failed";

/** Stable native reason the turn ended without the user pressing anything. */
export type VoiceEndpointReason =
  "stableTranscript" | "terminalPunctuation" | "silenceTimeout" | "noSpeech" | "hardCap";

/** Bounded conversation state returned by Rust; it never carries audio, paths, or engine detail. */
export type VoiceConversationStatus = {
  active: boolean;
  phase: VoicePhase;
  pendingReply: boolean;
  speechProgress: VoiceSpeechProgress | null;
  endpointReason: VoiceEndpointReason | null;
};

/** Stable conversation command failures that are safe to show in the WebView. */
export type VoiceCommandErrorCode =
  | "not_active"
  | "capture_active"
  | "speech_active"
  | "microphone_failed"
  | "speech_unavailable"
  | "speech_playback_failed"
  | "command_failed";

/** State before any conversation command has been issued. */
export const INITIAL_VOICE_CONVERSATION_STATUS: VoiceConversationStatus = {
  active: false,
  phase: "idle",
  pendingReply: false,
  speechProgress: null,
  endpointReason: null,
};

/** Starts the one hands-free conversation session over the selected input. */
export async function startVoiceConversation(): Promise<VoiceConversationStatus> {
  return invoke<VoiceConversationStatus>("start_voice_conversation");
}

/** Re-opens conversation capture after a spoken reply drained. */
export async function resumeVoiceListening(): Promise<VoiceConversationStatus> {
  return invoke<VoiceConversationStatus>("resume_voice_listening");
}

/** Marks the reply about to be submitted as spoken by the active conversation session. */
export async function armVoiceReply(): Promise<VoiceConversationStatus> {
  return invoke<VoiceConversationStatus>("arm_voice_reply");
}

/** Stops playback and generation, then returns to listening. */
export async function interruptVoiceTurn(): Promise<VoiceConversationStatus> {
  return invoke<VoiceConversationStatus>("interrupt_voice_turn");
}

/** Ends the conversation session, stops playback, and cancels the active generation. */
export async function endVoiceConversation(): Promise<VoiceConversationStatus> {
  return invoke<VoiceConversationStatus>("end_voice_conversation");
}

/** Reads the current conversation phase without observing or changing audio state. */
export async function getVoiceConversationStatus(): Promise<VoiceConversationStatus> {
  return invoke<VoiceConversationStatus>("get_voice_conversation_status");
}

/** Bounded phrases for the native last-latched end-of-turn reasons. */
const ENDPOINT_PHRASES: Record<VoiceEndpointReason, string> = {
  stableTranscript: "the transcript settled",
  terminalPunctuation: "you finished the sentence",
  silenceTimeout: "the pause was long enough",
  noSpeech: "nothing was heard",
  hardCap: "the turn reached its limit",
};

/** Describes the conversation phase in one calm path-free line. */
export function voiceConversationFeedback(status: VoiceConversationStatus): string {
  if (!status.active) return "Voice conversation is off · push-to-talk and typed chat are unchanged";
  if (status.phase === "listening") {
    // The endpoint reason is latched only once this capture ended on its own; a reopened capture clears it.
    return status.endpointReason === null ? "Listening" : `Turn ended · ${ENDPOINT_PHRASES[status.endpointReason]}`;
  }
  if (status.phase === "finalizing") {
    return status.speechProgress === "failed" ? "Finishing the reply · the spoken reply failed" : "Finishing the reply";
  }
  if (status.phase === "thinking") {
    return status.speechProgress === "buffering" ? "Thinking · preparing the spoken reply" : "Thinking";
  }
  if (status.speechProgress === "interrupted") return "Bottie is speaking · the reply was interrupted";
  if (status.speechProgress === "failed") return "Bottie is speaking · the spoken reply failed";
  if (status.speechProgress === "buffering") return "Bottie is speaking · preparing the next words";
  return "Bottie is speaking";
}

/**
 * Describes what the open conversation microphone is hearing right now.
 *
 * Returns null unless the session is actively listening, so the page never claims live capture during thinking or
 * speaking. This is presentation only; capture and endpointing stay native.
 */
export function voiceCaptureActivityFeedback(
  status: VoiceConversationStatus,
  activity: "speech" | "silence" | null,
): string | null {
  if (!status.active || status.phase !== "listening") return null;
  return activity === "speech" ? "Microphone open · hearing you" : "Microphone open · waiting for your voice";
}

/** Reports why the microphone is not taking a new turn, or null when it is open. */
export function voiceMicrophoneGateFeedback(status: VoiceConversationStatus): string | null {
  if (!status.active) return null;
  if (status.phase === "finalizing") return "Microphone closed while the reply finishes.";
  if (status.phase === "thinking") return "Microphone closed while Bottie thinks.";
  if (status.phase === "speaking") return "Microphone muted while Bottie speaks · Interrupt takes the turn back.";
  return null;
}

/** Normalizes one rejected native voice command into a bounded error code. */
export function voiceCommandErrorCode(error: unknown): VoiceCommandErrorCode {
  const raw =
    typeof error === "string"
      ? error
      : error instanceof Error
        ? error.message
        : typeof error === "object" && error !== null && "code" in error && typeof error.code === "string"
          ? error.code
          : "";
  switch (raw.trim()) {
    case "not_active":
      return "not_active";
    case "capture_active":
      return "capture_active";
    case "speech_active":
      return "speech_active";
    case "microphone_failed":
      return "microphone_failed";
    case "speech_unavailable":
      return "speech_unavailable";
    case "speech_playback_failed":
      return "speech_playback_failed";
    default:
      return "command_failed";
  }
}

/** Builds one bounded recovery sentence for a rejected conversation command. */
export function voiceCommandErrorFeedback(code: VoiceCommandErrorCode): string {
  if (code === "not_active") return "The voice conversation is no longer active.";
  if (code === "capture_active") return "Bottie is still finishing the transcript.";
  if (code === "speech_active") return "Bottie is still speaking the previous reply.";
  if (code === "microphone_failed") return "The microphone could not start listening for the next turn.";
  if (code === "speech_unavailable") return "Local speech is unavailable, so the spoken reply cannot start.";
  if (code === "speech_playback_failed") return "Local speech failed during the spoken reply.";
  return "The voice conversation request failed. Try again.";
}

/** One decision the page takes from the latest conversation and capture state. */
export type VoiceTurnAction = "none" | "submit" | "resume" | "end";

/** Inputs for the pure conversation turn decision. */
export type VoiceTurnInput = {
  active: boolean;
  phase: VoicePhase;
  pendingReply: boolean;
  /** A generation, a submission, or a durable write is already in flight. */
  busy: boolean;
  /** Native input is starting, recording, preparing a model, or transcribing. */
  capturing: boolean;
  microphone: MicrophoneStatus;
  /** Progress of the reply's speech, including a finished reply whose last chunks are still playing. */
  speechProgress: VoiceSpeechProgress | null;
  /** The current turn's transcript was already handed to the ordinary send path. */
  turnSubmitted: boolean;
  /** A recovery resume was already issued and native input has not reopened since. */
  resumeRequested: boolean;
};

/** Returns the ordered final transcript of the current capture, or null when none is ready. */
export function voiceTurnTranscript(status: MicrophoneStatus): string | null {
  const draft = buildTranscriptComposerDraft("", status);
  return draft.ok ? draft.draft : null;
}

/**
 * Decides the single conversation action for the current state.
 *
 * Submission needs a closed capture with a ready final transcript, because the native reply can only be armed
 * after the turn's capture ends. Resuming needs a drained reply, a turn that closed without a usable transcript, or
 * a session that lost its turn before a reply was dispatched, and never runs while native input or a generation is
 * still busy. A listening session whose capture failed ends, because reopening a failed device would only fail again.
 */
export function voiceTurnAction(input: VoiceTurnInput): VoiceTurnAction {
  if (!input.active) return "none";
  if (input.busy || input.capturing) return "none";
  if (input.phase === "listening" && !input.turnSubmitted && input.microphone.phase === "error") return "end";
  if (!input.turnSubmitted && voiceTurnTranscript(input.microphone) !== null) return "submit";
  // Never reopen capture while the reply is still speaking: that would cut the last chunks off and let Bottie speak
  // into its own microphone. The reply drains on its own and the next poll re-evaluates.
  if (!speechSettled(input.speechProgress)) return "none";
  // Nothing heard, an empty recognition, or a failed one: the capture closed on its own and nothing will reopen it.
  if (input.phase === "listening" && !input.turnSubmitted && !input.resumeRequested && captureSettled(input.microphone)) {
    return "resume";
  }
  if (input.phase === "finalizing" && !input.pendingReply) return "resume";
  // A session can lose its reply before adoption, which leaves it listening with a consumed turn. Recover that
  // once, because the reopened capture is what clears both markers for the next turn.
  if (input.turnSubmitted && !input.resumeRequested && (input.phase === "listening" || input.phase === "thinking")) {
    return "resume";
  }
  return "none";
}

/** Whether the capture closed and its final recognition finished, successfully or not. */
function captureSettled(status: MicrophoneStatus): boolean {
  return status.phase === "captured" && (status.transcriptionPhase === "ready" || status.transcriptionPhase === "error");
}

/** Whether playback finished (or never started) so capture may reopen. */
function speechSettled(progress: VoiceSpeechProgress | null): boolean {
  return progress === null || progress === "idle" || progress === "interrupted" || progress === "failed";
}
