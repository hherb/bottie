#!/usr/bin/env node

/** Compares one future protected Python package with accepted development evidence. */

import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SCHEMA_VERSION = 1;
const SOURCE_SHA_PATTERN = /^[a-f0-9]{40}$/;
const SHA256_PATTERN = /^[a-f0-9]{64}$/;
const MAX_RUNTIME_FILES = 4_096;
const MAX_RUNTIME_BYTES = 128 * 1_024 * 1_024;
const PLATFORMS = ["linux", "macos", "windows"];
const DEVELOPMENT_CONTAINMENT_FIELDS = {
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
const CONTAINMENT_METADATA_FIELDS = ["inspectionSha256", "platform", "schemaVersion", "sourceSha", "status", "target"];
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

/** Returns the canonical digest used to bind containment to one protected inspection. */
export function protectedInspectionSha256(inspection) {
  return sha256(canonicalJson(inspection));
}

/** Validates one protected inspection and returns the matching accepted candidate evidence. */
function validatedProtectedPythonInspection(sourceSha, platform, releaseCandidate, inspection) {
  requireSourceSha(sourceSha);
  requirePlatform(platform);
  const candidate = validatedReleaseCandidate(releaseCandidate);
  if (candidate.sourceSha !== sourceSha) {
    throw new Error("The protected Python source revision does not match the release candidate.");
  }
  const protectedInspection = validateInspection(platform, inspection, "protected package");
  const candidatePlatform = candidate.platforms.find((item) => item.platform === platform);
  if (canonicalJson(protectedInspection.runtime) !== canonicalJson(candidatePlatform.runtime)) {
    throw new Error("The protected Python runtime identity does not match the release candidate.");
  }
  return { candidate, candidatePlatform, inspection: protectedInspection };
}

/** Validates one protected inspection against the accepted platform runtime before containment exists. */
export function validateProtectedPythonInspection(sourceSha, platform, releaseCandidate, inspection) {
  return validatedProtectedPythonInspection(sourceSha, platform, releaseCandidate, inspection).inspection;
}

/** Binds one protected package to its development runtime identity and native containment proof. */
export function bindProtectedPythonPackage(sourceSha, platform, releaseCandidate, inspection, containment) {
  const {
    candidate,
    candidatePlatform,
    inspection: protectedInspection,
  } = validatedProtectedPythonInspection(sourceSha, platform, releaseCandidate, inspection);
  const protectedInspectionDigest = protectedInspectionSha256(protectedInspection);
  const shippingContainment = validateShippingContainment(
    sourceSha,
    platform,
    protectedInspection.target,
    protectedInspectionDigest,
    containment,
  );
  return {
    schemaVersion: SCHEMA_VERSION,
    sourceSha,
    status: "accepted",
    platform,
    target: protectedInspection.target,
    releaseCandidateSha256: sha256(canonicalJson(candidate)),
    releaseCandidateInspectionSha256: candidatePlatform.inspectionSha256,
    protectedInspectionSha256: protectedInspectionDigest,
    runner: { bytes: protectedInspection.runnerBytes, sha256: protectedInspection.runnerSha256 },
    runtime: protectedInspection.runtime,
    nativeTransports: protectedInspection.nativeTransports,
    containment: shippingContainment,
    containmentSha256: sha256(canonicalJson(shippingContainment)),
  };
}

/** Validates the accepted three-platform manifest instead of trusting its accepted label. */
function validatedReleaseCandidate(candidate) {
  try {
    requireExactKeys(
      candidate,
      ["platforms", "runtimeCore", "schemaVersion", "sourceSha", "status"],
      "The release-candidate evidence",
    );
    if (candidate.schemaVersion !== SCHEMA_VERSION || candidate.status !== "accepted") throw new Error();
    requireSourceSha(candidate.sourceSha);
    const runtimeCore = validateRuntimeCore(candidate.runtimeCore);
    if (!Array.isArray(candidate.platforms) || candidate.platforms.length !== PLATFORMS.length) throw new Error();
    const platforms = candidate.platforms.map((entry, index) => validateCandidatePlatform(PLATFORMS[index], entry));
    if (platforms.some((entry) => canonicalJson(runtimeCoreFor(entry.runtime)) !== canonicalJson(runtimeCore))) {
      throw new Error();
    }
    if (canonicalJson(platforms[0].runtime) !== canonicalJson(platforms[1].runtime)) throw new Error();
    if (
      platforms[2].runtime.fileCount !== platforms[0].runtime.fileCount + 1 ||
      platforms[2].runtime.totalBytes <= platforms[0].runtime.totalBytes
    ) {
      throw new Error();
    }
    return {
      schemaVersion: SCHEMA_VERSION,
      sourceSha: candidate.sourceSha,
      status: "accepted",
      runtimeCore,
      platforms,
    };
  } catch {
    throw new Error("The Python release-candidate evidence is invalid.");
  }
}

/** Validates one closed platform entry and its canonical input digests. */
function validateCandidatePlatform(platform, entry) {
  requireExactKeys(
    entry,
    [
      "containment",
      "containmentSha256",
      "inspectionSha256",
      "installedInspectionSha256",
      "nativeTransports",
      "platform",
      "runner",
      "runtime",
      "target",
    ],
    "The release-candidate platform evidence",
  );
  if (entry.platform !== platform) throw new Error();
  requireExactKeys(entry.runner, ["bytes", "sha256"], "The release-candidate runner evidence");
  const inspection = validateInspection(
    platform,
    {
      bundled: true,
      nativeTransports: entry.nativeTransports,
      runnerBytes: entry.runner.bytes,
      runnerSha256: entry.runner.sha256,
      runtime: entry.runtime,
      target: entry.target,
    },
    "release-candidate package",
  );
  const containment = validateDevelopmentContainment(platform, entry.containment);
  const inspectionDigest = protectedInspectionSha256(inspection);
  if (
    entry.inspectionSha256 !== inspectionDigest ||
    entry.containmentSha256 !== sha256(canonicalJson(containment)) ||
    (platform === "macos"
      ? entry.installedInspectionSha256 !== null
      : entry.installedInspectionSha256 !== inspectionDigest)
  ) {
    throw new Error();
  }
  return {
    containment,
    containmentSha256: entry.containmentSha256,
    inspectionSha256: entry.inspectionSha256,
    installedInspectionSha256: entry.installedInspectionSha256,
    nativeTransports: inspection.nativeTransports,
    platform,
    runner: { bytes: inspection.runnerBytes, sha256: inspection.runnerSha256 },
    runtime: inspection.runtime,
    target: inspection.target,
  };
}

/** Validates one protected or reconstructed development package inspection. */
function validateInspection(platform, inspection, kind) {
  requireExactKeys(
    inspection,
    ["bundled", "nativeTransports", "runnerBytes", "runnerSha256", "runtime", "target"],
    `The ${kind} inspection`,
  );
  if (
    inspection.bundled !== true ||
    !PLATFORM_TARGETS[platform].includes(inspection.target) ||
    !isPositiveInteger(inspection.runnerBytes) ||
    !isSha256(inspection.runnerSha256)
  ) {
    throw new Error(`The ${kind} inspection is incomplete.`);
  }
  return {
    bundled: true,
    nativeTransports: validateTransports(platform, inspection.nativeTransports, kind),
    runnerBytes: inspection.runnerBytes,
    runnerSha256: inspection.runnerSha256,
    runtime: validateRuntime(inspection.runtime),
    target: inspection.target,
  };
}

/** Requires the exact platform-relative transport paths with bounded immutable identities. */
function validateTransports(platform, transports, kind) {
  const expectedPaths = PLATFORM_TRANSPORTS[platform];
  if (!Array.isArray(transports) || transports.length !== expectedPaths.length) {
    throw new Error(`The ${kind} inspection has incomplete native transport evidence.`);
  }
  return transports.map((transport, index) => {
    requireExactKeys(transport, ["bytes", "path", "sha256"], `The ${kind} native transport evidence`);
    if (transport.path !== expectedPaths[index] || !isPositiveInteger(transport.bytes) || !isSha256(transport.sha256)) {
      throw new Error(`The ${kind} inspection has incomplete native transport evidence.`);
    }
    return { bytes: transport.bytes, path: transport.path, sha256: transport.sha256 };
  });
}

/** Validates the exact CPython/WASI runtime identity retained by package evidence. */
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
    "The protected Python runtime evidence",
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
    throw new Error("The protected Python runtime evidence is incomplete.");
  }
  return { ...runtime };
}

