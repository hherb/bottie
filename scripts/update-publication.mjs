#!/usr/bin/env node

/** Prepares and verifies Bottie's protected three-platform GitHub updater publication. */

import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { buildStaticUpdateManifest, buildUpdateDeliveryEvidence } from "./update-delivery.mjs";

const REPOSITORY_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const ARTIFACT_DIRECTORY = "package/updater-artifacts";
const RELEASE_CANDIDATE_PATH = "package/release-candidate-manifest.json";
const MANIFEST_PATH = "package/latest.json";
const DELIVERY_EVIDENCE_PATH = "package/update-delivery-evidence.json";
const ASSET_EVIDENCE_PATH = "package/update-publication-assets.json";
const GITHUB_RELEASE_PATH = "package/github-release.json";
const GITHUB_LATEST_RELEASE_PATH = "package/github-latest-release.json";
const PUBLIC_KEY_PATH = "distribution/update/bottie-updater.pub";
const SHA256_PATTERN = /^[a-f0-9]{64}$/;
const VERSION_PATTERN = /^\d+\.\d+\.\d+$/;
const SOURCE_SHA_PATTERN = /^[a-f0-9]{40}$/;
const REQUIRED_STATIC_TARGETS = ["linux-x86_64", "windows-x86_64"];
const MACOS_TARGETS = ["darwin-aarch64", "darwin-x86_64"];
const TARGET_SUFFIXES = {
  "darwin-aarch64": ".app.tar.gz",
  "darwin-x86_64": ".app.tar.gz",
  "linux-x86_64": ".deb",
  "windows-x86_64": ".msi",
};

/** Returns the canonical artifact and signature names for one three-platform release. */
export function expectedPublicationAssets(version, macosTarget = "darwin-aarch64") {
  requireVersion(version);
  if (!MACOS_TARGETS.includes(macosTarget)) throw new Error("Updater publication macOS target is unsupported.");
  return [macosTarget, ...REQUIRED_STATIC_TARGETS].sort().flatMap((target) => {
    const artifact = `bottie_${version}_${target}${TARGET_SUFFIXES[target]}`;
    return [
      { kind: "artifact", name: artifact, target },
      { kind: "signature", name: `${artifact}.sig`, target },
    ];
  });
}

/** Builds a signed static manifest only when final bytes match protected platform evidence. */
export async function buildUpdatePublication({
  artifactDirectory,
  distributionEvidence,
  notes,
  publicKey,
  publishedAt,
  version,
}) {
  const macosTarget = selectMacosTarget(distributionEvidence);
  const expectedAssets = expectedPublicationAssets(version, macosTarget);
  const directoryEntries = (await readdir(artifactDirectory)).sort();
  const expectedNames = expectedAssets.map((asset) => asset.name).sort();
  if (JSON.stringify(directoryEntries) !== JSON.stringify(expectedNames)) {
    throw new Error("Updater publication requires exactly the canonical three-platform asset set.");
  }
  const artifacts = [];
  const releaseAssets = {};
  for (const asset of expectedAssets.filter((item) => item.kind === "artifact")) {
    const artifactBytes = await readFile(join(artifactDirectory, asset.name));
    const signatureName = `${asset.name}.sig`;
    const signatureBytes = await readFile(join(artifactDirectory, signatureName));
    const protectedEvidence = distributionEvidence[asset.target];
    requireProtectedEvidence(protectedEvidence, asset.target, artifactBytes, signatureBytes, publicKey);
    artifacts.push({
      artifactSha256: sha256(artifactBytes),
      signature: signatureBytes.toString("utf8").trim(),
      target: asset.target,
      url: `https://github.com/hherb/bottie/releases/download/v${version}/${asset.name}`,
    });
    releaseAssets[asset.name] = fileEvidence(artifactBytes);
    releaseAssets[signatureName] = fileEvidence(signatureBytes);
  }
  const manifest = buildStaticUpdateManifest({ artifacts, notes, publishedAt, version });
  const manifestBytes = Buffer.from(`${JSON.stringify(manifest)}\n`);
  releaseAssets["latest.json"] = fileEvidence(manifestBytes);
  return {
    evidence: buildUpdateDeliveryEvidence({ artifacts, manifest, publicKey, status: "draft" }),
    manifest,
    releaseAssets,
  };
}

