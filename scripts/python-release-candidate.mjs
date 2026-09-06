#!/usr/bin/env node

/** Binds Bottie's credential-free packaged Python proofs to one source revision. */

import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SCHEMA_VERSION = 1;
const SOURCE_SHA_PATTERN = /^[a-f0-9]{40}$/;
const SHA256_PATTERN = /^[a-f0-9]{64}$/;
const MAX_RUNTIME_FILES = 4_096;
const MAX_RUNTIME_BYTES = 128 * 1_024 * 1_024;
const PLATFORMS = ["linux", "macos", "windows"];
const PLATFORM_CONTAINMENT_FIELDS = {
  linux: [
    "cancellation",
    "environmentIsolated",
    "execDenied",
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
    "credentialFreeEphemeralSignaturesVerified",
    "inspectedPackagedBytes",
    "privatePipeExecution",
  ],
  windows: [
    "appContainerDeniedHostFixture",
    "appContainerLowIntegrity",
    "appContainerNoCapabilities",
    "cancellation",
    "installedDevelopmentBundle",
    "jobCloseKilledRunner",
    "privatePipeExecution",
    "privilegesStripped",
    "resourceLimits",
  ],
};
const PLATFORM_TARGETS = {
  linux: ["x86_64-unknown-linux-gnu"],
  macos: ["aarch64-apple-darwin", "x86_64-apple-darwin"],
  windows: ["x86_64-pc-windows-msvc"],
};
const PLATFORM_TRANSPORTS = {
  linux: [],
  macos: [
    "Contents/Helpers/BottiePythonXPCClient.app/Contents/Info.plist",
    "Contents/Helpers/BottiePythonXPCClient.app/Contents/MacOS/bottie-python-xpc-client",
    "Contents/Helpers/BottiePythonXPCClient.app/Contents/XPCServices/" +
      "com.bottie.python-runner.xpc/Contents/Info.plist",
    "Contents/Helpers/BottiePythonXPCClient.app/Contents/XPCServices/" +
      "com.bottie.python-runner.xpc/Contents/MacOS/bottie-python-xpc-service",
  ],
  windows: ["bottie-python-appcontainer.exe"],
};

/** Creates a closed source-revision marker for one platform proof job. */
export function buildSourceMarker(platform, sourceSha) {
  requirePlatform(platform);
  requireSourceSha(sourceSha);
  return { schemaVersion: SCHEMA_VERSION, platform, sourceSha };
}

/** Validates and binds the complete three-platform development evidence set. */
export function bindPythonReleaseCandidate(sourceSha, evidenceByPlatform) {
  requireSourceSha(sourceSha);
  requireExactKeys(evidenceByPlatform, PLATFORMS, "The Python evidence set");
  const validated = PLATFORMS.map((platform) => validatePlatformEvidence(platform, evidenceByPlatform[platform]));
  const runtimes = Object.fromEntries(validated.map((item) => [item.platform, item.inspection.runtime]));
  const sharedRuntimeCore = runtimeCore(runtimes.linux);
  if (
    validated.some((item) => canonicalJson(runtimeCore(item.inspection.runtime)) !== canonicalJson(sharedRuntimeCore))
  ) {
    throw new Error("The packaged Python runtime core is inconsistent across platforms.");
  }
  if (canonicalJson(runtimes.macos) !== canonicalJson(runtimes.linux)) {
    throw new Error("The macOS and Linux packaged Python runtime identities are inconsistent.");
  }
  if (
    runtimes.windows.fileCount !== runtimes.linux.fileCount + 1 ||
    runtimes.windows.totalBytes <= runtimes.linux.totalBytes
  ) {
    throw new Error("The Windows packaged Python runtime layout is inconsistent.");
  }
  if (validated.some((item) => item.marker.sourceSha !== sourceSha)) {
    throw new Error("The Python evidence source revision does not match the release candidate.");
  }
  return {
    schemaVersion: SCHEMA_VERSION,
    sourceSha,
    status: "accepted",
    runtimeCore: sharedRuntimeCore,
    platforms: validated.map(({ containment, inspection, installedInspection, platform }) => ({
      containment,
      containmentSha256: evidenceSha256(containment),
      inspectionSha256: evidenceSha256(inspection),
      installedInspectionSha256: installedInspection ? evidenceSha256(installedInspection) : null,
      nativeTransports: inspection.nativeTransports,
      platform,
      runner: { bytes: inspection.runnerBytes, sha256: inspection.runnerSha256 },
      runtime: inspection.runtime,
      target: inspection.target,
    })),
  };
}