/** Validates the shared runtime fields recorded at the accepted-candidate root. */
function validateRuntimeCore(runtime) {
  requireExactKeys(
    runtime,
    ["licenceSha256", "pythonVersion", "pythonWasmSha256", "schemaVersion", "wasiSdkVersion"],
    "The release-candidate runtime core",
  );
  if (
    runtime.schemaVersion !== SCHEMA_VERSION ||
    runtime.pythonVersion !== "3.14.7" ||
    runtime.wasiSdkVersion !== "24" ||
    !isSha256(runtime.licenceSha256) ||
    !isSha256(runtime.pythonWasmSha256)
  ) {
    throw new Error();
  }
  return { ...runtime };
}

/** Selects the fields that must agree across platform-specific runtime layouts. */
function runtimeCoreFor(runtime) {
  return {
    schemaVersion: runtime.schemaVersion,
    licenceSha256: runtime.licenceSha256,
    pythonVersion: runtime.pythonVersion,
    pythonWasmSha256: runtime.pythonWasmSha256,
    wasiSdkVersion: runtime.wasiSdkVersion,
  };
}

/** Validates the original development containment record retained by the accepted candidate. */
function validateDevelopmentContainment(platform, containment) {
  const fields = DEVELOPMENT_CONTAINMENT_FIELDS[platform];
  requireExactKeys(containment, [...fields, "status"], "The release-candidate containment evidence");
  if (containment.status !== "ok" || !fields.every((field) => containment[field] === true)) throw new Error();
  return Object.fromEntries([...fields, "status"].sort().map((field) => [field, containment[field]]));
}