/** Accepts only one full release bound to current main and the exact uploaded asset bytes. */
export function verifyGitHubRelease({ expectedAssets, latestRelease, release, sourceSha, version }) {
  requireSourceSha(sourceSha);
  requireVersion(version);
  verifyReleaseShape(release, expectedAssets, sourceSha, version, false);
  if (
    latestRelease?.draft !== false ||
    latestRelease.prerelease !== false ||
    latestRelease.tag_name !== release.tag_name
  ) {
    throw new Error("The updater release is not GitHub's latest full release.");
  }
  return true;
}

/** Accepts one complete draft before the irreversible publication transition. */
export function verifyGitHubDraftRelease({ expectedAssets, release, sourceSha, version }) {
  requireSourceSha(sourceSha);
  requireVersion(version);
  verifyReleaseShape(release, expectedAssets, sourceSha, version, true);
  return true;
}

/** Requires one exact release state and one exact set of uploaded asset hashes and sizes. */
function verifyReleaseShape(release, expectedAssets, sourceSha, version, draft) {
  if (
    release?.draft !== draft ||
    release.prerelease !== false ||
    release.target_commitish !== sourceSha ||
    release.tag_name !== `v${version}`
  ) {
    throw new Error(draft ? "GitHub draft release is invalid." : "GitHub release is not an exact full release.");
  }
  const actual = Object.fromEntries(
    (release.assets ?? []).map((asset) => [asset.name, { digest: asset.digest, size: asset.size, state: asset.state }]),
  );
  const expectedNames = Object.keys(expectedAssets).sort();
  if (JSON.stringify(Object.keys(actual).sort()) !== JSON.stringify(expectedNames)) {
    throw new Error("GitHub release assets do not match the reviewed publication set.");
  }
  for (const name of expectedNames) {
    const expected = expectedAssets[name];
    const digest = expected.digest ?? `sha256:${expected.sha256}`;
    if (actual[name].state !== "uploaded" || actual[name].size !== expected.size || actual[name].digest !== digest) {
      throw new Error("GitHub release asset bytes do not match the reviewed publication set.");
    }
  }
}

/** Selects exactly one protected macOS architecture beside the fixed Linux and Windows targets. */
function selectMacosTarget(distributionEvidence) {
  if (!distributionEvidence || typeof distributionEvidence !== "object") {
    throw new Error("Protected updater distribution evidence is unavailable.");
  }
  for (const target of REQUIRED_STATIC_TARGETS) {
    if (!distributionEvidence[target]) throw new Error("Protected updater distribution evidence is incomplete.");
  }
  const selected = MACOS_TARGETS.filter((target) => distributionEvidence[target]);
  if (selected.length !== 1) throw new Error("Protected updater evidence must contain exactly one macOS target.");
  const allowed = new Set([...REQUIRED_STATIC_TARGETS, selected[0]]);
  if (Object.keys(distributionEvidence).some((target) => !allowed.has(target))) {
    throw new Error("Protected updater distribution evidence contains an unsupported target.");
  }
  return selected[0];
}

/** Binds one exported artifact and signature to the public protected verifier result. */
function requireProtectedEvidence(evidence, target, artifactBytes, signatureBytes, publicKey) {
  if (
    evidence?.schemaVersion !== 1 ||
    evidence.target !== target ||
    evidence.artifact?.sha256 !== sha256(artifactBytes) ||
    evidence.artifact?.size !== artifactBytes.length ||
    evidence.signature?.format !== "minisign" ||
    evidence.signature.sha256 !== sha256(signatureBytes) ||
    evidence.signature.verifies !== true ||
    evidence.publicKeySha256 !== sha256(publicKey)
  ) {
    throw new Error("Updater publication bytes do not match protected evidence.");
  }
}

/** Returns the only retained release-asset fields used by GitHub verification. */
function fileEvidence(bytes) {
  return { sha256: sha256(bytes), size: bytes.length };
}