/** Selects the immutable runtime fields that must agree despite platform-specific packaging. */
function runtimeCore(runtime) {
  return {
    schemaVersion: runtime.schemaVersion,
    licenceSha256: runtime.licenceSha256,
    pythonVersion: runtime.pythonVersion,
    pythonWasmSha256: runtime.pythonWasmSha256,
    wasiSdkVersion: runtime.wasiSdkVersion,
  };
}

/** Validates one platform's closed marker, package identity, and containment result. */
function validatePlatformEvidence(platform, evidence) {
  requireExactKeys(
    evidence,
    ["containment", "inspection", "installedInspection", "marker"],
    `The ${platform} Python evidence`,
  );
  const marker = validateMarker(platform, evidence.marker);
  const inspection = validateInspection(platform, evidence.inspection);
  const installedInspection = evidence.installedInspection;
  if (platform === "macos") {
    if (installedInspection !== undefined && installedInspection !== null) {
      throw new Error("The macOS development proof must not claim installed-package inspection.");
    }
  } else {
    validateInspection(platform, installedInspection);
    if (canonicalJson(installedInspection) !== canonicalJson(inspection)) {
      throw new Error(`The ${platform} installed inspection does not match the extracted package.`);
    }
  }
  return {
    containment: validateContainment(platform, evidence.containment),
    inspection,
    installedInspection: platform === "macos" ? null : installedInspection,
    marker,
    platform,
  };
}

/** Validates one exact platform source marker. */
function validateMarker(platform, marker) {
  requireExactKeys(marker, ["platform", "schemaVersion", "sourceSha"], "The Python source marker");
  if (marker.schemaVersion !== SCHEMA_VERSION || marker.platform !== platform) {
    throw new Error("The Python source marker does not match its platform.");
  }
  requireSourceSha(marker.sourceSha);
  return { schemaVersion: SCHEMA_VERSION, platform, sourceSha: marker.sourceSha };
}

/** Validates and normalizes one path-free packaged runtime inspection. */
function validateInspection(platform, inspection) {
  requireExactKeys(
    inspection,
    ["bundled", "nativeTransports", "runnerBytes", "runnerSha256", "runtime", "target"],
    `The ${platform} package inspection`,
  );
  if (
    inspection.bundled !== true ||
    !PLATFORM_TARGETS[platform].includes(inspection.target) ||
    !isPositiveInteger(inspection.runnerBytes) ||
    !isSha256(inspection.runnerSha256)
  ) {
    throw new Error(`The ${platform} package inspection is incomplete.`);
  }
  const nativeTransports = validateTransports(platform, inspection.nativeTransports);
  return {
    bundled: true,
    nativeTransports,
    runnerBytes: inspection.runnerBytes,
    runnerSha256: inspection.runnerSha256,
    runtime: validateRuntime(inspection.runtime),
    target: inspection.target,
  };
}

/** Requires the exact platform-relative transport set and immutable byte identity. */
function validateTransports(platform, transports) {
  const expectedPaths = PLATFORM_TRANSPORTS[platform];
  if (!Array.isArray(transports) || transports.length !== expectedPaths.length) {
    throw new Error(`The ${platform} native transport evidence is incomplete.`);
  }
  return transports.map((transport, index) => {
    requireExactKeys(transport, ["bytes", "path", "sha256"], `The ${platform} native transport evidence`);
    if (transport.path !== expectedPaths[index] || !isPositiveInteger(transport.bytes) || !isSha256(transport.sha256)) {
      throw new Error(`The ${platform} native transport evidence is incomplete.`);
    }
    return { bytes: transport.bytes, path: transport.path, sha256: transport.sha256 };
  });
}

