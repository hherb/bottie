/** Protected Tauri updater signing for exact final platform-distribution bytes. */

import { spawnSync } from "node:child_process";
import { copyFile, lstat, mkdir, rm } from "node:fs/promises";
import { isAbsolute, join, relative, resolve, sep } from "node:path";

const PRIVATE_KEY_CONTENT_ENVIRONMENT = "TAURI_SIGNING_PRIVATE_KEY";
const PRIVATE_KEY_PATH_ENVIRONMENT = "TAURI_SIGNING_PRIVATE_KEY_PATH";
const PRIVATE_KEY_PASSWORD_ENVIRONMENT = "TAURI_SIGNING_PRIVATE_KEY_PASSWORD";
const SHA256_PATTERN = /^[a-f0-9]{64}$/;
const PLATFORM_SIGNING_ENVIRONMENT_PATTERN = /^(?:TAURI_SIGNING_|BOTTIE_(?:APPLE|LINUX|WINDOWS)_)/;
const SUPPORTED_TARGETS = new Set(["darwin-aarch64", "darwin-x86_64", "linux-x86_64", "windows-x86_64"]);
const VERSION_PATTERN = /^\d+\.\d+\.\d+$/;
const RELEASE_SUFFIXES = {
  "darwin-aarch64": ".app.tar.gz",
  "darwin-x86_64": ".app.tar.gz",
  "linux-x86_64": ".deb",
  "windows-x86_64": ".msi",
};

/** Validates one protected private-key source and password without reading either value. */
export function requireUpdaterSigningEnvironment(environment, repositoryRoot) {
  const privateKey = environment[PRIVATE_KEY_CONTENT_ENVIRONMENT]?.trim();
  const privateKeyPath = environment[PRIVATE_KEY_PATH_ENVIRONMENT]?.trim();
  const password = environment[PRIVATE_KEY_PASSWORD_ENVIRONMENT];
  if (Boolean(privateKey) === Boolean(privateKeyPath) || !password) {
    throw new Error("Protected updater signing credentials are unavailable or ambiguous.");
  }
  if (privateKeyPath) {
    if (!isAbsolute(privateKeyPath)) throw new Error("The updater private-key path must be absolute.");
    const relativePath = relative(resolve(repositoryRoot), resolve(privateKeyPath));
    const insideRepository =
      relativePath === "" ||
      (!isAbsolute(relativePath) && relativePath !== ".." && !relativePath.startsWith(`..${sep}`));
    if (insideRepository) throw new Error("The updater private key must stay outside the repository.");
  }
  return { source: privateKeyPath ? "protected-path" : "protected-content" };
}

/** Returns the credential-free Tauri CLI arguments for one exact final artifact. */
export function updaterSigningArguments(artifactPath) {
  return ["--tauri", "signer", "sign", artifactPath];
}

/** Returns the locked native verifier arguments for exact artifact, signature, and public-key files. */
export function updaterVerificationArguments(repositoryRoot, artifactPath, signaturePath) {
  return [
    "run",
    "--quiet",
    "--locked",
    "--manifest-path",
    join(repositoryRoot, "src-tauri", "Cargo.toml"),
    "--bin",
    "bottie-updater-evidence",
    "--",
    "--verify",
    artifactPath,
    signaturePath,
    join(repositoryRoot, "distribution", "update", "bottie-updater.pub"),
  ];
}

/** Removes private updater signing inputs before invoking the public verification process. */
export function publicUpdaterVerificationEnvironment(environment) {
  return Object.fromEntries(
    Object.entries(environment).filter(([name]) => !PLATFORM_SIGNING_ENVIRONMENT_PATTERN.test(name)),
  );
}

/** Parses only the native verifier's exact path-free cryptographic evidence shape. */
export function parseUpdaterArtifactEvidence(output) {
  let evidence;
  try {
    evidence = JSON.parse(output);
  } catch {
    return null;
  }
  if (
    !hasExactKeys(evidence, ["artifact", "publicKeySha256", "schemaVersion", "signature"]) ||
    evidence.schemaVersion !== 1 ||
    !hasExactKeys(evidence.artifact, ["sha256", "size"]) ||
    !isSha256(evidence.artifact.sha256) ||
    !Number.isSafeInteger(evidence.artifact.size) ||
    evidence.artifact.size <= 0 ||
    !isSha256(evidence.publicKeySha256) ||
    !hasExactKeys(evidence.signature, ["format", "sha256", "verifies"]) ||
    evidence.signature.format !== "minisign" ||
    !isSha256(evidence.signature.sha256) ||
    evidence.signature.verifies !== true
  ) {
    return null;
  }
  return evidence;
}

