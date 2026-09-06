#!/usr/bin/env node

/** Inspects and exercises Bottie's installed Python resources from an already signed protected Linux DEB. */

import { spawnSync } from "node:child_process";
import { copyFile, lstat, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { verificationArguments } from "./linux-distribution.mjs";
import {
  protectedInspectionSha256,
  validateProtectedPackageInspection,
  validateProtectedPythonInspection,
} from "./python-protected-package.mjs";
import { inspectPackagedPythonBundle, validateRuntimeManifest } from "./python-runtime-bundle.mjs";

const MAX_CAPTURED_OUTPUT_BYTES = 128 * 1_024;
const PROOF_TIMEOUT_MS = 180_000;
const PUBLISHED_FINGERPRINT = "5C1D104ACE472474CE21070B065CFE6D5D9FD8A4";
const SOURCE_SHA_PATTERN = /^[a-f0-9]{40}$/;
const PROTECTED_ENVIRONMENT_PATTERN = /^(?:APPLE_|BOTTIE_LINUX_SIGNING_|BOTTIE_UPDATER_|GNUPG|LD_|TAURI_SIGNING_)/;
const NATIVE_PROOF_FIELDS = [
  "cancellation",
  "environmentIsolated",
  "execDenied",
  "landlockDeniedHostFixture",
  "networkDenied",
  "parentCloseKilledRunner",
  "parentDeathSignal",
  "processCreationDenied",
  "resourceLimits",
  "runtimeReadable",
  "workspaceReadable",
];

/** Requires one exact lowercase Git source revision. */
function requireSourceSha(sourceSha) {
  if (typeof sourceSha !== "string" || !SOURCE_SHA_PATTERN.test(sourceSha)) {
    throw new Error("The Linux shipping-containment source revision is invalid.");
  }
}

/** Removes signing credentials and host loader overrides from verification and proof subprocesses. */
export function credentialFreeLinuxEnvironment(environment) {
  return Object.fromEntries(Object.entries(environment).filter(([name]) => !PROTECTED_ENVIRONMENT_PATTERN.test(name)));
}

/** Returns the two public-trust commands used to verify an already signed protected DEB. */
export function linuxShippingVerificationPlan(repositoryRoot, proofRoot, debPath) {
  const repository = resolve(repositoryRoot);
  const proof = resolve(proofRoot);
  const keyrings = join(proof, "keyrings");
  const policies = join(proof, "policies");
  const publicKeyring = join(keyrings, PUBLISHED_FINGERPRINT, "bottie.gpg");
  return [
    {
      arguments: [
        "--batch",
        "--yes",
        "--dearmor",
        "--output",
        publicKeyring,
        join(repository, "distribution", "linux", "bottie-linux-signing-public.asc"),
      ],
      command: "/usr/bin/gpg",
      label: "published Linux public key",
    },
    {
      arguments: verificationArguments(policies, keyrings, resolve(debPath)),
      command: "debsig-verify",
      label: "signed Linux protected package",
    },
  ];
}

/** Requires the fixed installed helper and runtime to equal the exact extracted protected package. */
export function requireMatchingLinuxProtectedInspections(expected, installed) {
  const supplied = validateProtectedPackageInspection("linux", expected);
  const observed = validateProtectedPackageInspection("linux", installed);
  if (protectedInspectionSha256(supplied) !== protectedInspectionSha256(observed)) {
    throw new Error("The installed Linux package does not match the signed protected package inspection.");
  }
  return observed;
}

/** Validates the complete closed native proof object without accepting added diagnostics. */
function validateInstalledLinuxProof(proof) {
  const expectedProofKeys = ["status", ...NATIVE_PROOF_FIELDS].sort();
  const proofKeys = proof && typeof proof === "object" && !Array.isArray(proof) ? Object.keys(proof).sort() : [];
  if (
    JSON.stringify(proofKeys) !== JSON.stringify(expectedProofKeys) ||
    proof.status !== "ok" ||
    !NATIVE_PROOF_FIELDS.every((field) => proof[field] === true)
  ) {
    throw new Error("The installed Linux native proof is incomplete.");
  }
  return proof;
}

/** Parses the installed verifier's bounded JSON without reflecting malformed output. */
export function parseInstalledLinuxProof(output) {
  let proof;
  try {
    proof = JSON.parse(output.trim());
  } catch {
    throw new Error("The installed Linux native proof returned invalid JSON.");
  }
  return validateInstalledLinuxProof(proof);
}

/** Creates the closed path-free shipping record after every installed Linux proof has passed. */
export function linuxShippingContainmentRecord(sourceSha, inspection, proof) {
  requireSourceSha(sourceSha);
  const accepted = validateProtectedPackageInspection("linux", inspection);
  const nativeProof = validateInstalledLinuxProof(proof);
  return {
    ...nativeProof,
    installedProtectedPackage: true,
    inspectionSha256: protectedInspectionSha256(accepted),
    platform: "linux",
    schemaVersion: 1,
    sourceSha,
    target: accepted.target,
  };
}

/** Requires one ordinary DEB without following a symbolic link or exposing its path. */
async function requireDeb(path) {
  try {
    const status = await lstat(path);
    if (status.isFile() && !status.isSymbolicLink() && path.toLowerCase().endsWith(".deb")) return;
  } catch {
    // Collapse absent and invalid inputs into one path-free failure.
  }
  throw new Error("The signed Linux protected package is unavailable.");
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

/** Runs one bounded command while discarding paths, public identity output, and host diagnostics. */
function runHostCommand(command, arguments_, environment, timeout = PROOF_TIMEOUT_MS) {
  const result = spawnSync(command, arguments_, {
    encoding: "utf8",
    env: environment,
    maxBuffer: MAX_CAPTURED_OUTPUT_BYTES,
    timeout,
  });
  if (result.error || result.status !== 0) throw new Error("A Linux shipping-containment command failed.");
  return result.stdout?.trim() ?? "";
}

/** Prepares transient public trust roots without modifying system verification configuration. */
async function preparePublicTrust(repository, proofRoot) {
  const fingerprintRoot = (kind) => join(proofRoot, kind, PUBLISHED_FINGERPRINT);
  await mkdir(fingerprintRoot("keyrings"), { recursive: true, mode: 0o700 });
  await mkdir(fingerprintRoot("policies"), { recursive: true, mode: 0o700 });
  await copyFile(
    join(repository, "distribution", "linux", "bottie.pol"),
    join(fingerprintRoot("policies"), "bottie.pol"),
  );
}

/** Verifies, extracts, reinspects, and exercises one already installed signed protected DEB. */
export async function proveLinuxShippingContainment(repository, sourceSha, debPath, candidate) {
  if (process.platform !== "linux") throw new Error("The Linux shipping-containment proof requires Linux.");
  requireSourceSha(sourceSha);
  const deb = resolve(debPath);
  await requireDeb(deb);
  const temporary = await mkdtemp(join(tmpdir(), "bottie-linux-shipping-python-proof-"));
  const extracted = join(temporary, "extracted");
  const gnupgHome = join(temporary, "gnupg");
  const environment = {
    ...credentialFreeLinuxEnvironment(process.env),
    DEBSIG_GNUPG_PROGRAM: "/usr/bin/gpg",
    GNUPGHOME: gnupgHome,
  };
  try {
    await mkdir(extracted);
    await mkdir(gnupgHome, { mode: 0o700 });
    await preparePublicTrust(repository, temporary);
    for (const step of linuxShippingVerificationPlan(repository, temporary, deb)) {
      runHostCommand(step.command, step.arguments, environment);
    }
    runHostCommand("dpkg-deb", ["--extract", deb, extracted], environment);
    const manifest = validateRuntimeManifest(
      await readJson(join(repository, "python-runner", "runtime-manifest.json"), "The Python runtime manifest"),
    );
    const inspection = validateProtectedPythonInspection(
      sourceSha,
      "linux",
      candidate,
      await inspectPackagedPythonBundle(extracted, "linux", manifest),
    );
    requireMatchingLinuxProtectedInspections(inspection, await inspectPackagedPythonBundle("/", "linux", manifest));
    const proof = parseInstalledLinuxProof(
      runHostCommand(
        process.execPath,
        [join(repository, "scripts", "linux-python-containment.mjs"), "--prove-installed"],
        environment,
      ),
    );
    return { containment: linuxShippingContainmentRecord(sourceSha, inspection, proof), inspection };
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

/** Dispatches the single credential-free proof mode for an already signed and installed protected DEB. */
async function main() {
  const repository = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const [mode, ...arguments_] = process.argv.slice(2);
  if (mode !== "--prove" || arguments_.length !== 5) {
    throw new Error(
      "Use --prove with source revision, signed DEB, candidate evidence, inspection output, and containment output.",
    );
  }
  const [sourceSha, debPath, candidatePath, inspectionPath, containmentPath] = arguments_;
  const result = await proveLinuxShippingContainment(
    repository,
    sourceSha,
    debPath,
    await readJson(resolve(candidatePath), "The Python release-candidate evidence"),
  );
  await writeJson(resolve(inspectionPath), result.inspection);
  await writeJson(resolve(containmentPath), result.containment);
  console.log("[bottie] Credential-free Linux shipping containment accepted.");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`[bottie] ${error instanceof Error ? error.message : "The Linux containment proof failed."}`);
    process.exitCode = 1;
  });
}
