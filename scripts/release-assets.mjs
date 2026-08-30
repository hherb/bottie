#!/usr/bin/env node

/** Maintains Bottie's deterministic native runtime and downloaded-model asset contract. */

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const REPOSITORY_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const RUNTIME_ASSET_PATH = "runtime-assets.json";
const MODEL_NOTICE_PATH = "MODEL-NOTICE.txt";
const ONNX_RUNTIME_LICENCE_PATH = "third-party/onnxruntime-1.28.0/LICENSE";
const ONNX_RUNTIME_NOTICES_PATH = "third-party/onnxruntime-1.28.0/ThirdPartyNotices.txt";
const WHISPER_MODEL_LICENCE_PATH = "third-party/whisper.cpp-model/LICENSE";
const TERMS_EVIDENCE_PATH = "package/model-terms-evidence.json";
const TERMS_ACCEPTANCE_ARGUMENT = "--accept-gemma-terms=";
const SCHEMA_VERSION = 1;
const ONNX_RUNTIME_LICENCE_URL = "https://raw.githubusercontent.com/microsoft/onnxruntime/v1.28.0/LICENSE";
const ONNX_RUNTIME_NOTICES_URL =
  "https://raw.githubusercontent.com/microsoft/onnxruntime/v1.28.0/ThirdPartyNotices.txt";
const ONNX_RUNTIME_LICENCE_SHA256 = "2f07c72751aed99790b8a4869cf2311df85a860b22ded05fa22803587a48922c";
const ONNX_RUNTIME_NOTICES_SHA256 = "0e07b95f3a8d6230037707c5c4a2b554d12c4cb67369669ac255635528ffcee2";

const MODEL_NOTICE = `EmbeddingGemma model notice

Bottie uses the Q4 ONNX conversion of Google's EmbeddingGemma 300M model only for local semantic-memory indexing.
The model is downloaded on first use from the immutable repository revision recorded in runtime-assets.json and is
not bundled with Bottie's source or application packages.

EmbeddingGemma is provided under and subject to the Gemma Terms of Use found at:
https://ai.google.dev/gemma/terms

The reviewed terms version is dated 1 April 2026. Use of the model is subject to those terms and the incorporated
Gemma Prohibited Use Policy. Bottie's MIT licence does not replace or modify the Gemma terms.
`;

export const GEMMA_TERMS_ACKNOWLEDGEMENT =
  "I have read and accept the Gemma Terms of Use dated 2026-04-01 for Bottie 0.9.0 release review";

const EMBEDDING_GEMMA_FILES = [
  ["config.json", 1765, "6e1f06404b7163e0325ed2ea3e6781cde50f4a50b31780a95ad0d30e8404d77b"],
  ["onnx/model_q4.onnx", 519322, "ad1dfee81a70f7944b9b9d1cc6e48075b832881cf33fab2f2b248be78f3f0043"],
  ["onnx/model_q4.onnx_data", 196725760, "599962c3143b040de2dd05e5975be3e9091dd067cacc6a8f7186e3203bab9e02"],
  ["special_tokens_map.json", 662, "2f7b0adf4fb469770bb1490e3e35df87b1dc578246c5e7e6fc76ecf33213a397"],
  ["tokenizer.json", 20323312, "4dda02faaf32bc91031dc8c88457ac272b00c1016cc679757d1c441b248b9c47"],
  ["tokenizer_config.json", 1156830, "3ca953eea6c3c9fcda9cf3df22949ff18b216f7c74bd6459230f3f1013953f3a"],
];

const WHISPER_TINY_Q5 = {
  repository: "ggerganov/whisper.cpp",
  revision: "5359861c739e955e79d9a303bcbc70fb988958b1",
  variant: "tiny-q5_1-multilingual",
  file: {
    path: "ggml-tiny-q5_1.bin",
    sha256: "818710568da3ca15689e31a743197b520007872ff9576237bda97bd1b469c3d7",
    size: 32_152_673,
  },
};

const ONNX_RUNTIME_ARCHIVES = [
  [
    "aarch64-apple-darwin",
    "coreml",
    "https://cdn.pyke.io/0/pyke:ort-rs/ms@1.28.0/aarch64-apple-darwin+coreml.tar.lzma2",
    "6934874e2e953576d9c1db47ff1af39c62c4f4220dbe6f988e131f72879674c7",
  ],
  [
    "x86_64-pc-windows-msvc",
    "directml",
    "https://cdn.pyke.io/0/pyke:ort-rs/ms@1.28.0/x86_64-pc-windows-msvc+directml.tar.lzma2",
    "f7c654b3729cb9e5ad2a36a0c38e5b48e63bf4eed22968931aed33a0ad0b527d",
  ],
  [
    "x86_64-unknown-linux-gnu",
    "none",
    "https://cdn.pyke.io/0/pyke:ort-rs/ms@1.28.0/x86_64-unknown-linux-gnu.tar.lzma2",
    "e454f710f8a49f53aa5b4ff51e3454ae1835777e431c6c35c5255ce6f205fd68",
  ],
];

