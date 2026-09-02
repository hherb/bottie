#!/usr/bin/env node

/** Builds Bottie's deterministic, path-free, fail-closed beta release-candidate manifest. */

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, join, normalize, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import {
  acceptsLinuxDistribution,
  acceptsLinuxPackage,
  acceptsMacos,
  acceptsWindowsDistribution,
  acceptsWindowsPackage,
  summarizeLinux,
  summarizeMacos,
  summarizeWindows,
} from "./release-candidate-distributions.mjs";
import { acceptsModelTerms, acceptsRuntimeAssets } from "./release-candidate-runtime.mjs";

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
const SCHEMA_VERSION = 1;
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
  const windows = summarizeWindows(inputs.windowsDistribution);
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
  const windowsPackageAccepted = acceptsWindowsPackage(
    windows,
    inputs.version,
    inputs.requiredDocuments,
    inputs.runtimeAssets,
  );
  const gates = [
    gate("release-notes", notes.version === inputs.version && notes.channel === "beta", "invalid-release-notes"),
    gate("version-alignment", versionsMatch, "version-mismatch"),
    gate("dependency-inventory-current", inventoryCurrent, "missing-or-stale-dependency-inventory"),
    gate("dependency-review", dependencyReviewComplete, "unresolved-dependency-review"),
    gate("licence-and-notices", hasDocuments, "missing-licence-or-notice-bundle"),
    gate("runtime-assets", runtimeAssetsCurrent, "missing-or-stale-runtime-asset-contract"),
    gate(
      "model-terms",
      acceptsModelTerms(inputs.runtimeAssets, inputs.modelTermsAcceptance),
      "missing-or-stale-model-terms-acceptance",
    ),
    gate("artwork", artworkCurrent, "missing-or-stale-artwork-evidence"),
    gate(
      "macos-distribution",
      acceptsMacos(macos, inputs.version, inputs.requiredDocuments, inputs.runtimeAssets),
      "missing-unsigned-or-unnotarized-macos",
    ),
    gate("windows-package", windowsPackageAccepted, "missing-or-stale-windows-package"),
    gate(
      "windows-distribution",
      windowsPackageAccepted && acceptsWindowsDistribution(windows),
      "missing-or-unsigned-windows-package",
    ),
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
  const entries = Object.entries({ ...artwork.generationSources, ...artwork.files });
  return entries.length > 0 && entries.every(([path, hash]) => inventory.inputs?.[path] === hash && isSha256(hash));
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
    windowsDistribution: readOptionalJson(join(repositoryRoot, WINDOWS_EVIDENCE_PATH)),
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

/** Hashes inventory inputs and rejects traversal or absolute paths. */
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

/** Reads required JSON. */
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

/** Checks one lowercase SHA-256 value. */
function isSha256(value) {
  return typeof value === "string" && SHA256_PATTERN.test(value);
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
    const failures = manifest.gates.filter((item) => !item.passed).map((item) => item.id);
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
