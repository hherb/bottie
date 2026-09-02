/** Pure contracts for Bottie's signed static Tauri update manifest and publication evidence. */

import { createHash } from "node:crypto";

const SCHEMA_VERSION = 1;
const RELEASE_ORIGIN = "https://github.com";
const SEMVER_PATTERN = /^\d+\.\d+\.\d+$/;
const SHA256_PATTERN = /^[a-f0-9]{64}$/;
const TARGET_PATTERN = /^(?:darwin|linux|windows)-(?:aarch64|x86_64|i686|armv7)$/;
const BASE64_PATTERN = /^[A-Za-z0-9+/]+={0,2}$/;
const MAX_NOTES_LENGTH = 8_192;
const MAX_SIGNING_TEXT_LENGTH = 4_096;
const MIN_SIGNING_TEXT_LENGTH = 32;

/** Builds a deterministic Tauri static update manifest from exact immutable release artifacts. */
export function buildStaticUpdateManifest({ artifacts, notes, publishedAt, version }) {
  requireVersion(version);
  const normalizedNotes = requireBoundedText(notes, "Update notes", 1, MAX_NOTES_LENGTH);
  const pubDate = requirePublicationDate(publishedAt);
  const normalizedArtifacts = normalizeArtifacts(artifacts, version);
  const platforms = Object.fromEntries(
    normalizedArtifacts.map(({ signature, target, url }) => [target, { signature, url }]),
  );
  return { notes: normalizedNotes, platforms, pub_date: pubDate, version };
}

/** Produces path-free evidence bound to the exact manifest, public key, and updater artifact bytes. */
export function buildUpdateDeliveryEvidence({ artifacts, manifest, publicKey, status }) {
  const version = manifest?.version;
  requireVersion(version);
  if (status !== "draft" && status !== "published") throw new Error("Update publication status is invalid.");
  const normalizedArtifacts = normalizeArtifacts(artifacts, version);
  const expectedManifest = buildStaticUpdateManifest({
    artifacts: normalizedArtifacts,
    notes: manifest.notes,
    publishedAt: manifest.pub_date,
    version,
  });
  if (canonicalJson(manifest) !== canonicalJson(expectedManifest)) {
    throw new Error("Update manifest does not match the reviewed artifact inputs.");
  }
  const normalizedPublicKey = requirePublicKey(publicKey);
  return {
    schemaVersion: SCHEMA_VERSION,
    artifacts: normalizedArtifacts.map(({ artifactSha256, target }) => ({
      sha256: artifactSha256,
      target,
    })),
    manifest: { sha256: sha256(canonicalJson(expectedManifest)) },
    publicKeySha256: sha256(normalizedPublicKey),
    status,
    targets: normalizedArtifacts.map(({ target }) => target),
    version,
  };
}

/** Accepts only complete published evidence with internally consistent, path-free bindings. */
export function summarizeUpdateDeliveryEvidence(evidence) {
  if (
    evidence?.schemaVersion !== SCHEMA_VERSION ||
    evidence.status !== "published" ||
    !SEMVER_PATTERN.test(evidence.version ?? "") ||
    !isSha256(evidence.manifest?.sha256) ||
    !isSha256(evidence.publicKeySha256)
  ) {
    return null;
  }
  if (!Array.isArray(evidence.targets) || !Array.isArray(evidence.artifacts) || evidence.targets.length === 0) {
    return null;
  }
  const targets = [...evidence.targets];
  if (
    targets.some((target) => typeof target !== "string" || !TARGET_PATTERN.test(target)) ||
    new Set(targets).size !== targets.length ||
    targets.join("\0") !== [...targets].sort().join("\0") ||
    evidence.artifacts.length !== targets.length
  ) {
    return null;
  }
  const artifacts = evidence.artifacts.map((artifact) => ({
    sha256: isSha256(artifact?.sha256) ? artifact.sha256 : null,
    target: typeof artifact?.target === "string" ? artifact.target : null,
  }));
  if (artifacts.some((artifact, index) => !artifact.sha256 || artifact.target !== targets[index])) return null;
  const normalized = {
    schemaVersion: SCHEMA_VERSION,
    artifacts,
    manifest: { sha256: evidence.manifest.sha256 },
    publicKeySha256: evidence.publicKeySha256,
    status: "published",
    targets,
    version: evidence.version,
  };
  return canonicalJson(normalized) === canonicalJson(evidence) ? normalized : null;
}

/** Sorts and validates one exact updater artifact per supported desktop target. */
function normalizeArtifacts(artifacts, version) {
  if (!Array.isArray(artifacts) || artifacts.length === 0) {
    throw new Error("At least one signed update artifact is required.");
  }
  const normalized = artifacts.map((artifact) => {
    const target = artifact?.target;
    if (typeof target !== "string" || !TARGET_PATTERN.test(target)) {
      throw new Error("Update artifact target is unsupported.");
    }
    if (!isSha256(artifact.artifactSha256)) throw new Error("Update artifact SHA-256 is invalid.");
    return {
      artifactSha256: artifact.artifactSha256,
      signature: requireArtifactSignature(artifact.signature),
      target,
      url: requireImmutableReleaseUrl(artifact.url, version),
    };
  });
  normalized.sort((left, right) => left.target.localeCompare(right.target));
  for (let index = 1; index < normalized.length; index += 1) {
    if (normalized[index - 1].target === normalized[index].target) {
      throw new Error("Update manifest contains a duplicate target.");
    }
  }
  return normalized;
}

