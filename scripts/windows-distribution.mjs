#!/usr/bin/env node

/** Builds, Authenticode-signs, verifies, and inspects Bottie's Windows 0.9.0 distribution package. */

import { spawnSync } from "node:child_process";
import { lstat, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, relative, resolve, sep, win32 } from "node:path";
import { fileURLToPath } from "node:url";

import {
  buildWindowsBundle,
  combineWindowsPackageEvidence,
  findSingleMsi,
  inspectWindowsMsi,
  smokeWindowsBundle,
  versionedPackageEvidence,
  windowsSmokeBuildArguments,
} from "./windows-package.mjs";
import { signUpdaterArtifact } from "./updater-artifact.mjs";

const DEFAULT_EVIDENCE_PATH = "package/windows-package-evidence.json";
const SIGNING_CERTIFICATE_PATH_ENVIRONMENT = "BOTTIE_WINDOWS_SIGNING_CERTIFICATE_PATH";
const SIGNING_CERTIFICATE_PASSWORD_ENVIRONMENT = "BOTTIE_WINDOWS_SIGNING_CERTIFICATE_PASSWORD";
const SIGNTOOL_PATH_ENVIRONMENT = "BOTTIE_WINDOWS_SIGNTOOL_PATH";
const TIMESTAMP_URL = "http://timestamp.digicert.com";
const WINDOWS_EXECUTABLE_NAME = "bottie.exe";

/** Returns the locked product build that stops before packaging so its executable can be signed exactly once. */
export function distributionBuildArguments() {
  return ["build", "--no-bundle", "--no-sign", "--ci", "--", "--locked"];
}

/** Returns the MSI-only bundling step that preserves the already signed product executable. */
export function distributionBundleArguments() {
  return ["bundle", "--bundles", "msi", "--no-sign", "--ci", "--config", "src-tauri/tauri.updater.conf.json"];
}

/** Returns one SHA-256 Authenticode signing command with a fixed RFC 3161 timestamp service. */
export function signToolSignArguments(certificatePath, password, artifactPath) {
  return [
    "sign",
    "/fd",
    "SHA256",
    "/tr",
    TIMESTAMP_URL,
    "/td",
    "SHA256",
    "/f",
    certificatePath,
    "/p",
    password,
    artifactPath,
  ];
}

/** Returns one independent Windows distribution-policy verification command. */
export function signToolVerifyArguments(artifactPath) {
  return ["verify", "/pa", "/all", "/v", artifactPath];
}

/** Resolves a complete protected PFX credential pair while rejecting repository-contained certificate files. */
export function resolveSigningCredentials(environment, repositoryRoot) {
  const certificatePath = environment[SIGNING_CERTIFICATE_PATH_ENVIRONMENT]?.trim();
  const password = environment[SIGNING_CERTIFICATE_PASSWORD_ENVIRONMENT];
  if (!certificatePath || !password) throw new Error("Protected Windows signing credentials are unavailable.");
  if (!win32.isAbsolute(certificatePath)) throw new Error("The Windows signing certificate path must be absolute.");
  const relativeCertificatePath = win32.relative(win32.resolve(repositoryRoot), win32.resolve(certificatePath));
  const isRepositoryPath =
    relativeCertificatePath === "" ||
    (!win32.isAbsolute(relativeCertificatePath) &&
      relativeCertificatePath !== ".." &&
      !relativeCertificatePath.startsWith(`..${win32.sep}`));
  if (isRepositoryPath) throw new Error("The Windows signing certificate must stay outside the repository.");
  return { certificatePath, password };
}

/** Requires one caller-selected SignTool executable without searching or serializing host paths. */
function resolveSignToolPath(environment) {
  const path = environment[SIGNTOOL_PATH_ENVIRONMENT]?.trim();
  if (!path || !win32.isAbsolute(path)) throw new Error("The Windows SDK SignTool path is unavailable.");
  return path;
}

/** Runs SignTool while discarding certificate-, identity-, path-, and raw-command-bearing output. */
function runSignTool(signToolPath, arguments_) {
  const result = spawnSync(signToolPath, arguments_, { encoding: "utf8" });
  if (result.error || result.status !== 0) throw new Error("Windows Authenticode signing or verification failed.");
}

/** Signs and then independently verifies exactly one file without returning signer details. */
function signAndVerify(signToolPath, credentials, artifactPath) {
  runSignTool(signToolPath, signToolSignArguments(credentials.certificatePath, credentials.password, artifactPath));
  runSignTool(signToolPath, signToolVerifyArguments(artifactPath));
}

