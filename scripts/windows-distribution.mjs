#!/usr/bin/env node

/** Builds, Authenticode-signs, verifies, and inspects Bottie's Windows 0.9.0 distribution package. */

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
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
import { bindUpdaterArtifactEvidence, exportUpdaterArtifact, signUpdaterArtifact } from "./updater-artifact.mjs";

const DEFAULT_EVIDENCE_PATH = "package/windows-package-evidence.json";
const SIGNING_CERTIFICATE_PATH_ENVIRONMENT = "BOTTIE_WINDOWS_SIGNING_CERTIFICATE_PATH";
const SIGNING_CERTIFICATE_PASSWORD_ENVIRONMENT = "BOTTIE_WINDOWS_SIGNING_CERTIFICATE_PASSWORD";
const SIGNTOOL_PATH_ENVIRONMENT = "BOTTIE_WINDOWS_SIGNTOOL_PATH";
const TIMESTAMP_URL = "http://timestamp.digicert.com";
const WINDOWS_EXECUTABLE_NAME = "bottie.exe";
const WINDOWS_TARGET = "x86_64-pc-windows-msvc";
const PYTHON_DEVELOPMENT_CONFIG = "src-tauri/tauri.python-development.windows.conf.json";
const PYTHON_INPUT_DIRECTORY = "package/python-development";
const PYTHON_RUNNER_EVIDENCE = "python-runtime-evidence.json";
const SHA256_PATTERN = /^[a-f0-9]{64}$/;

/** Returns the locked product build that stops before packaging so its executable can be signed exactly once. */
export function distributionBuildArguments() {
  return ["build", "--no-bundle", "--no-sign", "--ci", "--", "--locked"];
}

/** Returns the MSI-only bundling step that preserves the already signed product executable. */
export function distributionBundleArguments() {
  return ["bundle", "--bundles", "msi", "--no-sign", "--ci", "--config", "src-tauri/tauri.updater.conf.json"];
}

/** Returns the locked product build that selects only the explicit Python-bearing Windows resources. */
export function protectedPythonDistributionBuildArguments() {
  return ["build", "--no-bundle", "--no-sign", "--ci", "--config", PYTHON_DEVELOPMENT_CONFIG, "--", "--locked"];
}

/** Returns the MSI bundling step that retains updater output and the explicit Python-bearing resources. */
export function protectedPythonDistributionBundleArguments() {
  return [
    "bundle",
    "--bundles",
    "msi",
    "--no-sign",
    "--ci",
    "--config",
    "src-tauri/tauri.updater.conf.json",
    "--config",
    PYTHON_DEVELOPMENT_CONFIG,
  ];
}

/** Returns the exact staged Python executables that must be signed before protected MSI bundling. */
export function protectedPythonDistributionSigningPlan(repositoryRoot) {
  const inputRoot = win32.join(win32.resolve(repositoryRoot), ...PYTHON_INPUT_DIRECTORY.split("/"));
  return [
    {
      label: "protected Python AppContainer controller",
      path: win32.join(inputRoot, `bottie-python-appcontainer-${WINDOWS_TARGET}.exe`),
    },
    {
      label: "protected Python runner",
      path: win32.join(inputRoot, `bottie-python-runner-${WINDOWS_TARGET}.exe`),
    },
  ];
}