/** Builds the path-free immutable runtime-asset record from reviewed document hashes. */
export function buildRuntimeAssetManifest({
  onnxRuntimeLicenceSha256,
  onnxRuntimeNoticesSha256,
  whisperModelLicenceSha256,
}) {
  return {
    schemaVersion: SCHEMA_VERSION,
    embeddingGemma: {
      repository: "onnx-community/embeddinggemma-300m-ONNX",
      revision: "75a84c732f1884df76bec365346230e32f582c82",
      variant: "EmbeddingGemma300MQ4",
      files: EMBEDDING_GEMMA_FILES.map(([path, size, sha256]) => ({ path, sha256, size })),
      terms: {
        lastModified: "2026-04-01",
        notice: MODEL_NOTICE_PATH,
        sha256: sha256(MODEL_NOTICE),
        url: "https://ai.google.dev/gemma/terms",
      },
    },
    whisperTinyQ5: {
      ...WHISPER_TINY_Q5,
      licence: {
        path: WHISPER_MODEL_LICENCE_PATH,
        sha256: whisperModelLicenceSha256,
        source: "https://huggingface.co/ggerganov/whisper.cpp",
        spdx: "MIT",
      },
    },
    onnxRuntime: {
      version: "1.28.0",
      selectedBy: "ort-sys 2.0.0-rc.13",
      archives: ONNX_RUNTIME_ARCHIVES.map(([target, featureSet, url, sha256]) => ({
        featureSet,
        sha256,
        target,
        url,
      })),
      licence: ONNX_RUNTIME_LICENCE_PATH,
      licenceSha256: onnxRuntimeLicenceSha256,
      thirdPartyNotices: ONNX_RUNTIME_NOTICES_PATH,
      thirdPartyNoticesSha256: onnxRuntimeNoticesSha256,
    },
  };
}

/** Creates non-identifying evidence only after the release operator supplies the exact legal acknowledgement. */
export function buildTermsAcceptanceEvidence(manifest, acknowledgement) {
  if (acknowledgement !== GEMMA_TERMS_ACKNOWLEDGEMENT) {
    throw new Error("The exact Gemma terms acknowledgement is required.");
  }
  return {
    schemaVersion: SCHEMA_VERSION,
    accepted: true,
    modelRevision: manifest.embeddingGemma.revision,
    termsSha256: manifest.embeddingGemma.terms.sha256,
  };
}

/** Extracts the exact legal acknowledgement without relying on a duplicated prefix length. */
export function parseTermsAcceptanceArgument(argument) {
  if (!argument?.startsWith(TERMS_ACCEPTANCE_ARGUMENT)) return null;
  return argument.slice(TERMS_ACCEPTANCE_ARGUMENT.length);
}

/** Returns one lowercase SHA-256 digest for bytes or text. */
function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

/** Downloads one versioned upstream text only when its reviewed digest matches. */
async function refreshUpstreamSource(url, relativePath, expectedSha256) {
  const response = await fetch(url, { redirect: "error" });
  if (!response.ok) throw new Error(`Could not refresh ${relativePath} from its versioned upstream source.`);
  const bytes = Buffer.from(await response.arrayBuffer());
  if (sha256(bytes) !== expectedSha256) throw new Error(`${relativePath} did not match its reviewed SHA-256.`);
  const path = join(REPOSITORY_ROOT, relativePath);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, bytes);
}

/** Writes generated sources or checks that every committed byte is current. */
async function main() {
  if (process.argv.includes("--refresh-upstream")) {
    await refreshUpstreamSource(ONNX_RUNTIME_LICENCE_URL, ONNX_RUNTIME_LICENCE_PATH, ONNX_RUNTIME_LICENCE_SHA256);
    await refreshUpstreamSource(ONNX_RUNTIME_NOTICES_URL, ONNX_RUNTIME_NOTICES_PATH, ONNX_RUNTIME_NOTICES_SHA256);
  }
  const licence = readFileSync(join(REPOSITORY_ROOT, ONNX_RUNTIME_LICENCE_PATH));
  const notices = readFileSync(join(REPOSITORY_ROOT, ONNX_RUNTIME_NOTICES_PATH));
  const whisperModelLicence = readFileSync(join(REPOSITORY_ROOT, WHISPER_MODEL_LICENCE_PATH));
  const manifest = buildRuntimeAssetManifest({
    onnxRuntimeLicenceSha256: sha256(licence),
    onnxRuntimeNoticesSha256: sha256(notices),
    whisperModelLicenceSha256: sha256(whisperModelLicence),
  });
  const outputs = new Map([
    [MODEL_NOTICE_PATH, MODEL_NOTICE],
    [RUNTIME_ASSET_PATH, `${JSON.stringify(manifest, null, 2)}\n`],
  ]);
  if (process.argv.includes("--check")) {
    for (const [path, expected] of outputs) {
      const absolutePath = join(REPOSITORY_ROOT, path);
      if (!existsSync(absolutePath) || readFileSync(absolutePath, "utf8") !== expected) {
        throw new Error(`${path} is stale; run node scripts/release-assets.mjs --write.`);
      }
    }
    console.log("[bottie] runtime-assets.json and MODEL-NOTICE.txt match the reviewed contract.");
    return;
  }
  const acceptanceArgument = process.argv.find((argument) => argument.startsWith(TERMS_ACCEPTANCE_ARGUMENT));
  if (acceptanceArgument) {
    const evidence = buildTermsAcceptanceEvidence(manifest, parseTermsAcceptanceArgument(acceptanceArgument));
    const path = join(REPOSITORY_ROOT, TERMS_EVIDENCE_PATH);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, `${JSON.stringify(evidence, null, 2)}\n`);
    console.log(`[bottie] wrote ${relative(REPOSITORY_ROOT, path)} without identity or timestamp.`);
    return;
  }
  for (const [path, source] of outputs) writeFileSync(join(REPOSITORY_ROOT, path), source);
  console.log(`[bottie] wrote ${[...outputs.keys()].join(" and ")}.`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    await main();
  } catch (error) {
    console.error(`[bottie] ${error instanceof Error ? error.message : "Release-asset preparation failed."}`);
    process.exitCode = 1;
  }
}
