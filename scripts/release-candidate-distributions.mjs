/** Pure path-free normalization and gates for protected desktop distribution evidence. */

import { acceptsPackagedDocuments, summarizeDocuments } from "./release-candidate-runtime.mjs";

const APPLICATION_IDENTIFIER = "com.bottie.app";
const EXPECTED_STORAGE_SCHEMA = 21;
const SCHEMA_VERSION = 1;
const SHA256_PATTERN = /^[a-f0-9]{64}$/;
const SEMVER_PATTERN = /^\d+\.\d+\.\d+$/;

/** Selects only public, identity-free macOS package and distribution evidence. */
export function summarizeMacos(evidence) {
  if (!evidence) return null;
  return {
    schemaVersion: evidence.schemaVersion ?? null,
    version: normalizedVersion(evidence.metadata?.version),
    identifier: normalizedChoice(evidence.metadata?.identifier, [APPLICATION_IDENTIFIER]),
    architectures: allowedStrings(evidence.metadata?.architectures, ["arm64", "x86_64"]),
    bundleDigest: shaOrNull(evidence.artifact?.bundleDigest),
    requiredEntries: {
      executable: evidence.artifact?.requiredEntries?.executable === true,
      icon: evidence.artifact?.requiredEntries?.icon === true,
      infoPlist: evidence.artifact?.requiredEntries?.infoPlist === true,
      licence: evidence.artifact?.requiredEntries?.licence === true,
      modelNotice: evidence.artifact?.requiredEntries?.modelNotice === true,
      thirdPartyNotices: evidence.artifact?.requiredEntries?.thirdPartyNotices === true,
    },
    requiredDocuments: summarizeDocuments(evidence.artifact?.requiredDocuments),
    signing: {
      classification: normalizedChoice(evidence.signing?.classification, ["developer-id-application"]),
      hardenedRuntime: evidence.signing?.hardenedRuntime === true,
      secureTimestamp: evidence.signing?.secureTimestamp === true,
      verifies: evidence.signing?.verifies === true,
    },
    notarization: {
      gatekeeperAccepted: evidence.notarization?.gatekeeper?.accepted === true,
      gatekeeperSource: normalizedChoice(evidence.notarization?.gatekeeper?.source, ["notarized-developer-id"]),
      submissionAccepted: evidence.notarization?.submission?.accepted === true,
      submissionStatus: normalizedChoice(evidence.notarization?.submission?.status, ["accepted"]),
      ticketStapled: evidence.notarization?.ticketStapled === true,
      ticketValid: evidence.notarization?.ticketValid === true,
    },
    updater: summarizeUpdater(evidence.updater),
  };
}

/** Selects only public direct-distribution Windows package, smoke, and updater evidence. */
export function summarizeWindows(evidence) {
  if (!evidence) return null;
  return {
    schemaVersion: evidence.schemaVersion ?? null,
    version: normalizedVersion(evidence.version),
    installer: summarizeInstaller(evidence.bundle?.installer),
    payload: {
      architecture: normalizedChoice(evidence.bundle?.payload?.architecture, ["x86_64"]),
      bundleDigest: shaOrNull(evidence.bundle?.payload?.bundleDigest),
      requiredDocuments: summarizeDocuments(evidence.bundle?.payload?.requiredDocuments),
      signature: summarizeSignature(evidence.bundle?.payload?.signature),
    },
    smoke: summarizeSmoke(evidence.smoke),
    updater: summarizeUpdater(evidence.updater),
  };
}

/** Selects only public Linux package, signature, icon, smoke, and updater evidence. */
export function summarizeLinux(evidence) {
  if (!evidence) return null;
  return {
    schemaVersion: evidence.schemaVersion ?? null,
    version: normalizedVersion(evidence.version ?? evidence.bundle?.installer?.metadata?.version),
    package: normalizedChoice(evidence.bundle?.installer?.metadata?.package, ["bottie"]),
    packageArchitecture: normalizedChoice(evidence.bundle?.installer?.metadata?.architecture, ["amd64", "arm64"]),
    payloadArchitecture: normalizedChoice(evidence.bundle?.payload?.architecture, ["aarch64", "x86_64"]),
    bundleDigest: shaOrNull(evidence.bundle?.payload?.bundleDigest),
    installedIconCount: stringArray(evidence.bundle?.payload?.installedIcons).length,
    installer: summarizeInstaller(evidence.bundle?.installer),
    requiredDocuments: summarizeDocuments(evidence.bundle?.payload?.requiredDocuments),
    smoke: summarizeSmoke(evidence.smoke),
    updater: summarizeUpdater(evidence.updater),
  };
}