/** Loads current protected evidence and canonical release inputs from the fixed ignored boundary. */
async function loadRepositoryPublication(repositoryRoot, publishedAt) {
  const packageMetadata = await readJson(join(repositoryRoot, "package.json"));
  const version = packageMetadata.version;
  const candidate = await readJson(join(repositoryRoot, RELEASE_CANDIDATE_PATH));
  if (candidate?.ready !== true || candidate.release?.version !== version) {
    throw new Error("The current-source release-candidate manifest is not ready.");
  }
  const macos = await readJson(join(repositoryRoot, "package", "macos-distribution-evidence.json"));
  const windows = await readJson(join(repositoryRoot, "package", "windows-package-evidence.json"));
  const linux = await readJson(join(repositoryRoot, "package", "linux-package-evidence.json"));
  const distributionEvidence = Object.fromEntries(
    [macos?.updater, windows?.updater, linux?.updater].filter(Boolean).map((evidence) => [evidence.target, evidence]),
  );
  return buildUpdatePublication({
    artifactDirectory: join(repositoryRoot, ARTIFACT_DIRECTORY),
    distributionEvidence,
    notes: await readFile(join(repositoryRoot, `RELEASES/${version}.md`), "utf8"),
    publicKey: await readFile(join(repositoryRoot, PUBLIC_KEY_PATH), "utf8"),
    publishedAt,
    version,
  });
}

/** Writes deterministic manifest plus path-free draft and asset evidence. */
async function preparePublication(repositoryRoot) {
  const publishedAt = process.env.BOTTIE_UPDATE_PUBLISHED_AT;
  if (!publishedAt) throw new Error("The updater publication time is unavailable.");
  const publication = await loadRepositoryPublication(repositoryRoot, publishedAt);
  await writeJson(join(repositoryRoot, MANIFEST_PATH), publication.manifest);
  await writeJson(join(repositoryRoot, DELIVERY_EVIDENCE_PATH), publication.evidence);
  await writeJson(join(repositoryRoot, ASSET_EVIDENCE_PATH), publication.releaseAssets);
}

/** Verifies the uploaded draft before changing its public state. */
async function verifyDraft(repositoryRoot) {
  const version = (await readJson(join(repositoryRoot, "package.json"))).version;
  const expectedAssets = await readJson(join(repositoryRoot, ASSET_EVIDENCE_PATH));
  const release = await readJson(join(repositoryRoot, GITHUB_RELEASE_PATH));
  verifyGitHubDraftRelease({ expectedAssets, release, sourceSha: process.env.GITHUB_SHA, version });
}

/** Verifies live latest resolution and upgrades retained delivery evidence to published. */
async function verifyPublished(repositoryRoot) {
  const version = (await readJson(join(repositoryRoot, "package.json"))).version;
  const expectedAssets = await readJson(join(repositoryRoot, ASSET_EVIDENCE_PATH));
  const release = await readJson(join(repositoryRoot, GITHUB_RELEASE_PATH));
  const latestRelease = await readJson(join(repositoryRoot, GITHUB_LATEST_RELEASE_PATH));
  verifyGitHubRelease({ expectedAssets, latestRelease, release, sourceSha: process.env.GITHUB_SHA, version });
  const draftEvidence = await readJson(join(repositoryRoot, DELIVERY_EVIDENCE_PATH));
  await writeJson(join(repositoryRoot, DELIVERY_EVIDENCE_PATH), { ...draftEvidence, status: "published" });
}

/** Reads one required JSON file with fixed path-free errors. */
async function readJson(path) {
  if (!existsSync(path)) throw new Error(`Required publication input ${basename(path)} is unavailable.`);
  return JSON.parse(await readFile(path, "utf8"));
}

/** Writes one owner-readable generated JSON record. */
async function writeJson(path, value) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
}

/** Requires Bottie's numeric Tauri-compatible release version. */
function requireVersion(version) {
  if (typeof version !== "string" || !VERSION_PATTERN.test(version)) {
    throw new Error("Updater publication version must be numeric SemVer.");
  }
}

/** Requires one exact Git source commit. */
function requireSourceSha(sourceSha) {
  if (typeof sourceSha !== "string" || !SOURCE_SHA_PATTERN.test(sourceSha)) {
    throw new Error("Updater publication source commit is invalid.");
  }
}

/** Returns one lowercase SHA-256 digest. */
function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

/** Dispatches only the three explicit protected-publication modes. */
async function main() {
  const mode = process.argv[2];
  if (process.argv.length !== 3 || !["--prepare", "--verify-draft", "--verify-published"].includes(mode)) {
    throw new Error("Use --prepare, --verify-draft, or --verify-published.");
  }
  if (mode === "--prepare") await preparePublication(REPOSITORY_ROOT);
  if (mode === "--verify-draft") await verifyDraft(REPOSITORY_ROOT);
  if (mode === "--verify-published") await verifyPublished(REPOSITORY_ROOT);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    await main();
  } catch (error) {
    console.error(`[bottie] ${error instanceof Error ? error.message : "Updater publication failed."}`);
    process.exitCode = 1;
  }
}
