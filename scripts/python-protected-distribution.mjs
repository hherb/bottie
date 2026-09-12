/** Closed validation for normalized outer-distribution evidence bound to protected Python packages. */

import { summarizeLinux, summarizeMacos, summarizeWindows } from "./release-candidate-distributions.mjs";

const SCHEMA_VERSION = 1;
const SHA256_PATTERN = /^[a-f0-9]{64}$/;
const SEMVER_PATTERN = /^\d+\.\d+\.\d+$/;
const DOCUMENTS = { licence: "sha256", modelNotice: "sha256", thirdPartyNotices: "sha256" };
const SIGNATURE = { classification: "string", timestamped: "boolean", verifies: "true" };
const UPDATER = {
  artifact: { sha256: "sha256", size: "positiveInteger" },
  publicKeySha256: "sha256",
  schemaVersion: "schemaVersion",
  signature: { format: "string", sha256: "sha256", verifies: "true" },
  target: "string",
};
const SMOKE = {
  database: {
    conversationCount: "nonNegativeInteger",
    migrationCount: "positiveInteger",
    profileCount: "positiveInteger",
    quickCheck: "string",
    schemaVersion: "positiveInteger",
  },
  isolatedSupportDirectory: "true",
  offlineProviderConnections: "positiveInteger",
  remainedRunning: "true",
  terminated: "true",
};
const DISTRIBUTION_SHAPES = {
  linux: {
    schemaVersion: "schemaVersion",
    version: "version",
    package: "string",
    packageArchitecture: "string",
    payloadArchitecture: "string",
    bundleDigest: "sha256",
    installedIconCount: "positiveInteger",
    installer: { sha256: "sha256", signature: SIGNATURE, size: "positiveInteger" },
    requiredDocuments: DOCUMENTS,
    smoke: SMOKE,
    updater: UPDATER,
  },
  macos: {
    schemaVersion: "schemaVersion",
    version: "version",
    identifier: "string",
    architectures: ["string"],
    bundleDigest: "sha256",
    requiredEntries: {
      executable: "true",
      icon: "true",
      infoPlist: "true",
      licence: "true",
      modelNotice: "true",
      thirdPartyNotices: "true",
    },
    requiredDocuments: DOCUMENTS,
    signing: {
      classification: "string",
      hardenedRuntime: "true",
      secureTimestamp: "true",
      verifies: "true",
    },
    notarization: {
      gatekeeperAccepted: "true",
      gatekeeperSource: "string",
      submissionAccepted: "true",
      submissionStatus: "string",
      ticketStapled: "true",
      ticketValid: "true",
    },
    updater: UPDATER,
  },
  windows: {
    schemaVersion: "schemaVersion",
    version: "version",
    installer: { sha256: "sha256", signature: SIGNATURE, size: "positiveInteger" },
    payload: {
      architecture: "string",
      bundleDigest: "sha256",
      requiredDocuments: DOCUMENTS,
      signature: SIGNATURE,
    },
    smoke: SMOKE,
    updater: UPDATER,
  },
};
const SUMMARIZERS = { linux: summarizeLinux, macos: summarizeMacos, windows: summarizeWindows };

/** Normalizes raw final distribution evidence and validates the resulting closed public contract. */
export function normalizeProtectedPythonOuterDistribution(platform, evidence) {
  if (!SUMMARIZERS[platform]) throw new Error("The protected Python outer distribution platform is invalid.");
  return validateProtectedPythonOuterDistribution(platform, SUMMARIZERS[platform](evidence));
}

/** Revalidates one normalized outer-distribution summary from an untrusted envelope. */
export function validateProtectedPythonOuterDistribution(platform, evidence) {
  const shape = DISTRIBUTION_SHAPES[platform];
  if (!shape) throw new Error("The protected Python outer distribution platform is invalid.");
  requireShape(evidence, shape);
  if (!hasPlatformSemantics(platform, evidence)) {
    throw new Error("The protected Python outer distribution is incomplete.");
  }
  return evidence;
}

/** Requires exact nested keys and value types for one normalized distribution shape. */
function requireShape(value, shape) {
  if (Array.isArray(shape)) {
    if (!Array.isArray(value) || value.length === 0) failClosed();
    value.forEach((item) => requireShape(item, shape[0]));
    return;
  }
  if (shape && typeof shape === "object") {
    const actual = value && typeof value === "object" && !Array.isArray(value) ? Object.keys(value).sort() : [];
    const expected = Object.keys(shape).sort();
    if (actual.length !== expected.length || actual.some((field, index) => field !== expected[index])) failClosed();
    expected.forEach((field) => requireShape(value[field], shape[field]));
    return;
  }
  if (!matchesType(value, shape)) failClosed();
}

/** Checks one leaf against its exact normalized evidence type. */
function matchesType(value, type) {
  if (type === "boolean") return typeof value === "boolean";
  if (type === "true") return value === true;
  if (type === "positiveInteger") return Number.isSafeInteger(value) && value > 0;
  if (type === "nonNegativeInteger") return Number.isSafeInteger(value) && value >= 0;
  if (type === "schemaVersion") return value === SCHEMA_VERSION;
  if (type === "sha256") return typeof value === "string" && SHA256_PATTERN.test(value);
  if (type === "version") return typeof value === "string" && SEMVER_PATTERN.test(value);
  return type === "string" && typeof value === "string" && value.length > 0;
}

/** Requires normalized choices and updater target to agree with the envelope platform. */
function hasPlatformSemantics(platform, evidence) {
  if (evidence.updater.signature.format !== "minisign") return false;
  if (platform === "macos") {
    const architecture = evidence.architectures[0];
    return (
      evidence.architectures.length === 1 &&
      ["arm64", "x86_64"].includes(architecture) &&
      evidence.identifier === "com.bottie.app" &&
      evidence.signing.classification === "developer-id-application" &&
      evidence.notarization.gatekeeperSource === "notarized-developer-id" &&
      evidence.notarization.submissionStatus === "accepted" &&
      evidence.updater.target === (architecture === "arm64" ? "darwin-aarch64" : "darwin-x86_64")
    );
  }
  if (platform === "windows") {
    return (
      evidence.payload.architecture === "x86_64" &&
      evidence.installer.signature.classification === "identified" &&
      evidence.installer.signature.timestamped &&
      evidence.payload.signature.classification === "identified" &&
      evidence.payload.signature.timestamped &&
      evidence.smoke.database.quickCheck === "ok" &&
      evidence.updater.target === "windows-x86_64"
    );
  }
  return (
    evidence.package === "bottie" &&
    ["amd64", "arm64"].includes(evidence.packageArchitecture) &&
    ["aarch64", "x86_64"].includes(evidence.payloadArchitecture) &&
    evidence.installer.signature.classification === "identified" &&
    evidence.smoke.database.quickCheck === "ok" &&
    evidence.updater.target === "linux-x86_64"
  );
}

/** Throws the stable closed-schema error for malformed normalized evidence. */
function failClosed() {
  throw new Error("The protected Python outer distribution is not closed and complete.");
}