/** Validates the complete current macOS Developer ID, notarization, and updater record. */
export function acceptsMacos(evidence, version, documents, runtimeAssets) {
  return Boolean(
    evidence?.schemaVersion === SCHEMA_VERSION &&
    evidence.version === version &&
    evidence.identifier === APPLICATION_IDENTIFIER &&
    evidence.architectures.length === 1 &&
    evidence.bundleDigest &&
    Object.values(evidence.requiredEntries).every(Boolean) &&
    acceptsPackagedDocuments(evidence.requiredDocuments, documents, runtimeAssets) &&
    evidence.signing.classification === "developer-id-application" &&
    evidence.signing.hardenedRuntime &&
    evidence.signing.secureTimestamp &&
    evidence.signing.verifies &&
    evidence.notarization.gatekeeperAccepted &&
    evidence.notarization.gatekeeperSource === "notarized-developer-id" &&
    evidence.notarization.submissionAccepted &&
    evidence.notarization.submissionStatus === "accepted" &&
    evidence.notarization.ticketStapled &&
    evidence.notarization.ticketValid &&
    acceptsUpdater(evidence.updater, evidence.architectures[0] === "arm64" ? "darwin-aarch64" : "darwin-x86_64"),
  );
}

/** Validates the exact direct Windows package payload and isolated smoke. */
export function acceptsWindowsPackage(evidence, version, documents, runtimeAssets) {
  return Boolean(
    evidence?.schemaVersion === SCHEMA_VERSION &&
    evidence.version === version &&
    evidence.installer.sha256 &&
    evidence.installer.size > 0 &&
    evidence.payload.architecture === "x86_64" &&
    evidence.payload.bundleDigest &&
    acceptsPackagedDocuments(evidence.payload.requiredDocuments, documents, runtimeAssets) &&
    acceptsSmoke(evidence.smoke),
  );
}

/** Requires Authenticode on final MSI and executable plus matching updater verification. */
export function acceptsWindowsDistribution(evidence) {
  return Boolean(
    evidence?.installer.signature.classification === "identified" &&
    evidence.installer.signature.timestamped &&
    evidence.installer.signature.verifies &&
    evidence.payload.signature.classification === "identified" &&
    evidence.payload.signature.timestamped &&
    evidence.payload.signature.verifies &&
    acceptsUpdater(evidence.updater, "windows-x86_64", evidence.installer.sha256),
  );
}

/** Validates the versioned Linux payload and its isolated offline smoke. */
export function acceptsLinuxPackage(evidence, version, documents, runtimeAssets) {
  return Boolean(
    evidence?.schemaVersion === SCHEMA_VERSION &&
    evidence.version === version &&
    evidence.package === "bottie" &&
    evidence.packageArchitecture &&
    evidence.payloadArchitecture &&
    evidence.bundleDigest &&
    evidence.installedIconCount >= 4 &&
    evidence.installer.sha256 &&
    evidence.installer.size > 0 &&
    acceptsPackagedDocuments(evidence.requiredDocuments, documents, runtimeAssets) &&
    acceptsSmoke(evidence.smoke),
  );
}

/** Requires embedded OpenPGP and matching updater verification on the exact DEB. */
export function acceptsLinuxDistribution(evidence) {
  return Boolean(
    evidence?.installer.signature.classification === "identified" &&
    evidence.installer.signature.verifies &&
    acceptsUpdater(evidence.updater, "linux-x86_64", evidence.installer.sha256),
  );
}

/** Reduces verified updater evidence to exact path-free cryptographic bindings. */
function summarizeUpdater(evidence) {
  if (!evidence) return null;
  return {
    artifact: { sha256: shaOrNull(evidence.artifact?.sha256), size: positiveInteger(evidence.artifact?.size) },
    publicKeySha256: shaOrNull(evidence.publicKeySha256),
    schemaVersion: evidence.schemaVersion ?? null,
    signature: {
      format: normalizedChoice(evidence.signature?.format, ["minisign"]),
      sha256: shaOrNull(evidence.signature?.sha256),
      verifies: evidence.signature?.verifies === true,
    },
    target: normalizedChoice(evidence.target, ["darwin-aarch64", "darwin-x86_64", "linux-x86_64", "windows-x86_64"]),
  };
}

