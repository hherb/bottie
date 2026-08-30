/** Development-only fixture for Bottie's final local voice transcript presentation. */

import type { PageState } from "./page-state.svelte";

const VOICE_PREVIEW_VALUE = "final-transcript";

/** Reports whether a query explicitly requests the final-transcript fixture. */
export function voicePreviewRequested(search: string): boolean {
  return new URLSearchParams(search).get("voice") === VOICE_PREVIEW_VALUE;
}

/** Applies deterministic path-free voice turns to the disconnected browser preview only. */
export function applyVoicePreview(state: PageState, search: string): boolean {
  if (!voicePreviewRequested(search)) return false;
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
  };
  return true;
}
