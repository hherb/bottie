#!/usr/bin/env node

/** Binds Bottie's ordinary ready candidate to complete protected Python platform evidence. */

import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { validateProtectedPythonPlatformEvidence } from "./python-protected-platforms.mjs";

const SCHEMA_VERSION = 1;
const SOURCE_SHA_PATTERN = /^[a-f0-9]{40}$/;
const SHA256_PATTERN = /^[a-f0-9]{64}$/;
const SEMVER_PATTERN = /^\d+\.\d+\.\d+$/;
const RELEASE_GATE_IDS = [
  "release-notes",
  "version-alignment",
  "dependency-inventory-current",
  "dependency-review",
  "licence-and-notices",
  "runtime-assets",
  "model-terms",
  "artwork",
  "macos-distribution",
  "windows-package",
  "windows-distribution",
  "linux-package",
  "linux-distribution",
];

/** Binds two independently accepted records without signing, releasing, or publishing anything. */
export function bindProtectedPythonReleaseEligibility(sourceSha, releaseCandidate, platformEvidence) {
  requireSourceSha(sourceSha);
  const release = validateReleaseCandidate(releaseCandidate);
  let protectedPlatforms;
  try {
    protectedPlatforms = validateProtectedPythonPlatformEvidence(sourceSha, platformEvidence);
  } catch {
    throw new Error("The protected Python platform evidence is invalid.");
  }
  return {
    schemaVersion: SCHEMA_VERSION,
    sourceSha,
    status: "eligible",
    release: release.release,
    releaseCandidateSha256: sha256(canonicalJson(releaseCandidate)),
    protectedPlatformEvidenceSha256: sha256(canonicalJson(protectedPlatforms)),
    protectedPythonReleaseCandidateSha256: protectedPlatforms.releaseCandidateSha256,
    runtimeCore: protectedPlatforms.runtimeCore,
    platforms: protectedPlatforms.platforms.map((record) => ({
      platform: record.platform,
      target: record.target,
      protectedInspectionSha256: record.protectedInspectionSha256,
      containmentSha256: record.containmentSha256,
      runner: record.runner,
      nativeTransports: record.nativeTransports,
    })),
  };
}

/** Validates the closed normalized ordinary release-candidate record and all passed gates. */
function validateReleaseCandidate(candidate) {
  requireExactKeys(
    candidate,
    ["artifacts", "gates", "inputs", "ready", "release", "schemaVersion"],
    "The ordinary release candidate",
  );
  if (candidate.schemaVersion !== SCHEMA_VERSION || candidate.ready !== true) {
    throw new Error("The ordinary release candidate is not ready.");
  }
  requireExactKeys(candidate.release, ["channel", "notesSha256", "tag", "title", "version"], "The release metadata");
  const { channel, notesSha256, tag, title, version } = candidate.release;
  if (
    channel !== "beta" ||
    !SEMVER_PATTERN.test(version) ||
    tag !== `v${version}` ||
    title !== `Bottie ${version} beta` ||
    !isSha256(notesSha256)
  ) {
    throw new Error("The ordinary release metadata is invalid.");
  }
  const inputFields = [
    "dependencyInventorySha256",
    "licenceSha256",
    "modelTermsSha256",
    "noticesSha256",
    "runtimeAssetsSha256",
  ];
  requireExactKeys(candidate.inputs, inputFields, "The ordinary release candidate inputs");
  if (!inputFields.every((field) => isSha256(candidate.inputs[field]))) {
    throw new Error("The ordinary release candidate inputs are incomplete.");
  }
  requireExactKeys(candidate.artifacts, ["linux", "macos", "windows"], "The ordinary release artifacts");
  if (!Object.values(candidate.artifacts).every(isNonEmptyObject)) {
    throw new Error("The ordinary release artifacts are incomplete.");
  }
  if (!Array.isArray(candidate.gates) || candidate.gates.length !== RELEASE_GATE_IDS.length) {
    throw new Error("The ordinary release gates are incomplete.");
  }
  candidate.gates.forEach((gate, index) => {
    requireExactKeys(gate, ["id", "passed"], "The ordinary release gates");
    if (gate.id !== RELEASE_GATE_IDS[index] || gate.passed !== true) {
      throw new Error("The ordinary release gates are incomplete.");
    }
  });
  return { release: { channel, notesSha256, tag, title, version } };
}

/** Requires an object to contain exactly one allowlisted field set. */
function requireExactKeys(value, expected, label) {
  const actual = value && typeof value === "object" && !Array.isArray(value) ? Object.keys(value).sort() : [];
  const sortedExpected = [...expected].sort();
  if (actual.length !== sortedExpected.length || actual.some((key, index) => key !== sortedExpected[index])) {
    throw new Error(`${label} is not closed.`);
  }
}

/** Returns whether a value is a non-empty record. */
function isNonEmptyObject(value) {
  return value && typeof value === "object" && !Array.isArray(value) && Object.keys(value).length > 0;
}

/** Requires one exact lowercase Git source revision. */
function requireSourceSha(sourceSha) {
  if (typeof sourceSha !== "string" || !SOURCE_SHA_PATTERN.test(sourceSha)) {
    throw new Error("The protected Python release source revision is invalid.");
  }
}

/** Returns whether a value is one lowercase SHA-256 digest. */
function isSha256(value) {
  return typeof value === "string" && SHA256_PATTERN.test(value);
}

/** Serializes JSON with recursively sorted object keys for stable bindings. */
function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

/** Returns one lowercase SHA-256 digest for text. */
function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

/** Reads one required JSON input without reflecting its host path. */
async function readJson(path) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch {
    throw new Error("Required protected Python release evidence is unavailable or malformed.");
  }
}

/** Writes one private path-free eligibility record. */
async function writeJson(path, value) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
}

/** Dispatches the exact credential-free release-eligibility binding command. */
async function main() {
  const [mode, ...arguments_] = process.argv.slice(2);
  if (mode === "--bind" && arguments_.length === 4) {
    const [sourceSha, releaseCandidatePath, platformEvidencePath, outputPath] = arguments_;
    await writeJson(
      resolve(outputPath),
      bindProtectedPythonReleaseEligibility(
        sourceSha,
        await readJson(resolve(releaseCandidatePath)),
        await readJson(resolve(platformEvidencePath)),
      ),
    );
    console.log("[bottie] Credential-free protected Python release eligibility accepted.");
    return;
  }
  throw new Error("Use --bind with the source, release candidate, platform evidence, and output paths.");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`[bottie] ${error instanceof Error ? error.message : "Protected release binding failed."}`);
    process.exitCode = 1;
  });
}
