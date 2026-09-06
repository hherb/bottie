#!/usr/bin/env node

/** Reinspects and exercises installed Python resources from an already signed protected Windows MSI. */

import { spawnSync } from "node:child_process";
import { lstat, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve, win32 } from "node:path";
import { fileURLToPath } from "node:url";

import {
  protectedInspectionSha256,
  validateProtectedPackageInspection,
  validateProtectedPythonInspection,
} from "./python-protected-package.mjs";
import { inspectPackagedPythonBundle, validateRuntimeManifest } from "./python-runtime-bundle.mjs";
import { signToolVerifyArguments } from "./windows-distribution.mjs";
import { inspectExtractedWindowsBundle, msiAdministrativeInstallArguments } from "./windows-package.mjs";

const MAX_CAPTURED_OUTPUT_BYTES = 128 * 1_024;
const PROOF_TIMEOUT_MS = 180_000;
const SOURCE_SHA_PATTERN = /^[a-f0-9]{40}$/;
const PROTECTED_ENVIRONMENT_PREFIXES = [
  "APPLE_",
  "BOTTIE_PYTHON_",
  "BOTTIE_UPDATER_",
  "BOTTIE_WINDOWS_SIGNING_",
  "COR_",
  "TAURI_SIGNING_",
];
const PROTECTED_ENVIRONMENT_NAMES = new Set([
  "BOTTIE_WINDOWS_SIGNTOOL_PATH",
  "DOTNET_STARTUP_HOOKS",
  "NODE_OPTIONS",
  "NODE_PATH",
  "__COMPAT_LAYER",
]);
const NATIVE_PROOF_FIELDS = [
  "appContainerDeniedHostFixture",
  "appContainerLowIntegrity",
  "appContainerNoCapabilities",
  "cancellation",
  "installedDevelopmentBundle",
  "jobCloseKilledRunner",
  "privatePipeExecution",
  "privilegesStripped",
  "resourceLimits",
];

/** Requires one exact lowercase Git source revision. */
function requireSourceSha(sourceSha) {
  if (typeof sourceSha !== "string" || !SOURCE_SHA_PATTERN.test(sourceSha)) {
    throw new Error("The Windows shipping-containment source revision is invalid.");
  }
}

/** Removes protected values and process-injection overrides from verification and proof subprocesses. */
export function credentialFreeWindowsEnvironment(environment) {
  return Object.fromEntries(Object.entries(environment).filter(([name]) => !isProtectedEnvironmentName(name)));
}

/** Identifies one case-insensitive protected or process-injection environment name. */
function isProtectedEnvironmentName(name) {
  const normalized = name.toUpperCase();
  return (
    PROTECTED_ENVIRONMENT_NAMES.has(normalized) ||
    PROTECTED_ENVIRONMENT_PREFIXES.some((prefix) => normalized.startsWith(prefix))
  );
}

/** Returns the independent verification steps for the MSI and every extracted executable in the Python path. */
export function windowsShippingVerificationPlan(signToolPath, msiPath, applicationRoot) {
  const steps = [{ label: "signed Windows protected package", path: msiPath }];
  if (applicationRoot !== undefined) {
    steps.push(
      { label: "protected Bottie application", path: win32.join(applicationRoot, "bottie.exe") },
      {
        label: "protected Python AppContainer controller",
        path: win32.join(applicationRoot, "bottie-python-appcontainer.exe"),
      },
      { label: "protected Python runner", path: win32.join(applicationRoot, "bottie-python-runner.exe") },
    );
  }
  return steps.map(({ label, path }) => ({
    arguments: signToolVerifyArguments(path),
    command: signToolPath,
    label,
  }));
}

/** Requires the fixed installed resources to equal the exact extracted protected package. */
export function requireMatchingWindowsProtectedInspections(expected, installed) {
  const supplied = validateProtectedPackageInspection("windows", expected);
  const observed = validateProtectedPackageInspection("windows", installed);
  if (protectedInspectionSha256(supplied) !== protectedInspectionSha256(observed)) {
    throw new Error("The installed Windows package does not match the signed protected package inspection.");
  }
  return observed;
}