/** Validates separate native shipping evidence bound to the exact protected inspection. */
function validateShippingContainment(sourceSha, platform, target, inspectionDigest, containment) {
  const fields = SHIPPING_CONTAINMENT_FIELDS[platform];
  requireExactKeys(containment, [...CONTAINMENT_METADATA_FIELDS, ...fields], "The shipping containment evidence");
  if (containment.sourceSha !== sourceSha) {
    throw new Error("The shipping containment source revision does not match the protected package.");
  }
  if (
    containment.schemaVersion !== SCHEMA_VERSION ||
    containment.platform !== platform ||
    containment.target !== target ||
    containment.status !== "ok" ||
    !fields.every((field) => containment[field] === true)
  ) {
    throw new Error("The shipping containment evidence is incomplete.");
  }
  if (containment.inspectionSha256 !== inspectionDigest) {
    throw new Error("The shipping containment evidence does not match the protected inspection.");
  }
  return Object.fromEntries(
    [...CONTAINMENT_METADATA_FIELDS, ...fields].sort().map((field) => [field, containment[field]]),
  );
}

/** Requires an object to contain exactly one allowlisted field set. */
function requireExactKeys(value, expected, label) {
  const actual = value && typeof value === "object" && !Array.isArray(value) ? Object.keys(value).sort() : [];
  const sortedExpected = [...expected].sort();
  if (actual.length !== sortedExpected.length || actual.some((key, index) => key !== sortedExpected[index])) {
    throw new Error(`${label} is not closed.`);
  }
}

/** Requires one supported desktop platform. */
function requirePlatform(platform) {
  if (!PLATFORMS.includes(platform)) throw new Error("The protected Python platform is unsupported.");
}

/** Requires one exact lowercase Git source revision. */
function requireSourceSha(sourceSha) {
  if (typeof sourceSha !== "string" || !SOURCE_SHA_PATTERN.test(sourceSha)) {
    throw new Error("The protected Python source revision is invalid.");
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

/** Returns one lowercase SHA-256 digest for text. */
function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

/** Reads one required JSON input without reflecting its host path. */
async function readJson(path) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch {
    throw new Error("Required protected Python evidence is unavailable or malformed.");
  }
}

/** Writes one private path-free comparison document. */
async function writeJson(path, value) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
}

/** Dispatches the credential-free protected-package comparison command. */
async function main() {
  const [mode, ...arguments_] = process.argv.slice(2);
  if (mode === "--compare" && arguments_.length === 6) {
    const [sourceSha, platform, candidatePath, inspectionPath, containmentPath, outputPath] = arguments_;
    await writeJson(
      resolve(outputPath),
      bindProtectedPythonPackage(
        sourceSha,
        platform,
        await readJson(resolve(candidatePath)),
        await readJson(resolve(inspectionPath)),
        await readJson(resolve(containmentPath)),
      ),
    );
    console.log("[bottie] Credential-free protected Python package evidence accepted.");
    return;
  }
  throw new Error("Use --compare with exact inputs.");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`[bottie] ${error instanceof Error ? error.message : "Protected Python comparison failed."}`);
    process.exitCode = 1;
  });
}