/** Requires one immutable asset URL under Bottie's exact versioned GitHub release tag. */
function requireImmutableReleaseUrl(value, version) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error("Update artifact URL is invalid.");
  }
  const prefix = `/hherb/bottie/releases/download/v${version}/`;
  if (
    url.origin !== RELEASE_ORIGIN ||
    !url.pathname.startsWith(prefix) ||
    url.pathname.length === prefix.length ||
    url.search ||
    url.hash ||
    url.username ||
    url.password
  ) {
    throw new Error("Update artifact URL must use Bottie's immutable release tag.");
  }
  return url.href;
}

/** Requires one parseable RFC 3339 publication time and emits its canonical UTC representation. */
function requirePublicationDate(value) {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}T/.test(value)) {
    throw new Error("Update publication date must be RFC 3339 text.");
  }
  const parsed = new Date(value);
  if (!Number.isFinite(parsed.valueOf())) throw new Error("Update publication date must be valid.");
  return parsed.toISOString();
}

/** Requires the complete four-line minisign signature that Tauri embeds in its static manifest. */
function requireArtifactSignature(value) {
  const encoded = requireEncodedSigningText(value, "Update artifact signature");
  const text = decodeSigningText(encoded, "Update artifact signature");
  const lines = text.split("\n");
  if (
    lines.length !== 4 ||
    lines[0] !== "untrusted comment: signature from minisign secret key" ||
    !isBase64(lines[1]) ||
    !lines[2].startsWith("trusted comment:") ||
    !isBase64(lines[3])
  ) {
    throw new Error("Update artifact signature must contain one complete minisign signature.");
  }
  return encoded;
}

/** Requires the generated base64 public-key file content that Tauri embeds in the application. */
function requirePublicKey(value) {
  const encoded = requireEncodedSigningText(value, "Updater public key");
  if (value !== encoded && value !== `${encoded}\n`) {
    throw new Error("Updater public key file must be canonical and may contain only one final newline.");
  }
  const text = decodeSigningText(encoded, "Updater public key");
  const lines = text.split("\n");
  if (
    (lines.length !== 2 && !(lines.length === 3 && lines[2] === "")) ||
    !/^untrusted comment: minisign public key: [A-F0-9]{16}$/.test(lines[0]) ||
    !lines[1].startsWith("RW") ||
    !isBase64(lines[1])
  ) {
    throw new Error("Updater public key must contain one complete minisign public key.");
  }
  return value;
}

/** Requires canonical generated base64 file content instead of a path or decoded signing text. */
function requireEncodedSigningText(value, label) {
  const text = requireBoundedText(value, label, MIN_SIGNING_TEXT_LENGTH, MAX_SIGNING_TEXT_LENGTH).trim();
  if (!isBase64(text) || Buffer.from(text, "base64").toString("base64") !== text) {
    throw new Error(`${label} must contain canonical generated base64 content.`);
  }
  return text;
}

/** Decodes generated signing content while rejecting invalid UTF-8, paths, and private material. */
function decodeSigningText(encoded, label) {
  const bytes = Buffer.from(encoded, "base64");
  const text = bytes.toString("utf8");
  if (
    !Buffer.from(text, "utf8").equals(bytes) ||
    /PRIVATE KEY|BEGIN [A-Z ]*PRIVATE/i.test(text) ||
    /^(?:\.{0,2}\/|[A-Za-z]:\\|\\\\)/.test(text)
  ) {
    throw new Error(`${label} must be public inline content, not private material or a path.`);
  }
  return text;
}

/** Checks one non-empty base64 field without decoding or retaining its bytes. */
function isBase64(value) {
  return typeof value === "string" && value.length >= MIN_SIGNING_TEXT_LENGTH && BASE64_PATTERN.test(value);
}

/** Requires bounded printable text while preserving signature and release-note bytes exactly. */
function requireBoundedText(value, label, minimum, maximum) {
  if (
    typeof value !== "string" ||
    value.length < minimum ||
    value.length > maximum ||
    /[\0\u0001-\u0008]/.test(value)
  ) {
    throw new Error(`${label} is invalid or outside Bottie's bounds.`);
  }
  return value;
}

/** Requires Bottie's numeric Tauri-compatible release version. */
function requireVersion(version) {
  if (typeof version !== "string" || !SEMVER_PATTERN.test(version)) {
    throw new Error("Update version must be numeric SemVer.");
  }
}

/** Returns stable JSON bytes for hashing and exact comparisons. */
function canonicalJson(value) {
  return `${JSON.stringify(value)}\n`;
}

/** Returns a lowercase SHA-256 digest. */
function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

/** Checks one lowercase SHA-256 value. */
function isSha256(value) {
  return typeof value === "string" && SHA256_PATTERN.test(value);
}