/** Validates the complete closed native proof object without accepting added diagnostics. */
function validateInstalledWindowsProof(proof) {
  const expectedProofKeys = ["status", ...NATIVE_PROOF_FIELDS].sort();
  const proofKeys = proof && typeof proof === "object" && !Array.isArray(proof) ? Object.keys(proof).sort() : [];
  if (
    JSON.stringify(proofKeys) !== JSON.stringify(expectedProofKeys) ||
    proof.status !== "ok" ||
    !NATIVE_PROOF_FIELDS.every((field) => proof[field] === true)
  ) {
    throw new Error("The installed Windows native proof is incomplete.");
  }
  return proof;
}

/** Parses the installed verifier's bounded JSON without reflecting malformed output. */
export function parseInstalledWindowsProof(output) {
  let proof;
  try {
    proof = JSON.parse(output.trim());
  } catch {
    throw new Error("The installed Windows native proof returned invalid JSON.");
  }
  return validateInstalledWindowsProof(proof);
}

/** Creates the closed path-free shipping record after every installed Windows proof has passed. */
export function windowsShippingContainmentRecord(sourceSha, inspection, proof) {
  requireSourceSha(sourceSha);
  const accepted = validateProtectedPackageInspection("windows", inspection);
  const nativeProof = validateInstalledWindowsProof(proof);
  return {
    appContainerDeniedHostFixture: nativeProof.appContainerDeniedHostFixture,
    appContainerLowIntegrity: nativeProof.appContainerLowIntegrity,
    appContainerNoCapabilities: nativeProof.appContainerNoCapabilities,
    cancellation: nativeProof.cancellation,
    installedProtectedPackage: true,
    inspectionSha256: protectedInspectionSha256(accepted),
    jobCloseKilledRunner: nativeProof.jobCloseKilledRunner,
    platform: "windows",
    privatePipeExecution: nativeProof.privatePipeExecution,
    privilegesStripped: nativeProof.privilegesStripped,
    resourceLimits: nativeProof.resourceLimits,
    schemaVersion: 1,
    sourceSha,
    status: "ok",
    target: accepted.target,
  };
}

/** Requires one ordinary MSI without following a symbolic link or exposing its path. */
async function requireMsi(path) {
  try {
    const status = await lstat(path);
    if (status.isFile() && !status.isSymbolicLink() && path.toLowerCase().endsWith(".msi")) return;
  } catch {
    // Collapse absent and invalid inputs into one path-free failure.
  }
  throw new Error("The signed Windows protected package is unavailable.");
}

/** Requires one ordinary installed application directory without exposing its path. */
async function requireApplicationDirectory(path) {
  try {
    const status = await lstat(path);
    if (status.isDirectory() && !status.isSymbolicLink()) return;
  } catch {
    // Collapse absent and invalid inputs into one path-free failure.
  }
  throw new Error("The installed Windows protected application is unavailable.");
}

/** Resolves one caller-selected Windows SDK verifier without searching host state. */
function requireSignTool(environment) {
  const path = environment.BOTTIE_WINDOWS_SIGNTOOL_PATH?.trim();
  if (!path || !win32.isAbsolute(path)) throw new Error("The Windows SDK verification tool is unavailable.");
  return path;
}

/** Reads one required JSON object without reflecting its host path. */
async function readJson(path, description) {
  try {
    const value = JSON.parse(await readFile(path, "utf8"));
    if (value && typeof value === "object" && !Array.isArray(value)) return value;
  } catch {
    // Collapse unreadable and malformed inputs into one path-free failure.
  }
  throw new Error(`${description} is unavailable or malformed.`);
}

/** Writes one private path-free evidence document. */
async function writeJson(path, value) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
}