/** Rebinds a closed staged-runtime marker to the final Authenticode-signed runner bytes. */
export function signedPythonRunnerEvidence(evidence, runner) {
  const expectedKeys = ["manifestSha256", "runnerBytes", "runnerSha256", "runtime", "schemaVersion", "target"];
  const actualKeys =
    evidence && typeof evidence === "object" && !Array.isArray(evidence) ? Object.keys(evidence).sort() : [];
  if (
    JSON.stringify(actualKeys) !== JSON.stringify(expectedKeys.sort()) ||
    evidence.schemaVersion !== 1 ||
    evidence.target !== WINDOWS_TARGET ||
    !SHA256_PATTERN.test(evidence.manifestSha256) ||
    !SHA256_PATTERN.test(evidence.runnerSha256) ||
    !Number.isSafeInteger(evidence.runnerBytes) ||
    evidence.runnerBytes <= 0 ||
    !evidence.runtime ||
    typeof evidence.runtime !== "object" ||
    Array.isArray(evidence.runtime)
  ) {
    throw new Error("The staged Python runner evidence is not closed and complete.");
  }
  if (!Buffer.isBuffer(runner) || runner.length === 0) {
    throw new Error("The signed Python runner is unavailable.");
  }
  return {
    ...evidence,
    runnerBytes: runner.length,
    runnerSha256: createHash("sha256").update(runner).digest("hex"),
  };
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

/** Signs the staged Python executables and refreshes the package-owned runner identity before bundling. */
async function signProtectedPythonCode(repositoryRoot, signToolPath, credentials) {
  const plan = protectedPythonDistributionSigningPlan(repositoryRoot);
  for (const step of plan) {
    await requireRegularFile(step.path, step.label);
    signAndVerify(signToolPath, credentials, step.path);
  }
  let runner;
  try {
    runner = await readFile(plan.at(-1).path);
  } catch {
    throw new Error("The signed Python runner is unavailable.");
  }
  const evidencePath = win32.join(
    win32.resolve(repositoryRoot),
    ...PYTHON_INPUT_DIRECTORY.split("/"),
    PYTHON_RUNNER_EVIDENCE,
  );
  let evidence;
  try {
    evidence = JSON.parse(await readFile(evidencePath, "utf8"));
  } catch {
    throw new Error("The staged Python runner evidence is unavailable or malformed.");
  }
  await writeFile(evidencePath, `${JSON.stringify(signedPythonRunnerEvidence(evidence, runner), null, 2)}\n`, {
    mode: 0o600,
  });
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
async function runSignedProduct(repositoryRoot, temporaryRoot, signToolPath, credentials, protectedPython) {
  const targetDirectory = join(temporaryRoot, "distribution-target");
  const extractedDirectory = join(temporaryRoot, "distribution-extracted");
  await mkdir(extractedDirectory);
  if (protectedPython) await signProtectedPythonCode(repositoryRoot, signToolPath, credentials);
  const buildArguments = protectedPython ? protectedPythonDistributionBuildArguments() : distributionBuildArguments();
  buildWindowsBundle(repositoryRoot, buildArguments, targetDirectory);
  const executablePath = join(targetDirectory, "release", WINDOWS_EXECUTABLE_NAME);
  await requireRegularFile(executablePath, "Bottie distribution executable");
  signAndVerify(signToolPath, credentials, executablePath);
  const bundleArguments = protectedPython
    ? protectedPythonDistributionBundleArguments()
    : distributionBundleArguments();
  buildWindowsBundle(repositoryRoot, bundleArguments, targetDirectory);
  const msiPath = await findSingleMsi(join(targetDirectory, "release", "bundle", "msi"));
  signAndVerify(signToolPath, credentials, msiPath);
  const updater = await signUpdaterArtifact(repositoryRoot, msiPath);
  return { artifactPath: msiPath, bundle: await inspectWindowsMsi(msiPath, extractedDirectory), updater };
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
async function runWindowsDistribution(repositoryRoot, protectedPython = false) {
  const credentials = resolveSigningCredentials(process.env, repositoryRoot);
  const signToolPath = resolveSignToolPath(process.env);
  await requireRegularFile(credentials.certificatePath, "protected Windows signing certificate");
  await requireRegularFile(signToolPath, "Windows SDK SignTool executable");
  const temporaryRoot = await mkdtemp(join(tmpdir(), "bottie-windows-distribution-"));
  try {
    const { artifactPath, bundle, updater } = await runSignedProduct(
      repositoryRoot,
      temporaryRoot,
      signToolPath,
      credentials,
      protectedPython,
    );
    const smoke = await runIsolatedSmoke(repositoryRoot, temporaryRoot);
    const packageEvidence = versionedPackageEvidence(
      await applicationVersion(repositoryRoot),
      combineWindowsPackageEvidence(bundle, smoke),
    );
    const evidence = {
      ...packageEvidence,
      updater: bindUpdaterArtifactEvidence(updater, "windows-x86_64", bundle.installer.sha256),
    };
    await emitEvidence(repositoryRoot, evidence);
    await exportUpdaterArtifact(repositoryRoot, artifactPath, "windows-x86_64", evidence.version);
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
}

/** Accepts only the deliberate protected-runner mode. */
async function main() {
  if (process.platform !== "win32") throw new Error("Windows distribution validation requires a Windows host.");
  const [mode, ...arguments_] = process.argv.slice(2);
  if (arguments_.length !== 0 || !["--run", "--run-python"].includes(mode)) {
    throw new Error("Use the exact --run or --run-python mode.");
  }
  const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  await runWindowsDistribution(repositoryRoot, mode === "--run-python");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    await main();
  } catch (error) {
    console.error(`[bottie] ${error instanceof Error ? error.message : "Windows distribution validation failed."}`);
    process.exitCode = 1;
  }
}
