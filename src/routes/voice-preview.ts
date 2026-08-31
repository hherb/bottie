/** Development-only fixture for Bottie's final local voice transcript presentation. */

import type { PageState } from "./page-state.svelte";

const TRANSCRIPT_PREVIEW_VALUE = "final-transcript";
const PLAYBACK_PREVIEW_VALUE = "local-playback";
const AUDIO_CONTENT_PREVIEW_VALUE = "audio-content";
const INPUT_DEVICES_PREVIEW_VALUE = "input-devices";

/** Reports whether a query explicitly requests the final-transcript fixture. */
export function voicePreviewRequested(search: string): boolean {
  const value = new URLSearchParams(search).get("voice");
  return (
    value === TRANSCRIPT_PREVIEW_VALUE ||
    value === PLAYBACK_PREVIEW_VALUE ||
    value === AUDIO_CONTENT_PREVIEW_VALUE ||
    value === INPUT_DEVICES_PREVIEW_VALUE
  );
}

/** Applies deterministic path-free voice turns to the disconnected browser preview only. */
export function applyVoicePreview(state: PageState, search: string): boolean {
  if (!voicePreviewRequested(search)) return false;
  const value = new URLSearchParams(search).get("voice");
  if (value === INPUT_DEVICES_PREVIEW_VALUE) {
    state.microphone.deviceList = {
      devices: [
        { token: "system-default", label: "System default", isSystemDefault: true },
        { token: "local-input-preview-1", label: "MacBook microphone", isSystemDefault: false },
        { token: "local-input-preview-2", label: "Studio USB microphone", isSystemDefault: false },
      ],
      selectedToken: "local-input-preview-2",
      selectionAvailable: true,
    };
    state.microphone.devicesLoaded = true;
    return true;
  }
  if (value === PLAYBACK_PREVIEW_VALUE) {
    state.speech.applyPreview(
      [
        { id: "voice.en-au", name: "Karen", language: "en-AU" },
        { id: "voice.en-gb", name: "Daniel", language: "en-GB" },
        { id: "voice.fr-fr", name: "Thomas", language: "fr-FR" },
      ],
      {
        phase: "speaking",
        selectedVoiceId: "voice.en-au",
        errorCode: null,
        latency: { playbackAcceptedMs: 14 },
      },
    );
    return true;
  }
  if (value === AUDIO_CONTENT_PREVIEW_VALUE) {
    state.models = [
      {
        providerId: "omlx",
        providerName: "oMLX",
        modelId: "fixture-audio-model",
        displayName: "Fixture audio model",
        maxContextTokens: 32_768,
        loadState: "loaded",
        capabilities: { text: true, streaming: true, tools: false, vision: false, audio: true, embeddings: false },
      },
    ];
    state.selectedProviderId = "omlx";
    state.selectedModelKey = "omlx:fixture-audio-model";
    state.providerStatus = "available";
    state.prompt = "Summarize this recording";
  }
  state.microphone.status = {
    phase: "captured",
    permission: "granted",
    durationMs: 12_800,
    maxDurationMs: 60_000,
    sampleRateHz: 48_000,
    channels: 1,
    retainedByteSize: 2_457_600,
    inputLevel: 0,
    voiceActivity: "silence",
    voiceSegments: [
      { activity: "speech", startMs: 420, endMs: 3_200 },
      { activity: "silence", startMs: 3_200, endMs: 4_100 },
      { activity: "speech", startMs: 4_100, endMs: 8_900 },
      { activity: "silence", startMs: 8_900, endMs: 9_700 },
      { activity: "speech", startMs: 9_700, endMs: 12_300 },
    ],
    transcriptionPhase: "ready",
    transcriptSegments: [
      { text: "Draft the release note locally.", startMs: 420, endMs: 3_200, isFinal: true, isCorrected: false },
      {
        text: "Keep provider delivery and persistence out of this slice.",
        startMs: 4_100,
        endMs: 8_900,
        isFinal: true,
        isCorrected: true,
      },
      { text: "Then review the compact layout.", startMs: 9_700, endMs: 12_300, isFinal: true, isCorrected: false },
    ],
    transcriptionErrorCode: null,
    errorCode: null,
    latency: { inputReadyMs: 18, firstTranscriptMs: 1_725, finalTranscriptMs: 245 },
  };
  return true;
}