/** Runs one bounded host command without retaining arguments, output, or host paths. */
function runHostCommand(command, arguments_, environment) {
  const result = spawnSync(command, arguments_, {
    encoding: "utf8",
    env: environment,
    maxBuffer: MAX_CAPTURED_OUTPUT_BYTES,
    timeout: PROOF_TIMEOUT_MS,
  });
  if (result.error || result.status !== 0) throw new Error("A Windows shipping-containment command failed.");
  return result.stdout?.trim() ?? "";
}

/** Runs an already resolved public-trust verification plan. */
function runVerificationPlan(plan, environment) {
  for (const step of plan) runHostCommand(step.command, step.arguments, environment);
}

/** Verifies, extracts, reinspects, and exercises one already installed protected Windows MSI. */
export async function proveWindowsShippingContainment(repository, sourceSha, msiPath, installedRoot, candidate) {
  if (process.platform !== "win32") throw new Error("The Windows shipping-containment proof requires Windows.");
  requireSourceSha(sourceSha);
  const msi = resolve(msiPath);
  const installed = resolve(installedRoot);
  await requireMsi(msi);
  await requireApplicationDirectory(installed);
  const signTool = requireSignTool(process.env);
  const environment = credentialFreeWindowsEnvironment(process.env);
  runVerificationPlan(windowsShippingVerificationPlan(signTool, msi), environment);
  const temporary = await mkdtemp(join(tmpdir(), "bottie-windows-shipping-python-proof-"));
  const extracted = join(temporary, "extracted");
  try {
    await mkdir(extracted);
    runHostCommand("msiexec.exe", msiAdministrativeInstallArguments(msi, extracted), environment);
    const packageLayout = await inspectExtractedWindowsBundle(extracted);
    const packageApplication = join(extracted, ...packageLayout.applicationDirectory.split("/"));
    runVerificationPlan(windowsShippingVerificationPlan(signTool, msi, packageApplication).slice(1), environment);
    const manifest = validateRuntimeManifest(
      await readJson(join(repository, "python-runner", "runtime-manifest.json"), "The Python runtime manifest"),
    );
    const inspection = validateProtectedPythonInspection(
      sourceSha,
      "windows",
      candidate,
      await inspectPackagedPythonBundle(packageApplication, "windows", manifest),
    );
    requireMatchingWindowsProtectedInspections(
      inspection,
      await inspectPackagedPythonBundle(installed, "windows", manifest),
    );
    const proof = parseInstalledWindowsProof(
      runHostCommand(
        process.execPath,
        [join(repository, "scripts", "windows-python-appcontainer.mjs"), "--prove-installed"],
        { ...environment, BOTTIE_PYTHON_INSTALLED_ROOT: installed },
      ),
    );
    return { containment: windowsShippingContainmentRecord(sourceSha, inspection, proof), inspection };
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

/** Dispatches the single credential-free proof mode for an already signed and installed protected MSI. */
async function main() {
  const repository = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const [mode, ...arguments_] = process.argv.slice(2);
  if (mode !== "--prove" || arguments_.length !== 6) {
    throw new Error(
      "Use --prove with source revision, signed MSI, installed application, candidate evidence, inspection output, " +
        "and containment output.",
    );
  }
  const [sourceSha, msiPath, installedRoot, candidatePath, inspectionPath, containmentPath] = arguments_;
  const result = await proveWindowsShippingContainment(
    repository,
    sourceSha,
    msiPath,
    installedRoot,
    await readJson(resolve(candidatePath), "The Python release-candidate evidence"),
  );
  await writeJson(resolve(inspectionPath), result.inspection);
  await writeJson(resolve(containmentPath), result.containment);
  console.log("[bottie] Credential-free Windows shipping containment accepted.");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`[bottie] ${error instanceof Error ? error.message : "The Windows containment proof failed."}`);
    process.exitCode = 1;
  });
}