/** Normalizes one installer without carrying its host filename or path. */
function summarizeInstaller(installer) {
  return {
    sha256: shaOrNull(installer?.sha256),
    signature: summarizeSignature(installer?.signature),
    size: positiveInteger(installer?.size),
  };
}

/** Reduces one signature to the only acceptable public fields. */
function summarizeSignature(signature) {
  return {
    classification: normalizedChoice(signature?.classification, ["identified", "unsigned", "untrusted"]),
    timestamped: signature?.timestamped === true,
    verifies: signature?.verifies === true,
  };
}

/** Normalizes the exact path-free smoke contract used by Windows and Linux. */
function summarizeSmoke(smoke) {
  if (!smoke) return null;
  return {
    database: {
      conversationCount: nonNegativeInteger(smoke.database?.conversationCount),
      migrationCount: nonNegativeInteger(smoke.database?.migrationCount),
      profileCount: nonNegativeInteger(smoke.database?.profileCount),
      quickCheck: normalizedChoice(smoke.database?.quickCheck, ["ok"]),
      schemaVersion: nonNegativeInteger(smoke.database?.schemaVersion),
    },
    isolatedSupportDirectory: smoke.isolatedSupportDirectory === true || smoke.isolatedXdgDirectories === true,
    offlineProviderConnections: positiveInteger(smoke.offlineProviderConnections),
    remainedRunning: smoke.remainedRunning === true,
    terminated: smoke.terminated === true,
  };
}

/** Requires one exact verified updater record and optional final distribution-byte equality. */
function acceptsUpdater(evidence, target, artifactSha256) {
  return Boolean(
    evidence?.schemaVersion === SCHEMA_VERSION &&
    evidence.target === target &&
    evidence.artifact.size > 0 &&
    isSha256(evidence.artifact.sha256) &&
    (artifactSha256 === undefined || evidence.artifact.sha256 === artifactSha256) &&
    isSha256(evidence.publicKeySha256) &&
    evidence.signature.format === "minisign" &&
    isSha256(evidence.signature.sha256) &&
    evidence.signature.verifies,
  );
}

/** Confirms the disposable package opened one healthy current-schema profile and stopped cleanly. */
function acceptsSmoke(smoke) {
  return Boolean(
    smoke?.database.conversationCount === 0 &&
    smoke.database.migrationCount === EXPECTED_STORAGE_SCHEMA &&
    smoke.database.profileCount === 1 &&
    smoke.database.quickCheck === "ok" &&
    smoke.database.schemaVersion === EXPECTED_STORAGE_SCHEMA &&
    smoke.isolatedSupportDirectory &&
    smoke.offlineProviderConnections > 0 &&
    smoke.remainedRunning &&
    smoke.terminated,
  );
}

/** Returns only non-empty string array entries. */
function stringArray(value) {
  return Array.isArray(value) ? value.filter((item) => typeof item === "string" && item.length > 0) : [];
}

/** Returns only allowlisted strings from an evidence array. */
function allowedStrings(value, allowed) {
  return stringArray(value).filter((item) => allowed.includes(item));
}

/** Returns one numeric application version or null. */
function normalizedVersion(value) {
  return typeof value === "string" && SEMVER_PATTERN.test(value) ? value : null;
}

/** Returns one allowlisted public state or null. */
function normalizedChoice(value, allowed) {
  return allowed.includes(value) ? value : null;
}

/** Returns one positive integer or zero. */
function positiveInteger(value) {
  return Number.isInteger(value) && value > 0 ? value : 0;
}

/** Returns one non-negative integer or null. */
function nonNegativeInteger(value) {
  return Number.isInteger(value) && value >= 0 ? value : null;
}

/** Returns a valid SHA-256 or null. */
function shaOrNull(value) {
  return isSha256(value) ? value : null;
}

/** Checks one lowercase SHA-256 value. */
function isSha256(value) {
  return typeof value === "string" && SHA256_PATTERN.test(value);
}
