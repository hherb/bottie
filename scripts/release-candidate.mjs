#!/usr/bin/env node

/** Builds Bottie's deterministic, path-free, fail-closed beta release-candidate manifest. */

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, join, normalize, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import {
  acceptsModelTerms,
  acceptsPackagedDocuments,
  acceptsRuntimeAssets,
  summarizeDocuments,
} from "./release-candidate-runtime.mjs";

const REPOSITORY_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const RELEASE_VERSION = "0.9.0";
const RELEASE_NOTES_PATH = `RELEASES/${RELEASE_VERSION}.md`;
const DEPENDENCY_INVENTORY_PATH = "dependency-inventory.json";
const RUNTIME_ASSETS_PATH = "runtime-assets.json";
const MODEL_TERMS_EVIDENCE_PATH = "package/model-terms-evidence.json";
const MACOS_EVIDENCE_PATH = "package/macos-distribution-evidence.json";
const WINDOWS_EVIDENCE_PATH = "package/windows-package-evidence.json";
const LINUX_EVIDENCE_PATH = "package/linux-package-evidence.json";
const OUTPUT_PATH = "package/release-candidate-manifest.json";
const LICENCE_PATH = "LICENSE";
const NOTICES_PATH = "THIRD-PARTY-NOTICES.txt";
const APPLICATION_IDENTIFIER = "com.bottie.app";
const SCHEMA_VERSION = 1;
const EXPECTED_STORAGE_SCHEMA = 21;
const SHA256_PATTERN = /^[a-f0-9]{64}$/;
const SEMVER_PATTERN = /^\d+\.\d+\.\d+$/;

