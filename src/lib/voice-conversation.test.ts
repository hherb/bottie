import { describe, expect, it } from "vitest";

import type { MicrophoneStatus } from "$lib/microphone";
import {
  INITIAL_VOICE_CONVERSATION_STATUS,
  voiceCaptureActivityFeedback,
  voiceCommandErrorCode,
  voiceCommandErrorFeedback,
  voiceConversationFeedback,
  voiceMicrophoneGateFeedback,
  voiceTurnAction,
  voiceTurnTranscript,
  type VoiceConversationStatus,
  type VoiceTurnInput,
} from "./voice-conversation";

const IDLE_MICROPHONE: MicrophoneStatus = {
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

const CAPTURED_TRANSCRIPT: MicrophoneStatus = {
  ...IDLE_MICROPHONE,
  phase: "captured",
  permission: "granted",
  transcriptionPhase: "ready",
  transcriptSegments: [
    { text: "Draft the release note locally.", startMs: 0, endMs: 1_200, isFinal: true, isCorrected: false },
    { text: "Keep it short.", startMs: 1_300, endMs: 2_100, isFinal: true, isCorrected: false },
  ],
};

const PARTIAL_TRANSCRIPT: MicrophoneStatus = {
  ...CAPTURED_TRANSCRIPT,
  transcriptSegments: [{ text: "Draft the release", startMs: 0, endMs: 1_200, isFinal: false, isCorrected: false }],
};

function status(patch: Partial<VoiceConversationStatus> = {}): VoiceConversationStatus {
  return { ...INITIAL_VOICE_CONVERSATION_STATUS, active: true, phase: "listening", ...patch };
}

function turnInput(patch: Partial<VoiceTurnInput> = {}): VoiceTurnInput {
  return {
    active: true,
    phase: "listening",
    pendingReply: false,
    busy: false,
    capturing: false,
    microphone: IDLE_MICROPHONE,
    speechProgress: null,
    turnSubmitted: false,
    resumeRequested: false,
    ...patch,
  };
}

describe("voice conversation presentation", () => {
  it("starts from one inactive path-free status", () => {
    expect(INITIAL_VOICE_CONVERSATION_STATUS).toEqual({
      active: false,
      phase: "idle",
      pendingReply: false,
      speechProgress: null,
      endpointReason: null,
    });
  });

  it("names each conversation phase without native detail", () => {
    expect(voiceConversationFeedback(INITIAL_VOICE_CONVERSATION_STATUS)).toBe(
      "Voice conversation is off · push-to-talk and typed chat are unchanged",
    );
    expect(voiceConversationFeedback(status())).toBe("Listening");
    expect(voiceConversationFeedback(status({ phase: "finalizing" }))).toBe("Finishing the transcript");
    expect(voiceConversationFeedback(status({ phase: "thinking" }))).toBe("Thinking");
    expect(voiceConversationFeedback(status({ phase: "speaking" }))).toBe("Bottie is speaking");
  });

  it("reopens listening only after the reply stopped speaking", () => {
    expect(voiceTurnAction(turnInput({ phase: "finalizing" }))).toBe("resume");
    expect(voiceTurnAction(turnInput({ phase: "finalizing", speechProgress: "idle" }))).toBe("resume");
    expect(voiceTurnAction(turnInput({ phase: "finalizing", speechProgress: "buffering" }))).toBe("none");
    expect(voiceTurnAction(turnInput({ phase: "finalizing", speechProgress: "speaking" }))).toBe("none");
    expect(voiceTurnAction(turnInput({ phase: "finalizing", speechProgress: "failed" }))).toBe("resume");
  });

  it("explains the latched end-of-turn reason and falls back for unknown reasons", () => {
    expect(voiceConversationFeedback(status({ phase: "finalizing", endpointReason: "terminalPunctuation" }))).toBe(
      "Finishing the transcript · you finished the sentence",
    );
    expect(
      voiceConversationFeedback({
        ...status({ phase: "finalizing" }),
        endpointReason: "unknown_reason" as unknown as VoiceConversationStatus["endpointReason"],
      }),
    ).toBe("Finishing the transcript");
  });

  it("reports live capture only while the conversation is listening", () => {
    expect(voiceCaptureActivityFeedback(status({ phase: "listening" }), null)).toBe(
      "Microphone open · waiting for your voice",
    );
    expect(voiceCaptureActivityFeedback(status({ phase: "listening" }), "silence")).toBe(
      "Microphone open · waiting for your voice",
    );
    expect(voiceCaptureActivityFeedback(status({ phase: "listening" }), "speech")).toBe(
      "Microphone open · hearing you",
    );
    expect(voiceCaptureActivityFeedback(status({ phase: "speaking" }), "speech")).toBeNull();
    expect(voiceCaptureActivityFeedback({ ...INITIAL_VOICE_CONVERSATION_STATUS }, "speech")).toBeNull();
  });

  it("folds spoken-reply progress into the speaking vocabulary", () => {
    expect(voiceConversationFeedback(status({ phase: "thinking", speechProgress: "buffering" }))).toBe(
      "Thinking · preparing the spoken reply",
    );
    expect(voiceConversationFeedback(status({ phase: "speaking", speechProgress: "buffering" }))).toBe(
      "Bottie is speaking · preparing the next words",
    );
    expect(voiceConversationFeedback(status({ phase: "speaking", speechProgress: "interrupted" }))).toBe(
      "Bottie is speaking · the reply was interrupted",
    );
    expect(voiceConversationFeedback(status({ phase: "speaking", speechProgress: "failed" }))).toBe(
      "Bottie is speaking · the spoken reply failed",
    );
  });

  it("states why the microphone is not taking a turn while Bottie works", () => {
    expect(voiceMicrophoneGateFeedback(INITIAL_VOICE_CONVERSATION_STATUS)).toBeNull();
    expect(voiceMicrophoneGateFeedback(status())).toBeNull();
    expect(voiceMicrophoneGateFeedback(status({ phase: "thinking" }))).toBe("Microphone closed while Bottie thinks.");
    expect(voiceMicrophoneGateFeedback(status({ phase: "speaking" }))).toBe(
      "Microphone muted while Bottie speaks · Interrupt takes the turn back.",
    );
    expect(voiceMicrophoneGateFeedback(status({ phase: "finalizing" }))).toBe(
      "Microphone closed while the turn finishes.",
    );
  });
});

describe("voice conversation errors", () => {
  it("normalizes rejected commands into bounded codes", () => {
    expect(voiceCommandErrorCode("not_active")).toBe("not_active");
    expect(voiceCommandErrorCode("capture_active")).toBe("capture_active");
    expect(voiceCommandErrorCode("speech_unavailable")).toBe("speech_unavailable");
    expect(voiceCommandErrorCode("speech_playback_failed")).toBe("speech_playback_failed");
    expect(voiceCommandErrorCode("unavailable")).toBe("unavailable");
    expect(voiceCommandErrorCode({ code: "capture_active" })).toBe("capture_active");
    expect(voiceCommandErrorCode(new Error("not_active"))).toBe("not_active");
    expect(voiceCommandErrorCode("some native detail with a path /tmp/engine")).toBe("command_failed");
    expect(voiceCommandErrorCode(undefined)).toBe("command_failed");
  });

  it("never echoes native detail in feedback", () => {
    expect(voiceCommandErrorFeedback("not_active")).toBe("The voice conversation is no longer active.");
    expect(voiceCommandErrorFeedback("capture_active")).toBe("Bottie is still finishing the transcript.");
    expect(voiceCommandErrorFeedback("speech_unavailable")).toBe(
      "Local speech is unavailable, so the spoken reply cannot start.",
    );
    expect(voiceCommandErrorFeedback("speech_playback_failed")).toBe("Local speech failed during the spoken reply.");
    expect(voiceCommandErrorFeedback("unavailable")).toBe("Voice conversation needs the native desktop app.");
    expect(voiceCommandErrorFeedback("command_failed")).toBe("The voice conversation request failed. Try again.");
  });
});

describe("voice turn decision", () => {
  it("returns the ordered final transcript only when the capture is ready", () => {
    expect(voiceTurnTranscript(IDLE_MICROPHONE)).toBeNull();
    expect(voiceTurnTranscript(PARTIAL_TRANSCRIPT)).toBeNull();
    expect(voiceTurnTranscript(CAPTURED_TRANSCRIPT)).toBe("Draft the release note locally.\nKeep it short.");
  });

  it("submits one closed capture with a ready transcript", () => {
    expect(voiceTurnAction(turnInput({ microphone: CAPTURED_TRANSCRIPT }))).toBe("submit");
  });

  it("never submits while inactive, busy, or still capturing", () => {
    expect(voiceTurnAction(turnInput({ active: false, microphone: CAPTURED_TRANSCRIPT }))).toBe("none");
    expect(voiceTurnAction(turnInput({ busy: true, microphone: CAPTURED_TRANSCRIPT }))).toBe("none");
    expect(voiceTurnAction(turnInput({ capturing: true, microphone: CAPTURED_TRANSCRIPT }))).toBe("none");
  });

  it("never submits the same turn twice", () => {
    expect(
      voiceTurnAction(turnInput({ microphone: CAPTURED_TRANSCRIPT, turnSubmitted: true, phase: "speaking" })),
    ).toBe("none");
  });

  it("resumes only after a drained reply that no longer pends", () => {
    expect(voiceTurnAction(turnInput({ phase: "finalizing", turnSubmitted: true }))).toBe("resume");
    expect(voiceTurnAction(turnInput({ phase: "finalizing", pendingReply: true, turnSubmitted: true }))).toBe("none");
    // A turn that ended with nothing to send (no speech) also has to reopen listening.
    expect(voiceTurnAction(turnInput({ phase: "finalizing" }))).toBe("resume");
  });

  it("recovers a session that lost its reply before adoption, once", () => {
    expect(voiceTurnAction(turnInput({ phase: "listening", turnSubmitted: true }))).toBe("resume");
    expect(voiceTurnAction(turnInput({ phase: "thinking", turnSubmitted: true }))).toBe("resume");
    expect(voiceTurnAction(turnInput({ phase: "listening", turnSubmitted: true, resumeRequested: true }))).toBe("none");
    // A reopened capture clears the marker, so the next turn can submit normally.
    expect(voiceTurnAction(turnInput({ capturing: true, phase: "listening" }))).toBe("none");
  });

  it("waits while the session is healthy and no turn is pending", () => {
    expect(voiceTurnAction(turnInput({ capturing: true }))).toBe("none");
    expect(voiceTurnAction(turnInput())).toBe("none");
    expect(voiceTurnAction(turnInput({ phase: "speaking", turnSubmitted: true }))).toBe("none");
  });
});
