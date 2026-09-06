#!/usr/bin/env node

/** Reinspects and exercises an already signed protected macOS Python package. */

import { spawnSync } from "node:child_process";
import { lstat, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { exerciseProof } from "./macos-python-xpc.mjs";
import { packagedBundleLayout } from "./macos-packaged-python-smoke.mjs";
import { credentialFreeMacosEnvironment } from "./macos-protected-python-package.mjs";
import { protectedInspectionSha256, validateProtectedPackageInspection } from "./python-protected-package.mjs";
import { inspectPackagedPythonBundle, validateRuntimeManifest } from "./python-runtime-bundle.mjs";

const MAX_CAPTURED_OUTPUT_BYTES = 128 * 1_024;
const PROOF_TIMEOUT_MS = 45_000;
const SOURCE_SHA_PATTERN = /^[a-f0-9]{40}$/;
const HOST_INJECTION_ENVIRONMENT_PATTERN = /^(?:CODESIGN_|DYLD_|LD_)/;

/** Requires one exact lowercase Git source revision. */
function requireSourceSha(sourceSha) {
  if (typeof sourceSha !== "string" || !SOURCE_SHA_PATTERN.test(sourceSha)) {
    throw new Error("The macOS shipping-containment source revision is invalid.");
  }
}

/** Removes signing credentials and host loader overrides from proof subprocesses. */
export function macosShippingProofEnvironment(environment) {
  return Object.fromEntries(
    Object.entries(credentialFreeMacosEnvironment(environment)).filter(
      ([name]) => !HOST_INJECTION_ENVIRONMENT_PATTERN.test(name),
    ),
  );
}

/** Returns the exact independent signature-verification commands for the protected app. */
export function macosShippingVerificationPlan(applicationRoot) {
  const application = resolve(applicationRoot);
  const layout = packagedBundleLayout(application);
  return [
    { label: "packaged Python runner", path: layout.runner },
    { label: "packaged Python XPC service", path: layout.service },
    { label: "packaged Python XPC client", path: layout.application },
    { label: "protected Bottie application", path: application },
  ].map(({ label, path }) => ({
    arguments: ["--verify", "--strict", "--verbose=2", path],
    command: "/usr/bin/codesign",
    label,
  }));
}

/** Requires the supplied inspection to equal a fresh inspection of the signed app. */
export function requireMatchingMacosProtectedInspection(expected, actual) {
  const supplied = validateProtectedPackageInspection("macos", expected);
  const reinspected = validateProtectedPackageInspection("macos", actual);
  if (protectedInspectionSha256(supplied) !== protectedInspectionSha256(reinspected)) {
    throw new Error("The signed macOS protected package does not match its supplied inspection.");
  }
  return reinspected;
}

/** Creates the closed path-free shipping record after every macOS proof has passed. */
export function macosShippingContainmentRecord(sourceSha, inspection) {
  requireSourceSha(sourceSha);
  const accepted = validateProtectedPackageInspection("macos", inspection);
  return {
    appSandboxDeniedHostFixture: true,
    cancellation: true,
    clientExitKilledRunner: true,
    inspectedProtectedPackage: true,
    inspectionSha256: protectedInspectionSha256(accepted),
    platform: "macos",
    privatePipeExecution: true,
    schemaVersion: 1,
    sourceSha,
    status: "ok",
    target: accepted.target,
  };
}

/** Requires the protected application root to be one ordinary directory. */
async function requireApplicationDirectory(path) {
  try {
    const status = await lstat(path);
    if (status.isDirectory() && !status.isSymbolicLink()) return;
  } catch {
    // Collapse absent and invalid roots into one path-free failure.
  }
  throw new Error("The signed macOS protected application is unavailable.");
}

/** Reads one required JSON object without reflecting its host path. */
async function readJson(path) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch {
    throw new Error("Required macOS shipping-containment evidence is unavailable or malformed.");
  }
}

/** Writes one private path-free shipping-containment document. */
async function writeJson(path, value) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
}

/** Runs one verification command without retaining its arguments, output, or host paths. */
function runVerificationCommand(step, environment) {
  const result = spawnSync(step.command, step.arguments, {
    encoding: "utf8",
    env: environment,
    maxBuffer: MAX_CAPTURED_OUTPUT_BYTES,
    timeout: PROOF_TIMEOUT_MS,
  });
  if (result.error || result.status !== 0) {
    throw new Error(`${basename(step.command)} failed while verifying the ${step.label}.`);
  }
}

/** Reinspects and exercises the exact signed app before returning its closed record. */
export async function proveMacosShippingContainment(repository, sourceSha, applicationRoot, suppliedInspection) {
  if (process.platform !== "darwin") throw new Error("The macOS shipping-containment proof requires macOS.");
  requireSourceSha(sourceSha);
  const application = resolve(applicationRoot);
  await requireApplicationDirectory(application);
  const manifest = validateRuntimeManifest(
    await readJson(resolve(repository, "python-runner", "runtime-manifest.json")),
  );
  const inspection = requireMatchingMacosProtectedInspection(
    suppliedInspection,
    await inspectPackagedPythonBundle(application, "macos", manifest),
  );
  const environment = macosShippingProofEnvironment(process.env);
  for (const step of macosShippingVerificationPlan(application)) runVerificationCommand(step, environment);
  const fixtureDirectory = await mkdtemp(`${tmpdir()}/bottie-macos-shipping-python-proof-`);
  try {
    await exerciseProof(packagedBundleLayout(application), fixtureDirectory, environment);
  } finally {
    await rm(fixtureDirectory, { recursive: true, force: true });
  }
  return macosShippingContainmentRecord(sourceSha, inspection);
}

/** Dispatches the single local proof mode for an already signed protected app. */
async function main() {
  const repository = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const [mode, ...arguments_] = process.argv.slice(2);
  if (mode !== "--prove" || arguments_.length !== 4) {
    throw new Error("Use --prove with source revision, signed app, inspection, and output.");
  }
  const [sourceSha, applicationPath, inspectionPath, outputPath] = arguments_;
  const containment = await proveMacosShippingContainment(
    repository,
    sourceSha,
    applicationPath,
    await readJson(resolve(inspectionPath)),
  );
  await writeJson(resolve(outputPath), containment);
  console.log("[bottie] Credential-free macOS shipping containment accepted.");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`[bottie] ${error instanceof Error ? error.message : "The macOS containment proof failed."}`);
    process.exitCode = 1;
  });
}
