import { readFile, readdir } from "node:fs/promises";

import { describe, expect, it } from "vitest";

describe("native microphone runtime contract", () => {
  it("keeps microphone capture native with no WebView media authority", async () => {
    const frontendRoot = new URL("../src/", import.meta.url);
    const frontendFiles = (await readdir(frontendRoot, { recursive: true })).filter(
      (file) => /\.(svelte|ts)$/.test(file) && !file.endsWith(".test.ts"),
    );
    const [
      frontendSources,
      native,
      correction,
      voiceActivity,
      transcription,
      capability,
      cargoManifest,
      runtimeAssets,
    ] = await Promise.all([
      Promise.all(frontendFiles.map((file) => readFile(new URL(file, frontendRoot), "utf8"))),
      readFile(new URL("../src-tauri/src/microphone.rs", import.meta.url), "utf8"),
      readFile(new URL("../src-tauri/src/microphone/correction.rs", import.meta.url), "utf8"),
      readFile(new URL("../src-tauri/src/microphone/vad.rs", import.meta.url), "utf8"),
      readFile(new URL("../src-tauri/src/microphone/transcription.rs", import.meta.url), "utf8"),
      readFile(new URL("../src-tauri/capabilities/default.json", import.meta.url), "utf8"),
      readFile(new URL("../src-tauri/Cargo.toml", import.meta.url), "utf8"),
      readFile(new URL("../runtime-assets.json", import.meta.url), "utf8"),
    ]);
    const frontend = frontendSources.join("\n");

    expect(frontend).not.toMatch(/getUserMedia|MediaRecorder|AudioContext/);
    expect(native).toContain("MAX_CAPTURE_DURATION");
    expect(native).toContain("MAX_RETAINED_BYTES");
    expect(native).toContain("retained_byte_size");
    expect(native).toContain("voice_segments");
    expect(native).toContain("mod correction");
    expect(correction).toContain("MAX_TRANSCRIPT_TURN_BYTES");
    expect(correction).toContain("MAX_TRANSCRIPT_TEXT_BYTES");
    expect(correction).toContain("correct_transcript");
    expect(voiceActivity).toContain("MAX_VOICE_SEGMENTS");
    expect(voiceActivity).toContain(`pub(super) struct VoiceSegment {
    pub(super) activity: VoiceActivity,
    pub(super) start_ms: u64,
    pub(super) end_ms: u64,
}`);
    expect(voiceActivity).not.toMatch(/pub\(super\) (?:samples|device|path|backend)/i);
    expect(transcription).toContain("MAX_TRANSCRIPT_SEGMENTS");
    expect(transcription).toContain("MAX_TRANSCRIPT_TEXT_BYTES");
    expect(transcription).toContain("is_corrected");
    expect(transcription).toContain("WhisperContext");
    expect(transcription).toContain("verify_model_reader");
    expect(transcription).toContain("install_logging_hooks");
    expect(runtimeAssets).toContain('"whisperTinyQ5"');
    expect(runtimeAssets).toContain('"ggml-tiny-q5_1.bin"');
    expect(capability).not.toMatch(/microphone|media|audio/i);
    expect(cargoManifest).toContain('default-run = "bottie"');
    expect(cargoManifest).toContain('whisper-rs = "=0.16.0"');
  });

  it("installs the native Linux audio headers in packaging runners", async () => {
    const workflows = await Promise.all([
      readFile(new URL("../.github/workflows/linux-package-smoke.yml", import.meta.url), "utf8"),
      readFile(new URL("../.github/workflows/linux-distribution-validation.yml", import.meta.url), "utf8"),
    ]);

    for (const workflow of workflows) expect(workflow).toContain("libasound2-dev");
  });

  it("keeps local text-to-speech behind bounded Rust commands", async () => {
    const frontendRoot = new URL("../src/", import.meta.url);
    const frontendFiles = (await readdir(frontendRoot, { recursive: true })).filter(
      (file) => /\.(svelte|ts)$/.test(file) && !file.endsWith(".test.ts"),
    );
    const [frontendSources, native, capability, cargoManifest, tauriConfig, workflows] = await Promise.all([
      Promise.all(frontendFiles.map((file) => readFile(new URL(file, frontendRoot), "utf8"))),
      readFile(new URL("../src-tauri/src/speech.rs", import.meta.url), "utf8"),
      readFile(new URL("../src-tauri/capabilities/default.json", import.meta.url), "utf8"),
      readFile(new URL("../src-tauri/Cargo.toml", import.meta.url), "utf8"),
      readFile(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"),
      Promise.all([
        readFile(new URL("../.github/workflows/linux-package-smoke.yml", import.meta.url), "utf8"),
        readFile(new URL("../.github/workflows/linux-distribution-validation.yml", import.meta.url), "utf8"),
      ]),
    ]);

    expect(frontendSources.join("\n")).not.toMatch(/speechSynthesis|SpeechSynthesisUtterance|AudioContext/);
    expect(native).toContain("MAX_SPEECH_TEXT_BYTES");
    expect(native).toContain("MAX_SPEECH_VOICES");
    expect(native).toContain("struct SpeechVoice");
    expect(native).toContain("struct SpeechStatus");
    expect(native).toContain("tts::Tts");
    expect(capability).not.toMatch(/speech|audio/i);
    expect(cargoManifest).toContain('tts = "=0.26.3"');
    expect(tauriConfig).toContain('"libspeechd2"');
    expect(tauriConfig).toContain('"speech-dispatcher"');
    expect(tauriConfig).toContain('"speech-dispatcher-espeak-ng"');
    for (const workflow of workflows) expect(workflow).toContain("libspeechd-dev");
  });

  it("declares one clear macOS microphone purpose without cloud speech authority", async () => {
    const infoPlist = await readFile(new URL("../src-tauri/Info.plist", import.meta.url), "utf8");

    expect(infoPlist).toContain("<key>NSMicrophoneUsageDescription</key>");
    expect(infoPlist).toContain("only after you choose Record voice");
    expect(infoPlist).toContain("keeps this capture on your device");
    expect(infoPlist).not.toMatch(/NSSpeechRecognitionUsageDescription|NSAudioCaptureUsageDescription/);
  });
});