/** Parses the intentionally small release-notes header without a YAML dependency. */
export function parseReleaseNotes(source) {
  const title = source.match(/^# (Bottie (\d+\.\d+\.\d+) beta)$/m);
  const version = source.match(/^Version: (\d+\.\d+\.\d+)$/m)?.[1];
  const channel = source.match(/^Channel: ([a-z]+)$/m)?.[1];
  if (!title || !version || !SEMVER_PATTERN.test(version) || title[2] !== version) {
    throw new Error("Release notes must declare one matching numeric version.");
  }
  if (channel !== "beta") throw new Error("Release notes must declare the beta channel.");
  return { channel, title: title[1], version };
}

/** Builds the complete normalized gate record without retaining caller paths or identities. */
export function buildReleaseCandidateManifest(inputs) {
  const notes = parseReleaseNotes(inputs.releaseNotes);
  const dependency = inputs.dependencyInventory;
  const macos = summarizeMacos(inputs.macosDistribution);
  const windows = summarizeWindows(inputs.windowsPackage);
  const linux = summarizeLinux(inputs.linuxPackage);
  const versionsMatch =
    notes.version === inputs.version &&
    Object.values(inputs.applicationVersions ?? {}).length === 3 &&
    Object.values(inputs.applicationVersions).every((version) => version === inputs.version);
  const inventoryCurrent = hashesMatch(dependency?.inputs, inputs.currentInputHashes);
  const dependencyReviewComplete =
    dependency?.schemaVersion === SCHEMA_VERSION &&
    dependency?.summary?.unknown === 0 &&
    dependency?.summary?.["review-required"] === 0;
  const artworkCurrent = isArtworkCurrent(dependency);
  const hasDocuments = isSha256(inputs.requiredDocuments?.licence) && isSha256(inputs.requiredDocuments?.notices);
  const runtimeAssetsCurrent = acceptsRuntimeAssets(
    inputs.runtimeAssets,
    inputs.runtimeAssetSources,
    inputs.requiredDocuments,
  );
  const modelTermsAccepted = acceptsModelTerms(inputs.runtimeAssets, inputs.modelTermsAcceptance);

  const gates = [
    gate("release-notes", notes.version === inputs.version && notes.channel === "beta", "invalid-release-notes"),
    gate("version-alignment", versionsMatch, "version-mismatch"),
    gate("dependency-inventory-current", inventoryCurrent, "missing-or-stale-dependency-inventory"),
    gate("dependency-review", dependencyReviewComplete, "unresolved-dependency-review"),
    gate("licence-and-notices", hasDocuments, "missing-licence-or-notice-bundle"),
    gate("runtime-assets", runtimeAssetsCurrent, "missing-or-stale-runtime-asset-contract"),
    gate("model-terms", modelTermsAccepted, "missing-or-stale-model-terms-acceptance"),
    gate("artwork", artworkCurrent, "missing-or-stale-artwork-evidence"),
    gate(
      "macos-distribution",
      acceptsMacos(macos, inputs.version, inputs.requiredDocuments, inputs.runtimeAssets),
      "missing-unsigned-or-unnotarized-macos",
    ),
    gate(
      "windows-package",
      acceptsWindowsPackage(windows, inputs.version, inputs.requiredDocuments, inputs.runtimeAssets),
      "missing-or-stale-windows-package",
    ),
    gate("windows-distribution", acceptsWindowsDistribution(windows), "missing-or-unsigned-windows-package"),
    gate(
      "linux-package",
      acceptsLinuxPackage(linux, inputs.version, inputs.requiredDocuments, inputs.runtimeAssets),
      "missing-or-stale-linux-package",
    ),
    gate("linux-distribution", acceptsLinuxDistribution(linux), "missing-or-unsigned-linux-package"),
  ];
  return {
    schemaVersion: SCHEMA_VERSION,
    release: {
      channel: notes.channel,
      notesSha256: sha256(inputs.releaseNotes),
      tag: `v${inputs.version}`,
      title: notes.title,
      version: inputs.version,
    },
    inputs: {
      dependencyInventorySha256: dependency ? sha256(JSON.stringify(dependency)) : null,
      licenceSha256: inputs.requiredDocuments?.licence ?? null,
      noticesSha256: inputs.requiredDocuments?.notices ?? null,
      runtimeAssetsSha256: inputs.runtimeAssets?.manifestSha256 ?? null,
      modelTermsSha256: inputs.runtimeAssets?.embeddingGemma?.terms?.sha256 ?? null,
    },
    artifacts: { linux, macos, windows },
    gates,
    ready: gates.every((item) => item.passed),
  };
}

/** Returns one stable gate outcome and no raw caller-supplied diagnostic text. */
function gate(id, passed, failure) {
  return passed ? { id, passed: true } : { failure, id, passed: false };
}

/** Requires the inventory's exact input set and hashes to match the current checkout. */
function hashesMatch(expected, current) {
  if (!expected || !current) return false;
  const expectedEntries = Object.entries(expected).sort(([left], [right]) => left.localeCompare(right));
  const currentEntries = Object.entries(current).sort(([left], [right]) => left.localeCompare(right));
  return (
    expectedEntries.length > 0 &&
    JSON.stringify(expectedEntries) === JSON.stringify(currentEntries) &&
    expectedEntries.every(([, hash]) => isSha256(hash))
  );
}

/** Confirms the reviewed artwork sources and generated files are classified and input-hash bound. */
function isArtworkCurrent(inventory) {
  const artwork = inventory?.assets?.find((asset) => asset.name === "Bottie application icons and browser favicon");
  if (!artwork || artwork.classification !== "compatible") return false;
  const hashes = { ...artwork.generationSources, ...artwork.files };
  const entries = Object.entries(hashes);
  return entries.length > 0 && entries.every(([path, hash]) => inventory.inputs?.[path] === hash && isSha256(hash));
}

/** Selects only public, identity-free macOS package and distribution evidence. */
function summarizeMacos(evidence) {
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
  };
}

/** Selects only public Windows package, signature, and isolated-smoke evidence. */
function summarizeWindows(evidence) {
  if (!evidence) return null;
  return {
    schemaVersion: evidence.schemaVersion ?? null,
    version: normalizedVersion(evidence.version),
    product: evidence.bundle?.payload?.applicationDirectory === "PFiles/bottie" ? "bottie" : null,
    architecture: normalizedChoice(evidence.bundle?.payload?.architecture, ["aarch64", "x86_64"]),
    bundleDigest: shaOrNull(evidence.bundle?.payload?.bundleDigest),
    embeddedIcon: {
      height: positiveInteger(evidence.bundle?.payload?.embeddedIcon?.height),
      width: positiveInteger(evidence.bundle?.payload?.embeddedIcon?.width),
    },
    installer: summarizeInstaller(evidence.bundle?.installer),
    payloadSignature: summarizeSignature(evidence.bundle?.payload?.signature),
    requiredDocuments: summarizeDocuments(evidence.bundle?.payload?.requiredDocuments),
    smoke: summarizeSmoke(evidence.smoke),
  };
}