/** Requires the exact reviewed CPython/WASI runtime evidence shape and bounds. */
function validateRuntime(runtime) {
  requireExactKeys(
    runtime,
    [
      "fileCount",
      "licenceSha256",
      "pythonVersion",
      "pythonWasmSha256",
      "runtimeTreeSha256",
      "schemaVersion",
      "totalBytes",
      "wasiSdkVersion",
    ],
    "The packaged Python runtime evidence",
  );
  if (
    runtime.schemaVersion !== SCHEMA_VERSION ||
    runtime.pythonVersion !== "3.14.7" ||
    runtime.wasiSdkVersion !== "24" ||
    !isPositiveInteger(runtime.fileCount) ||
    runtime.fileCount > MAX_RUNTIME_FILES ||
    !isPositiveInteger(runtime.totalBytes) ||
    runtime.totalBytes > MAX_RUNTIME_BYTES ||
    ![runtime.licenceSha256, runtime.pythonWasmSha256, runtime.runtimeTreeSha256].every(isSha256)
  ) {
    throw new Error("The packaged Python runtime evidence is incomplete.");
  }
  return { ...runtime };
}

/** Requires one exact closed all-success containment record. */
function validateContainment(platform, containment) {
  const fields = PLATFORM_CONTAINMENT_FIELDS[platform];
  requireExactKeys(containment, [...fields, "status"], `The ${platform} containment evidence`);
  if (containment.status !== "ok" || !fields.every((field) => containment[field] === true)) {
    throw new Error(`The ${platform} containment evidence is incomplete.`);
  }
  return Object.fromEntries([...fields, "status"].sort().map((field) => [field, containment[field]]));
}

/** Requires an object to contain exactly one allowlisted set of fields. */
function requireExactKeys(value, expected, label) {
  const actual = value && typeof value === "object" && !Array.isArray(value) ? Object.keys(value).sort() : [];
  const sortedExpected = [...expected].sort();
  if (actual.length !== sortedExpected.length || actual.some((key, index) => key !== sortedExpected[index])) {
    const suffix = label === "The Python evidence set" ? " must contain exactly three platforms." : " is not closed.";
    throw new Error(`${label}${suffix}`);
  }
}

/** Requires one supported desktop platform. */
function requirePlatform(platform) {
  if (!PLATFORMS.includes(platform)) throw new Error("The Python evidence platform is unsupported.");
}

/** Requires one exact lowercase Git source revision. */
function requireSourceSha(sourceSha) {
  if (typeof sourceSha !== "string" || !SOURCE_SHA_PATTERN.test(sourceSha)) {
    throw new Error("The Python evidence source revision is invalid.");
  }
}

/** Returns whether a value is a positive safe integer. */
function isPositiveInteger(value) {
  return Number.isSafeInteger(value) && value > 0;
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

/** Hashes one already-validated evidence object canonically. */
function evidenceSha256(value) {
  return createHash("sha256").update(canonicalJson(value)).digest("hex");
}

/** Reads one required JSON input without retaining its host path in errors. */
async function readJson(path) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch {
    throw new Error("Required Python release-candidate evidence is unavailable or malformed.");
  }
}

/** Writes one private path-free JSON evidence document. */
async function writeJson(path, value) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
}

/** Loads the fixed platform evidence filenames from one merged artifact directory. */
async function loadEvidence(directory) {
  const result = {};
  for (const platform of PLATFORMS) {
    result[platform] = {
      containment: await readJson(join(directory, `${platform}-containment.json`)),
      inspection: await readJson(join(directory, `${platform}.json`)),
      installedInspection: platform === "macos" ? null : await readJson(join(directory, `${platform}-installed.json`)),
      marker: await readJson(join(directory, `${platform}-revision.json`)),
    };
  }
  return result;
}

/** Dispatches the exact marker and aggregate binding command modes. */
async function main() {
  const [mode, ...arguments_] = process.argv.slice(2);
  if (mode === "--stamp" && arguments_.length === 3) {
    await writeJson(resolve(arguments_[2]), buildSourceMarker(arguments_[0], arguments_[1]));
    return;
  }
  if (mode === "--bind" && arguments_.length === 3) {
    const [sourceSha, inputDirectory, outputPath] = arguments_;
    await writeJson(
      resolve(outputPath),
      bindPythonReleaseCandidate(sourceSha, await loadEvidence(resolve(inputDirectory))),
    );
    console.log("[bottie] Credential-free Python release-candidate evidence accepted.");
    return;
  }
  throw new Error("Use --stamp or --bind with exact inputs.");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`[bottie] ${error instanceof Error ? error.message : "Python evidence binding failed."}`);
    process.exitCode = 1;
  });
}