/** Requires one regular file before any credential-bearing host command is invoked. */
async function requireRegularFile(path, description) {
  try {
    if ((await lstat(path)).isFile()) return;
  } catch {
    // The fixed path is reduced to the same path-free error below.
  }
  throw new Error(`The expected ${description} is unavailable.`);
}

/** Builds a separately identified unsigned package and returns only its isolated native smoke outcome. */
async function runIsolatedSmoke(repositoryRoot, temporaryRoot) {
  const targetDirectory = join(temporaryRoot, "smoke-target");
  const extractedDirectory = join(temporaryRoot, "smoke-extracted");
  await mkdir(extractedDirectory);
  buildWindowsBundle(repositoryRoot, windowsSmokeBuildArguments(), targetDirectory);
  const msiPath = await findSingleMsi(join(targetDirectory, "release", "bundle", "msi"));
  const bundle = await inspectWindowsMsi(msiPath, extractedDirectory);
  return smokeWindowsBundle(extractedDirectory, bundle);
}

/** Builds the real product, signs its executable before bundling, then signs and inspects the resulting MSI. */
async function runSignedProduct(repositoryRoot, temporaryRoot, signToolPath, credentials) {
  const targetDirectory = join(temporaryRoot, "distribution-target");
  const extractedDirectory = join(temporaryRoot, "distribution-extracted");
  await mkdir(extractedDirectory);
  buildWindowsBundle(repositoryRoot, distributionBuildArguments(), targetDirectory);
  const executablePath = join(targetDirectory, "release", WINDOWS_EXECUTABLE_NAME);
  await requireRegularFile(executablePath, "Bottie distribution executable");
  signAndVerify(signToolPath, credentials, executablePath);
  buildWindowsBundle(repositoryRoot, distributionBundleArguments(), targetDirectory);
  const msiPath = await findSingleMsi(join(targetDirectory, "release", "bundle", "msi"));
  signAndVerify(signToolPath, credentials, msiPath);
  await signUpdaterArtifact(repositoryRoot, msiPath);
  return inspectWindowsMsi(msiPath, extractedDirectory);
}

/** Reads the checked-out numeric application version. */
async function applicationVersion(repositoryRoot) {
  const config = JSON.parse(await readFile(join(repositoryRoot, "src-tauri", "tauri.conf.json"), "utf8"));
  return config.version;
}

/** Emits only path-free evidence to the ignored release-gate input file. */
async function emitEvidence(repositoryRoot, evidence) {
  const suppliedPath = process.env.BOTTIE_WINDOWS_EVIDENCE_PATH?.trim() || DEFAULT_EVIDENCE_PATH;
  const evidencePath = isAbsolute(suppliedPath) ? suppliedPath : resolve(repositoryRoot, suppliedPath);
  const evidenceRoot = resolve(repositoryRoot, "package");
  const relativeEvidencePath = relative(evidenceRoot, evidencePath);
  if (relativeEvidencePath === "" || relativeEvidencePath === ".." || relativeEvidencePath.startsWith(`..${sep}`)) {
    throw new Error("Windows distribution evidence must stay inside the repository package directory.");
  }
  await mkdir(dirname(evidencePath), { recursive: true });
  await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
}

/** Runs the complete protected Windows distribution validation in disposable build directories. */
async function runWindowsDistribution(repositoryRoot) {
  const credentials = resolveSigningCredentials(process.env, repositoryRoot);
  const signToolPath = resolveSignToolPath(process.env);
  await requireRegularFile(credentials.certificatePath, "protected Windows signing certificate");
  await requireRegularFile(signToolPath, "Windows SDK SignTool executable");
  const temporaryRoot = await mkdtemp(join(tmpdir(), "bottie-windows-distribution-"));
  try {
    const bundle = await runSignedProduct(repositoryRoot, temporaryRoot, signToolPath, credentials);
    const smoke = await runIsolatedSmoke(repositoryRoot, temporaryRoot);
    const evidence = versionedPackageEvidence(
      await applicationVersion(repositoryRoot),
      combineWindowsPackageEvidence(bundle, smoke),
    );
    await emitEvidence(repositoryRoot, evidence);
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
}

/** Accepts only the deliberate protected-runner mode. */
async function main() {
  if (process.platform !== "win32") throw new Error("Windows distribution validation requires a Windows host.");
  if (process.argv.slice(2).length !== 1 || process.argv[2] !== "--run") throw new Error("Use the exact --run mode.");
  const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  await runWindowsDistribution(repositoryRoot);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    await main();
  } catch (error) {
    console.error(`[bottie] ${error instanceof Error ? error.message : "Windows distribution validation failed."}`);
    process.exitCode = 1;
  }
}
