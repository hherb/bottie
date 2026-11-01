import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import MicrophoneControl from "./MicrophoneControl.svelte";
import { INITIAL_MICROPHONE_DEVICE_LIST, type MicrophoneInputDeviceList, type MicrophoneStatus } from "./microphone";
import { INITIAL_VOICE_CONVERSATION_STATUS, type VoiceConversationStatus } from "./voice-conversation";

const IDLE_STATUS: MicrophoneStatus = {
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

/** Inert action props shared by every microphone-control render. */
function inertProps() {
  return {
    onstart: vi.fn(),
    onstop: vi.fn(),
    ondiscard: vi.fn(),
    oncorrect: vi.fn(),
    ontogglesendaudio: vi.fn(),
    ontoggleretainaudio: vi.fn(),
    onloaddevices: vi.fn(),
    onselectdevice: vi.fn(),
    onusetext: vi.fn(),
    onstartconversation: vi.fn(),
    onendconversation: vi.fn(),
    oninterruptconversation: vi.fn(),
  };
}

/** Renders the microphone control with inert actions and one path-free native status. */
function rendered(
  status: MicrophoneStatus,
  disabled = false,
  willInterrupt = false,
  audioAvailable = false,
  sendAudio = false,
  retainAudio = false,
  deviceList: MicrophoneInputDeviceList = INITIAL_MICROPHONE_DEVICE_LIST,
  devicesLoaded = false,
  deviceListFailed = false,
  transcriptDraftFeedback = "",
  transcriptDraftError = false,
): string {
  return render(MicrophoneControl, {
    props: {
      ...inertProps(),
      status,
      disabled,
      willInterrupt,
      audioAvailable,
      audioUnavailableReason: audioAvailable ? "" : "Choose an audio-capable model to send this recording.",
      sendAudio,
      retainAudio,
      deviceList,
      devicesLoaded,
      deviceListFailed,
      transcriptDraftFeedback,
      transcriptDraftError,
    },
  }).body;
}

/** Renders the same control with one path-free conversation status. */
function renderedConversation(
  status: MicrophoneStatus,
  voiceStatus: VoiceConversationStatus,
  voiceError: string | null = null,
  voiceFeedback = "",
): string {
  return render(MicrophoneControl, {
    props: {
      ...inertProps(),
      status,
      disabled: false,
      willInterrupt: false,
      audioAvailable: false,
      audioUnavailableReason: "Choose an audio-capable model to send this recording.",
      sendAudio: false,
      retainAudio: false,
      deviceList: INITIAL_MICROPHONE_DEVICE_LIST,
      devicesLoaded: false,
      deviceListFailed: false,
      transcriptDraftFeedback: "",
      transcriptDraftError: false,
      voiceStatus,
      voiceError,
      voiceFeedback,
    },
  }).body;
}

describe("MicrophoneControl", () => {
  it("requests microphone access only behind an explicit labelled action", () => {
    const html = rendered(IDLE_STATUS);

    expect(html).toContain('aria-label="Record voice locally"');
    expect(html).toContain("Record voice");
    expect(html).toContain("requested only when you choose Record voice");
    expect(html).not.toContain("getUserMedia");
    expect(html).toContain('aria-label="Choose microphone input"');
    expect(html).toContain("Choose microphone");
    expect(html).not.toContain('aria-label="Microphone input"');
  });

  it("shows one keyboard-operable remembered microphone choice after discovery", () => {
    const html = rendered(
      IDLE_STATUS,
      false,
      false,
      false,
      false,
      false,
      {
        devices: [
          { token: "system-default", label: "System default", isSystemDefault: true },
          { token: "local-input-001", label: "Desk microphone", isSystemDefault: false },
        ],
        selectedToken: "local-input-001",
        selectionAvailable: true,
      },
      true,
    );

    expect(html).toContain('aria-label="Microphone input"');
    expect(html).toContain('<option value="system-default">System default</option>');
    expect(html).toMatch(/<option value="local-input-001" selected(?:="")?>Desk microphone<\/option>/);
    expect(html).toContain("1 microphone available · last available choice is remembered on this device");
    expect(html).not.toMatch(/device id|host api|hardware address/i);
  });

  it("keeps System default visible and explains empty or stale discovery", () => {
    const empty = rendered(IDLE_STATUS, false, false, false, false, false, INITIAL_MICROPHONE_DEVICE_LIST, true);
    const stale = rendered(
      IDLE_STATUS,
      false,
      false,
      false,
      false,
      false,
      {
        devices: [{ token: "system-default", label: "System default", isSystemDefault: true }],
        selectedToken: "local-input-004",
        selectionAvailable: false,
      },
      true,
    );

    expect(empty).toContain("No microphones are currently available");
    expect(empty).toContain("System default will be checked when you Record");
    expect(stale).toContain('role="alert"');
    expect(stale).toContain("Selected microphone is no longer available");
    expect(stale).toContain("Choose another microphone before recording");
  });

  it("reports path-free discovery failure without opening capture", () => {
    const html = rendered(IDLE_STATUS, false, false, false, false, false, INITIAL_MICROPHONE_DEVICE_LIST, false, true);

    expect(html).toContain('role="alert"');
    expect(html).toContain("Microphone choices could not be refreshed");
    expect(html).toContain("Your current session selection is unchanged");
  });

  it("labels Record as an explicit interruption when Bottie is producing output", () => {
    const html = rendered(IDLE_STATUS, false, true);

    expect(html).toContain('aria-label="Interrupt Bottie and record voice locally"');
    expect(html).toContain("Interrupt &amp; record");
    expect(html).not.toMatch(/aria-label="Interrupt Bottie and record voice locally"[^>]*disabled/);
  });

  it("shows a clear stop action and bounded local activity while recording", () => {
    const html = rendered({
      ...IDLE_STATUS,
      phase: "recording",
      permission: "granted",
      durationMs: 3_420,
      sampleRateHz: 48_000,
      channels: 1,
      inputLevel: 0.42,
      voiceActivity: "speech",
      voiceSegments: [{ activity: "speech", startMs: 0, endMs: 3_420 }],
    });

    expect(html).toContain('aria-label="Stop voice capture"');
    expect(html).toContain("Recording locally");
    expect(html).toContain('role="progressbar"');
    expect(html).toContain('aria-valuenow="42"');
    expect(html).toContain("Speech detected");
  });

  it("keeps a completed capture visibly session-only and discardable", () => {
    const html = rendered({
      ...IDLE_STATUS,
      phase: "captured",
      permission: "granted",
      durationMs: 3_420,
      retainedByteSize: 656_640,
      voiceActivity: "silence",
      voiceSegments: [
        { activity: "silence", startMs: 0, endMs: 420 },
        { activity: "speech", startMs: 420, endMs: 2_920 },
        { activity: "silence", startMs: 2_920, endMs: 3_420 },
      ],
    });

    expect(html).toContain("held only in native memory");
    expect(html).toContain("0:02 speech");
    expect(html).toContain('aria-label="Discard voice capture"');
    expect(html).toContain("Record again");
    expect(html).not.toContain("Send voice");
  });

  it("requires explicit provider delivery and separate local retention choices", () => {
    const captured = {
      ...IDLE_STATUS,
      phase: "captured" as const,
      permission: "granted" as const,
      durationMs: 1_250,
      retainedByteSize: 80_000,
    };

    const initial = rendered(captured, false, false, true);
    expect(initial).toContain('aria-label="Send recording with the next message"');
    expect(initial).toContain('aria-pressed="false"');
    expect(initial).toContain('aria-label="Retain recording locally with the message"');
    expect(initial).not.toMatch(/Retain recording locally with the message[^>]*disabled/);

    const unavailable = rendered(captured);
    expect(unavailable).toMatch(/Send recording with the next message[^>]*disabled/);
    expect(unavailable).not.toMatch(/Retain recording locally with the message[^>]*disabled/);
    expect(unavailable).toContain("You can still retain it locally.");

    const enabled = rendered(captured, false, false, true, true, true);
    expect(enabled).toContain('aria-label="Stop sending recording with the next message"');
    expect(enabled).toContain('aria-label="Do not retain recording locally with the message"');
    expect(enabled).toContain("Recording will be sent and retained as a local WAV attachment.");

    const routeChanged = rendered(captured, false, false, false, true);
    expect(routeChanged).not.toMatch(/Stop sending recording with the next message[^>]*disabled/);
    expect(routeChanged).toContain(
      "Choose an audio-capable model to send this recording. Turn off Send recording or choose an audio-capable model.",
    );
  });

  it("renders path-free partial and final transcript turns with visible timing", () => {
    const html = rendered({
      ...IDLE_STATUS,
      phase: "captured",
      permission: "granted",
      durationMs: 3_420,
      transcriptionPhase: "ready",
      transcriptSegments: [
        { text: "Hello there", startMs: 420, endMs: 1_800, isFinal: true, isCorrected: false },
        { text: "General Kenobi", startMs: 1_900, endMs: 3_100, isFinal: true, isCorrected: true },
      ],
    });

    expect(html).toContain('aria-label="Local voice transcript"');
    expect(html).toContain("Hello there");
    expect(html).toContain("General Kenobi");
    expect(html).toContain("0:00–0:01");
    expect(html).toContain("0:01–0:03");
    expect(html).toContain("Turn 1");
    expect(html).toContain("Turn 2");
    expect(html).toContain('aria-label="Correct voice turn 1"');
    expect(html).toContain('aria-label="Save correction for voice turn 1"');
    expect(html).toContain('aria-label="Use final transcript as an editable text draft"');
    expect(html).toContain("Use transcript as text");
    expect(html).toContain("Corrected");
    expect(html).not.toMatch(/model path|sample|hash/i);
  });

  it("keeps partial turns visible but non-editable", () => {
    const html = rendered({
      ...IDLE_STATUS,
      phase: "recording",
      permission: "granted",
      transcriptionPhase: "transcribing",
      transcriptSegments: [{ text: "Still listening", startMs: 420, endMs: 1_800, isFinal: false, isCorrected: false }],
    });

    expect(html).toContain("Turn 1");
    expect(html).toContain("Still listening");
    expect(html).toContain("Partial");
    expect(html).not.toContain("Correct voice turn 1");
    expect(html).not.toContain("Use transcript as text");
  });

  it("does not offer transcript transfer for empty, failed, or stale transcript state", () => {
    const empty = rendered({ ...IDLE_STATUS, phase: "captured", transcriptionPhase: "ready" });
    const failed = rendered({
      ...IDLE_STATUS,
      phase: "captured",
      transcriptionPhase: "error",
      transcriptSegments: [{ text: "Old result", startMs: 0, endMs: 800, isFinal: true, isCorrected: false }],
    });
    const stale = rendered({
      ...IDLE_STATUS,
      phase: "recording",
      transcriptionPhase: "ready",
      transcriptSegments: [{ text: "Old result", startMs: 0, endMs: 800, isFinal: true, isCorrected: false }],
    });

    expect(empty).not.toContain("Use transcript as text");
    expect(failed).not.toContain("Use transcript as text");
    expect(stale).not.toContain("Use transcript as text");
  });

  it("announces a transcript draft boundary failure without implying submission", () => {
    const html = rendered(
      IDLE_STATUS,
      false,
      false,
      false,
      false,
      false,
      INITIAL_MICROPHONE_DEVICE_LIST,
      false,
      false,
      "The transcript was not added because the combined draft exceeds the 32 KiB text limit.",
      true,
    );

    expect(html).toContain('class="transcript-draft-feedback error"');
    expect(html).toContain('role="alert"');
    expect(html).toContain("The transcript was not added");
    expect(html).not.toMatch(/sent|submitted/i);
  });

  it("keeps capture actions bounded while the native recognizer is preparing", () => {
    const html = rendered({
      ...IDLE_STATUS,
      phase: "captured",
      permission: "granted",
      transcriptionPhase: "preparing_model",
    });

    expect(html).toContain("Preparing the local speech model");
    expect(html).toMatch(/aria-label="Record voice locally"[^>]*disabled/);
    expect(html).toContain('aria-label="Discard voice capture"');
  });

  it("renders a restrained accessible native timing summary", () => {
    const html = rendered({
      ...IDLE_STATUS,
      phase: "captured",
      permission: "granted",
      transcriptionPhase: "ready",
      latency: { inputReadyMs: 18, firstTranscriptMs: 1_725, finalTranscriptMs: 245 },
    });

    expect(html).toContain('aria-label="Local voice timing"');
    expect(html).toContain("Input ready after Record: 18 ms");
    expect(html).toContain("First transcript after Record: 1.73 s");
    expect(html).toContain("Final transcript after capture stopped: 245 ms");
    expect(html).not.toMatch(/first sample|first token|audible/i);
  });
});

describe("MicrophoneControl conversation mode", () => {
  const ACTIVE_LISTENING: VoiceConversationStatus = {
    ...INITIAL_VOICE_CONVERSATION_STATUS,
    active: true,
    phase: "listening",
  };
  const RECORDING: MicrophoneStatus = { ...IDLE_STATUS, phase: "recording", permission: "granted" };

  it("offers hands-free conversation without removing push-to-talk", () => {
    const html = rendered(IDLE_STATUS);

    expect(html).toContain('aria-label="Start a hands-free voice conversation"');
    expect(html).toContain("Start conversation");
    expect(html).toContain('aria-label="Record voice locally"');
  });

  it("replaces push-to-talk controls with conversation controls while active", () => {
    const html = renderedConversation(RECORDING, ACTIVE_LISTENING);

    expect(html).toContain('aria-label="End voice conversation"');
    expect(html).toContain("End conversation");
    expect(html).toContain('aria-label="Interrupt Bottie\'s spoken reply"');
    expect(html).toContain("Listening");
    expect(html).not.toContain('aria-label="Stop voice capture"');
    expect(html).not.toContain('aria-label="Record voice locally"');
    expect(html).toContain('aria-label="Microphone input level"');
  });

  it("shows that the microphone is muted while Bottie speaks", () => {
    const html = renderedConversation(
      { ...IDLE_STATUS, phase: "captured", permission: "granted" },
      { ...ACTIVE_LISTENING, phase: "speaking", speechProgress: "speaking" },
    );

    expect(html).toContain("Bottie is speaking");
    expect(html).toContain("Microphone muted while Bottie speaks");
    expect(html).toContain('aria-label="Interrupt Bottie\'s spoken reply"');
    expect(html).not.toMatch(/aria-label="Interrupt Bottie's spoken reply"[^>]*disabled/);
  });

  it("disables Interrupt while only listening", () => {
    const html = renderedConversation(RECORDING, ACTIVE_LISTENING);

    expect(html).toMatch(/aria-label="Interrupt Bottie's spoken reply"[^>]*disabled/);
  });

  it("renders bounded conversation errors and notes without native detail", () => {
    const html = renderedConversation(
      { ...IDLE_STATUS, phase: "captured", permission: "granted" },
      { ...ACTIVE_LISTENING, phase: "thinking" },
      "Local speech is unavailable, so the spoken reply cannot start.",
      "Finish or clear your draft first · the voice turn was copied into it.",
    );

    expect(html).toContain("Thinking");
    expect(html).toContain("Microphone closed while Bottie thinks");
    expect(html).toContain('class="voice-conversation-error error"');
    expect(html).toContain("Local speech is unavailable");
    expect(html).toContain("the voice turn was copied into it");
    expect(html).not.toContain("/tmp/");
  });
});
