import { describe, expect, it } from "vitest";

import {
  GEMMA_TERMS_ACKNOWLEDGEMENT,
  buildRuntimeAssetManifest,
  buildTermsAcceptanceEvidence,
  parseTermsAcceptanceArgument,
} from "./release-assets.mjs";

const SHA = "a".repeat(64);
const ASSET_HASHES = {
  onnxRuntimeLicenceSha256: SHA,
  onnxRuntimeNoticesSha256: SHA,
  whisperModelLicenceSha256: SHA,
};

describe("release runtime assets", () => {
  it("pins every runtime-downloaded EmbeddingGemma file to one immutable revision", () => {
    const manifest = buildRuntimeAssetManifest(ASSET_HASHES);

    expect(manifest.embeddingGemma.repository).toBe("onnx-community/embeddinggemma-300m-ONNX");
    expect(manifest.embeddingGemma.revision).toMatch(/^[a-f0-9]{40}$/);
    expect(manifest.embeddingGemma.files.map((file) => file.path)).toEqual([
      "config.json",
      "onnx/model_q4.onnx",
      "onnx/model_q4.onnx_data",
      "special_tokens_map.json",
      "tokenizer.json",
      "tokenizer_config.json",
    ]);
    expect(manifest.embeddingGemma.files.every((file) => file.sha256.match(/^[a-f0-9]{64}$/))).toBe(true);
  });

  it("pins the multilingual Whisper tiny Q5 model and its reviewed MIT licence", () => {
    const manifest = buildRuntimeAssetManifest(ASSET_HASHES);

    expect(manifest.whisperTinyQ5.repository).toBe("ggerganov/whisper.cpp");
    expect(manifest.whisperTinyQ5.revision).toMatch(/^[a-f0-9]{40}$/);
    expect(manifest.whisperTinyQ5.variant).toBe("tiny-q5_1-multilingual");
    expect(manifest.whisperTinyQ5.file).toEqual({
      path: "ggml-tiny-q5_1.bin",
      sha256: "818710568da3ca15689e31a743197b520007872ff9576237bda97bd1b469c3d7",
      size: 32_152_673,
    });
    expect(manifest.whisperTinyQ5.licence).toMatchObject({ sha256: SHA, spdx: "MIT" });
  });

  it("records the four supported ONNX Runtime archives and exact reviewed notices", () => {
    const manifest = buildRuntimeAssetManifest(ASSET_HASHES);

    expect(manifest.onnxRuntime.version).toBe("1.28.0");
    expect(manifest.onnxRuntime.archives.map((archive) => archive.target)).toEqual([
      "aarch64-apple-darwin",
      "x86_64-pc-windows-msvc",
      "x86_64-unknown-linux-gnu",
    ]);
    expect(manifest.onnxRuntime.archives.every((archive) => archive.sha256.match(/^[a-f0-9]{64}$/))).toBe(true);
    expect(manifest.onnxRuntime.licenceSha256).toBe(SHA);
    expect(manifest.onnxRuntime.thirdPartyNoticesSha256).toBe(SHA);
  });

  it("requires one exact explicit Gemma terms acknowledgement and emits no identity or timestamp", () => {
    const manifest = buildRuntimeAssetManifest(ASSET_HASHES);

    expect(() => buildTermsAcceptanceEvidence(manifest, "yes")).toThrow(/exact Gemma terms acknowledgement/);
    const evidence = buildTermsAcceptanceEvidence(manifest, GEMMA_TERMS_ACKNOWLEDGEMENT);
    const serialized = JSON.stringify(evidence);

    expect(evidence).toEqual({
      schemaVersion: 1,
      accepted: true,
      modelRevision: manifest.embeddingGemma.revision,
      termsSha256: manifest.embeddingGemma.terms.sha256,
    });
    expect(serialized).not.toMatch(/time|date|identity|user|path/i);
  });

  it("parses the documented terms-acceptance CLI argument without dropping its first character", () => {
    expect(parseTermsAcceptanceArgument(`--accept-gemma-terms=${GEMMA_TERMS_ACKNOWLEDGEMENT}`)).toBe(
      GEMMA_TERMS_ACKNOWLEDGEMENT,
    );
    expect(parseTermsAcceptanceArgument("--check")).toBeNull();
  });
});
