import { readFile, readdir } from "node:fs/promises";

import { describe, expect, it } from "vitest";

describe("native microphone runtime contract", () => {
  it("keeps microphone capture native with no WebView media authority", async () => {
    const frontendRoot = new URL("../src/", import.meta.url);
    const frontendFiles = (await readdir(frontendRoot, { recursive: true })).filter(
      (file) => /\.(svelte|ts)$/.test(file) && !file.endsWith(".test.ts"),
    );
    const [frontendSources, native, voiceActivity, transcription, capability, cargoManifest, runtimeAssets] =
      await Promise.all([
        Promise.all(frontendFiles.map((file) => readFile(new URL(file, frontendRoot), "utf8"))),
        readFile(new URL("../src-tauri/src/microphone.rs", import.meta.url), "utf8"),
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
    expect(voiceActivity).toContain("MAX_VOICE_SEGMENTS");
    expect(voiceActivity).toContain(`pub(super) struct VoiceSegment {
    pub(super) activity: VoiceActivity,
    pub(super) start_ms: u64,
    pub(super) end_ms: u64,
}`);
    expect(voiceActivity).not.toMatch(/pub\(super\) (?:samples|device|path|backend)/i);
    expect(transcription).toContain("MAX_TRANSCRIPT_SEGMENTS");
    expect(transcription).toContain("MAX_TRANSCRIPT_TEXT_BYTES");
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

  it("declares one clear macOS microphone purpose without cloud speech authority", async () => {
    const infoPlist = await readFile(new URL("../src-tauri/Info.plist", import.meta.url), "utf8");

    expect(infoPlist).toContain("<key>NSMicrophoneUsageDescription</key>");
    expect(infoPlist).toContain("only after you choose Record voice");
    expect(infoPlist).toContain("keeps this capture on your device");
    expect(infoPlist).not.toMatch(/NSSpeechRecognitionUsageDescription|NSAudioCaptureUsageDescription/);
  });
});