/** Selects only public Linux package, signature, icon, and isolated-smoke evidence. */
function summarizeLinux(evidence) {
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

/** Reduces one signature to the only two acceptable public states. */
function summarizeSignature(signature) {
  return {
    classification: normalizedChoice(signature?.classification, ["identified", "unsigned", "untrusted"]),
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
    isolatedSupportDirectory: smoke.isolatedSupportDirectory === true,
    offlineProviderConnections: positiveInteger(smoke.offlineProviderConnections),
    remainedRunning: smoke.remainedRunning === true,
    terminated: smoke.terminated === true,
  };
}

/** Validates the complete current macOS Developer ID and notarization record. */
function acceptsMacos(evidence, version, documents, runtimeAssets) {
  return Boolean(
    evidence?.schemaVersion === SCHEMA_VERSION &&
    evidence.version === version &&
    evidence.identifier === APPLICATION_IDENTIFIER &&
    evidence.architectures.length > 0 &&
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
    evidence.notarization.ticketValid,
  );
}

/** Validates the versioned Windows payload and its isolated offline smoke. */
function acceptsWindowsPackage(evidence, version, documents, runtimeAssets) {
  return Boolean(
    evidence?.schemaVersion === SCHEMA_VERSION &&
    evidence.version === version &&
    evidence.product === "bottie" &&
    evidence.architecture &&
    evidence.bundleDigest &&
    evidence.embeddedIcon.height > 0 &&
    evidence.embeddedIcon.width > 0 &&
    evidence.installer.sha256 &&
    evidence.installer.size > 0 &&
    acceptsPackagedDocuments(evidence.requiredDocuments, documents, runtimeAssets) &&
    acceptsSmoke(evidence.smoke),
  );
}

/** Requires both the Windows installer and installed executable to verify. */
function acceptsWindowsDistribution(evidence) {
  return Boolean(
    evidence?.installer.signature.classification === "identified" &&
    evidence.installer.signature.verifies &&
    evidence.payloadSignature.classification === "identified" &&
    evidence.payloadSignature.verifies,
  );
}

/** Validates the versioned Linux payload and its isolated offline smoke. */
function acceptsLinuxPackage(evidence, version, documents, runtimeAssets) {
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

/** Requires the Linux package archive to carry one verifying distribution signature. */
function acceptsLinuxDistribution(evidence) {
  return Boolean(
    evidence?.installer.signature.classification === "identified" && evidence.installer.signature.verifies,
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

/** Loads the exact repository sources and evidence files used by the command-line gate. */
function loadInputs(repositoryRoot) {
  const dependencyInventory = readJson(join(repositoryRoot, DEPENDENCY_INVENTORY_PATH));
  const runtimeAssetPath = join(repositoryRoot, RUNTIME_ASSETS_PATH);
  const runtimeAssetManifest = readOptionalJson(runtimeAssetPath);
  return {
    version: RELEASE_VERSION,
    releaseNotes: readFileSync(join(repositoryRoot, RELEASE_NOTES_PATH), "utf8"),
    applicationVersions: readApplicationVersions(repositoryRoot),
    dependencyInventory,
    currentInputHashes: currentHashes(repositoryRoot, dependencyInventory?.inputs),
    requiredDocuments: {
      licence: optionalFileSha256(join(repositoryRoot, LICENCE_PATH)),
      notices: optionalFileSha256(join(repositoryRoot, NOTICES_PATH)),
    },
    runtimeAssets: runtimeAssetManifest
      ? { ...runtimeAssetManifest, manifestSha256: optionalFileSha256(runtimeAssetPath) }
      : null,
    runtimeAssetSources: {
      modelNotice: optionalFileSha256(join(repositoryRoot, "MODEL-NOTICE.txt")),
      onnxRuntimeLicence: optionalFileSha256(join(repositoryRoot, "third-party/onnxruntime-1.28.0/LICENSE")),
      onnxRuntimeNotices: optionalFileSha256(
        join(repositoryRoot, "third-party/onnxruntime-1.28.0/ThirdPartyNotices.txt"),
      ),
    },
    modelTermsAcceptance: readOptionalJson(join(repositoryRoot, MODEL_TERMS_EVIDENCE_PATH)),
    macosDistribution: readOptionalJson(join(repositoryRoot, MACOS_EVIDENCE_PATH)),
    windowsPackage: readOptionalJson(join(repositoryRoot, WINDOWS_EVIDENCE_PATH)),
    linuxPackage: readOptionalJson(join(repositoryRoot, LINUX_EVIDENCE_PATH)),
  };
}

/** Reads the three independent application-version declarations. */
function readApplicationVersions(repositoryRoot) {
  const npm = readJson(join(repositoryRoot, "package.json")).version;
  const tauri = readJson(join(repositoryRoot, "src-tauri/tauri.conf.json")).version;
  const cargoSource = readFileSync(join(repositoryRoot, "src-tauri/Cargo.toml"), "utf8");
  const cargo = cargoSource.match(/^version = "([^"]+)"$/m)?.[1] ?? null;
  return { cargo, npm, tauri };
}

/** Hashes the inventory's repository-relative inputs and rejects traversal or absolute paths. */
function currentHashes(repositoryRoot, inputs) {
  if (!inputs || typeof inputs !== "object") return null;
  const result = {};
  for (const path of Object.keys(inputs).sort()) {
    const normalizedPath = normalize(path);
    if (
      isAbsolute(path) ||
      normalizedPath === ".." ||
      normalizedPath.startsWith(`..${sep}`) ||
      relative(repositoryRoot, resolve(repositoryRoot, normalizedPath)).startsWith("..")
    ) {
      return null;
    }
    const absolutePath = join(repositoryRoot, normalizedPath);
    if (!existsSync(absolutePath)) return null;
    result[path] = fileSha256(absolutePath);
  }
  return result;
}

/** Reads required JSON and rejects malformed source files. */
function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

/** Reads optional evidence while treating absence or malformed JSON as a failed gate. */
function readOptionalJson(path) {
  if (!existsSync(path)) return null;
  try {
    return readJson(path);
  } catch {
    return null;
  }
}

/** Returns a file hash or null when a required distribution document is absent. */
function optionalFileSha256(path) {
  return existsSync(path) ? fileSha256(path) : null;
}

/** Returns one lowercase SHA-256 digest for bytes or text. */
function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

/** Returns one lowercase SHA-256 digest for a repository file. */
function fileSha256(path) {
  return sha256(readFileSync(path));
}

/** Returns a valid SHA-256 or null. */
function shaOrNull(value) {
  return isSha256(value) ? value : null;
}

/** Checks one lowercase SHA-256 value. */
function isSha256(value) {
  return typeof value === "string" && SHA256_PATTERN.test(value);
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

/** Writes the normalized manifest before returning a failing process status for open gates. */
function main() {
  const arguments_ = process.argv.slice(2);
  if (arguments_.length !== 1 || arguments_[0] !== "--write") throw new Error("Use the exact --write mode.");
  const manifest = buildReleaseCandidateManifest(loadInputs(REPOSITORY_ROOT));
  const outputPath = join(REPOSITORY_ROOT, OUTPUT_PATH);
  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`, { mode: 0o600 });
  if (!manifest.ready) {
    const failures = manifest.gates.filter((gate_) => !gate_.passed).map((gate_) => gate_.id);
    throw new Error(`Release candidate blocked by: ${failures.join(", ")}.`);
  }
  console.log(`[bottie] ${manifest.release.tag} release-candidate gates passed.`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(`[bottie] ${error instanceof Error ? error.message : "Release-candidate validation failed."}`);
    process.exitCode = 1;
  }
}