/** Binds verified updater evidence to one supported release target and optional final distribution hash. */
export function bindUpdaterArtifactEvidence(evidence, target, expectedArtifactSha256) {
  const verified = parseUpdaterArtifactEvidence(JSON.stringify(evidence));
  if (!verified) throw new Error("Verified updater artifact evidence is invalid.");
  if (!SUPPORTED_TARGETS.has(target)) throw new Error("Updater artifact target is unsupported.");
  if (expectedArtifactSha256 !== undefined && verified.artifact.sha256 !== expectedArtifactSha256) {
    throw new Error("Updater evidence does not match the final artifact bytes.");
  }
  return { ...verified, target };
}

/** Copies exact verified updater bytes into one canonical ignored release-staging boundary. */
export async function exportUpdaterArtifact(repositoryRoot, artifactPath, target, version) {
  if (!SUPPORTED_TARGETS.has(target)) throw new Error("Updater export target is unsupported.");
  if (!VERSION_PATTERN.test(version)) throw new Error("Updater export version must be numeric SemVer.");
  const suffix = RELEASE_SUFFIXES[target];
  if (!artifactPath.toLowerCase().endsWith(suffix === ".app.tar.gz" ? ".tar.gz" : suffix)) {
    throw new Error("Updater export artifact format does not match its target.");
  }
  await requireRegularFile(artifactPath);
  await requireRegularFile(`${artifactPath}.sig`);
  const outputDirectory = join(repositoryRoot, "package", "updater-artifacts");
  const artifact = `bottie_${version}_${target}${suffix}`;
  const signature = `${artifact}.sig`;
  await mkdir(outputDirectory, { recursive: true });
  await copyFile(artifactPath, join(outputDirectory, artifact));
  await copyFile(`${artifactPath}.sig`, join(outputDirectory, signature));
  return { artifact, signature, target };
}

/** Signs one final platform artifact and returns only its adjacent signature path. */
export async function signUpdaterArtifact(repositoryRoot, artifactPath, environment = process.env) {
  requireUpdaterSigningEnvironment(environment, repositoryRoot);
  const signaturePath = `${artifactPath}.sig`;
  await requireRegularFile(artifactPath);
  await rm(signaturePath, { force: true });
  const wrapper = join(repositoryRoot, "scripts", "macos-development-signing.mjs");
  const result = spawnSync(process.execPath, [wrapper, ...updaterSigningArguments(artifactPath)], {
    cwd: repositoryRoot,
    env: environment,
    stdio: ["ignore", "ignore", "inherit"],
  });
  if (result.error || result.status !== 0) throw new Error("Tauri updater signing failed.");
  await requireRegularFile(signaturePath);
  return verifyUpdaterArtifact(repositoryRoot, artifactPath, signaturePath, environment);
}

/** Cryptographically verifies one Tauri signature and returns only bounded path-free evidence. */
function verifyUpdaterArtifact(repositoryRoot, artifactPath, signaturePath, environment) {
  const result = spawnSync("cargo", updaterVerificationArguments(repositoryRoot, artifactPath, signaturePath), {
    cwd: repositoryRoot,
    encoding: "utf8",
    env: publicUpdaterVerificationEnvironment(environment),
  });
  const evidence = parseUpdaterArtifactEvidence(result.stdout ?? "");
  if (result.error || result.status !== 0 || !evidence) {
    throw new Error("Tauri updater signature verification failed.");
  }
  return evidence;
}

/** Returns true only for an ordinary object with the exact expected key set. */
function hasExactKeys(value, keys) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const actual = Object.keys(value).sort();
  return actual.length === keys.length && actual.every((key, index) => key === [...keys].sort()[index]);
}

/** Reports whether a value is one lowercase SHA-256 digest. */
function isSha256(value) {
  return typeof value === "string" && SHA256_PATTERN.test(value);
}

/** Requires one regular protected artifact while retaining no path in the failure. */
async function requireRegularFile(path) {
  try {
    if ((await lstat(path)).isFile()) return;
  } catch {
    // Reduce absent and unreadable artifacts to one fixed failure.
  }
  throw new Error("The protected updater artifact is unavailable.");
}
