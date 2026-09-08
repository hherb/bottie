#!/usr/bin/env node

/** Binds the complete protected Python platform set without using credentials or package bytes. */

import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { protectedInspectionSha256, validateProtectedPackageInspection } from "./python-protected-package.mjs";

const SCHEMA_VERSION = 1;
const SOURCE_SHA_PATTERN = /^[a-f0-9]{40}$/;
const SHA256_PATTERN = /^[a-f0-9]{64}$/;
const PLATFORMS = ["linux", "macos", "windows"];
const COMPARISON_FIELDS = [
  "containment",
  "containmentSha256",
  "nativeTransports",
  "platform",
  "protectedInspectionSha256",
  "releaseCandidateInspectionSha256",
  "releaseCandidateSha256",
  "runner",
  "runtime",
  "schemaVersion",
  "sourceSha",
  "status",
  "target",
];
const CONTAINMENT_METADATA_FIELDS = ["inspectionSha256", "platform", "schemaVersion", "sourceSha", "status", "target"];
const SHIPPING_CONTAINMENT_FIELDS = {
  linux: [
    "cancellation",
    "environmentIsolated",
    "execDenied",
    "installedProtectedPackage",
    "landlockDeniedHostFixture",
    "networkDenied",
    "parentCloseKilledRunner",
    "parentDeathSignal",
    "processCreationDenied",
    "resourceLimits",
    "runtimeReadable",
    "workspaceReadable",
  ],
  macos: [
    "appSandboxDeniedHostFixture",
    "cancellation",
    "clientExitKilledRunner",
    "inspectedProtectedPackage",
    "privatePipeExecution",
  ],
  windows: [
    "appContainerDeniedHostFixture",
    "appContainerLowIntegrity",
    "appContainerNoCapabilities",
    "cancellation",
    "installedProtectedPackage",
    "jobCloseKilledRunner",
    "privatePipeExecution",
    "privilegesStripped",
    "resourceLimits",
  ],
};

/** Binds three independently accepted protected-package comparisons into one closed record. */
export function bindProtectedPythonPlatforms(sourceSha, comparisons) {
  requireSourceSha(sourceSha);
  requireExactKeys(
    comparisons,
    PLATFORMS,
    "The protected Python comparison set",
    " must contain exactly three platforms.",
  );
  const platforms = PLATFORMS.map((platform) => validateComparison(sourceSha, platform, comparisons[platform]));
  const releaseCandidateSha256 = platforms[0].releaseCandidateSha256;
  if (platforms.some((record) => record.releaseCandidateSha256 !== releaseCandidateSha256)) {
    throw new Error("The protected Python comparisons do not share one release candidate.");
  }
  const runtimeCore = runtimeCoreFor(platforms[0].runtime);
  if (platforms.some((record) => canonicalJson(runtimeCoreFor(record.runtime)) !== canonicalJson(runtimeCore))) {
    throw new Error("The protected Python comparisons do not share one runtime core.");
  }
  if (canonicalJson(platforms[0].runtime) !== canonicalJson(platforms[1].runtime)) {
    throw new Error("The protected Linux and macOS runtime layouts do not match.");
  }
  if (
    platforms[2].runtime.fileCount !== platforms[0].runtime.fileCount + 1 ||
    platforms[2].runtime.totalBytes <= platforms[0].runtime.totalBytes
  ) {
    throw new Error("The protected Windows runtime layout is invalid.");
  }
  return {
    schemaVersion: SCHEMA_VERSION,
    sourceSha,
    status: "accepted",
    releaseCandidateSha256,
    runtimeCore,
    platforms,
  };
}

/** Revalidates one complete aggregate instead of trusting its accepted label or stored digests. */
export function validateProtectedPythonPlatformEvidence(sourceSha, evidence) {
  requireExactKeys(
    evidence,
    ["platforms", "releaseCandidateSha256", "runtimeCore", "schemaVersion", "sourceSha", "status"],
    "The protected Python platform evidence",
  );
  if (
    evidence.schemaVersion !== SCHEMA_VERSION ||
    evidence.sourceSha !== sourceSha ||
    evidence.status !== "accepted" ||
    !Array.isArray(evidence.platforms) ||
    evidence.platforms.length !== PLATFORMS.length
  ) {
    throw new Error("The protected Python platform evidence is invalid.");
  }
  const comparisons = Object.fromEntries(
    evidence.platforms.map((record, index) => {
      if (record?.platform !== PLATFORMS[index]) {
        throw new Error("The protected Python platform evidence is invalid.");
      }
      return [record.platform, record];
    }),
  );
  const rebuilt = bindProtectedPythonPlatforms(sourceSha, comparisons);
  if (canonicalJson(rebuilt) !== canonicalJson(evidence)) {
    throw new Error("The protected Python platform evidence is invalid.");
  }
  return rebuilt;
}

/** Revalidates one comparison record and all of its canonical bindings. */
function validateComparison(sourceSha, platform, record) {
  requireExactKeys(record, COMPARISON_FIELDS, "The protected Python comparison record");
  if (record.schemaVersion !== SCHEMA_VERSION || record.status !== "accepted" || record.platform !== platform) {
    throw new Error("The protected Python comparison record is incomplete.");
  }
  if (record.sourceSha !== sourceSha) {
    throw new Error("The protected Python comparison source revision does not match.");
  }
  if (!isSha256(record.releaseCandidateSha256) || !isSha256(record.releaseCandidateInspectionSha256)) {
    throw new Error("The protected Python comparison release candidate is invalid.");
  }
  requireExactKeys(record.runner, ["bytes", "sha256"], "The protected Python comparison runner");
  const inspection = validateProtectedPackageInspection(platform, {
    bundled: true,
    nativeTransports: record.nativeTransports,
    runnerBytes: record.runner.bytes,
    runnerSha256: record.runner.sha256,
    runtime: record.runtime,
    target: record.target,
  });
  const inspectionSha256 = protectedInspectionSha256(inspection);
  if (record.protectedInspectionSha256 !== inspectionSha256) {
    throw new Error("The protected Python comparison inspection binding is invalid.");
  }
  const containment = validateContainment(sourceSha, platform, inspection.target, inspectionSha256, record.containment);
  if (record.containmentSha256 !== sha256(canonicalJson(containment))) {
    throw new Error("The protected Python comparison containment binding is invalid.");
  }
  return {
    schemaVersion: SCHEMA_VERSION,
    sourceSha,
    status: "accepted",
    platform,
    target: inspection.target,
    releaseCandidateSha256: record.releaseCandidateSha256,
    releaseCandidateInspectionSha256: record.releaseCandidateInspectionSha256,
    protectedInspectionSha256: inspectionSha256,
    runner: { bytes: inspection.runnerBytes, sha256: inspection.runnerSha256 },
    runtime: inspection.runtime,
    nativeTransports: inspection.nativeTransports,
    containment,
    containmentSha256: record.containmentSha256,
  };
}

/** Revalidates one closed native shipping-containment record. */
function validateContainment(sourceSha, platform, target, inspectionSha256, containment) {
  const fields = SHIPPING_CONTAINMENT_FIELDS[platform];
  requireExactKeys(
    containment,
    [...CONTAINMENT_METADATA_FIELDS, ...fields],
    "The protected Python comparison containment",
  );
  if (
    containment.schemaVersion !== SCHEMA_VERSION ||
    containment.sourceSha !== sourceSha ||
    containment.platform !== platform ||
    containment.target !== target ||
    containment.inspectionSha256 !== inspectionSha256 ||
    containment.status !== "ok" ||
    !fields.every((field) => containment[field] === true)
  ) {
    throw new Error("The protected Python comparison containment is incomplete.");
  }
  return Object.fromEntries(
    [...CONTAINMENT_METADATA_FIELDS, ...fields].sort().map((field) => [field, containment[field]]),
  );
}

/** Selects the runtime fields that must agree across platform-specific package layouts. */
function runtimeCoreFor(runtime) {
  return {
    schemaVersion: runtime.schemaVersion,
    licenceSha256: runtime.licenceSha256,
    pythonVersion: runtime.pythonVersion,
    pythonWasmSha256: runtime.pythonWasmSha256,
    wasiSdkVersion: runtime.wasiSdkVersion,
  };
}

/** Requires an object to contain exactly one allowlisted field set. */
function requireExactKeys(value, expected, label, suffix = " is not closed.") {
  const actual = value && typeof value === "object" && !Array.isArray(value) ? Object.keys(value).sort() : [];
  const sortedExpected = [...expected].sort();
  if (actual.length !== sortedExpected.length || actual.some((key, index) => key !== sortedExpected[index])) {
    throw new Error(`${label}${suffix}`);
  }
}

/** Requires one exact lowercase Git source revision. */
function requireSourceSha(sourceSha) {
  if (typeof sourceSha !== "string" || !SOURCE_SHA_PATTERN.test(sourceSha)) {
    throw new Error("The protected Python source revision is invalid.");
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
    throw new Error("Required protected Python platform evidence is unavailable or malformed.");
  }
}

/** Loads the fixed comparison filenames from one private merged-artifact directory. */
async function loadComparisons(directory) {
  return Object.fromEntries(
    await Promise.all(
      PLATFORMS.map(async (platform) => [
        platform,
        await readJson(join(directory, `${platform}-protected-comparison.json`)),
      ]),
    ),
  );
}

/** Writes one private path-free protected-platform evidence document. */
async function writeJson(path, value) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
}

/** Dispatches the exact credential-free protected-platform binding command. */
async function main() {
  const [mode, ...arguments_] = process.argv.slice(2);
  if (mode === "--bind" && arguments_.length === 3) {
    const [sourceSha, inputDirectory, outputPath] = arguments_;
    await writeJson(
      resolve(outputPath),
      bindProtectedPythonPlatforms(sourceSha, await loadComparisons(resolve(inputDirectory))),
    );
    console.log("[bottie] Credential-free protected Python platform evidence accepted.");
    return;
  }
  throw new Error("Use --bind with the exact source revision, input directory, and output path.");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`[bottie] ${error instanceof Error ? error.message : "Protected platform binding failed."}`);
    process.exitCode = 1;
  });
}
