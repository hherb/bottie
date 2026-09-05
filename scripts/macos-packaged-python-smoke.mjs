#!/usr/bin/env node

/** Ephemerally signs, inspects, and exercises Bottie's packaged macOS Python XPC transport. */

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { exerciseProof, productBundleLayout } from "./macos-python-xpc.mjs";
import { inspectPackagedPythonBundle, validateRuntimeManifest } from "./python-runtime-bundle.mjs";

const MAX_CAPTURED_OUTPUT_BYTES = 128 * 1_024;
const PROOF_TIMEOUT_MS = 45_000;
const SERVICE_IDENTIFIER = "com.bottie.python-runner";
const SIGNING_OPTIONS = ["--options", "runtime", "--timestamp=none"];
const SIGNING_IDENTITY_PATTERN = /^[A-F0-9]{40}$/;

/** Returns every exact Python transport path inside one packaged development app. */
export function packagedBundleLayout(applicationRoot) {
  const application = join(resolve(applicationRoot), "Contents", "Helpers", "BottiePythonXPCClient.app");
  const service = join(application, "Contents", "XPCServices", `${SERVICE_IDENTIFIER}.xpc`);
  return {
    application,
    applicationExecutable: join(application, "Contents", "MacOS", "bottie-python-xpc-client"),
    runner: join(service, "Contents", "Helpers", "bottie-python-runner"),
    runtime: join(service, "Contents", "Resources", "python-runtime"),
    service,
    serviceExecutable: join(service, "Contents", "MacOS", "bottie-python-xpc-service"),
  };
}

/** Returns the credential-free inside-out signing plan for already-staged product inputs. */
export function credentialFreeSigningPlan(repositoryRoot, layout, identity, keychain) {
  if (!SIGNING_IDENTITY_PATTERN.test(identity) || !keychain) {
    throw new Error("The ephemeral macOS signing identity is incomplete.");
  }
  const entitlementRoot = join(resolve(repositoryRoot), "macos-python-xpc");
  const clientApplication = layout.clientApplication ?? layout.application;
  const signingPrefix = ["--force", "--sign", identity, "--keychain", resolve(keychain), ...SIGNING_OPTIONS];
  return [
    {
      arguments: [...signingPrefix, "--entitlements", join(entitlementRoot, "Runner.entitlements"), layout.runner],
      path: layout.runner,
    },
    {
      arguments: [...signingPrefix, "--entitlements", join(entitlementRoot, "Service.entitlements"), layout.service],
      path: layout.service,
    },
    {
      arguments: [...signingPrefix, clientApplication],
      path: clientApplication,
    },
  ];
}

/** Classifies a bounded host-command failure without retaining its raw output. */
export function commandFailureKind(output) {
  const normalized = `${output ?? ""}`.toLowerCase();
  if (normalized.includes("errsecinternalcomponent")) return "keychain-access";
  if (normalized.includes("no identity found")) return "identity-unavailable";
  if (normalized.includes("timestamp service")) return "timestamp-unavailable";
  if (normalized.includes("resource fork") || normalized.includes("finder information")) {
    return "unexpected-metadata";
  }
  if (normalized.includes("bundle format")) return "invalid-bundle";
  return "unspecified";
}

/** Runs one host command without exposing arguments, identities, raw output, or paths in failures. */
function runHostCommand(command, arguments_, operation = "preparing or exercising the packaged containment proof") {
  const result = spawnSync(command, arguments_, {
    encoding: "utf8",
    maxBuffer: MAX_CAPTURED_OUTPUT_BYTES,
    timeout: PROOF_TIMEOUT_MS,
  });
  if (result.error || result.status !== 0) {
    const kind = commandFailureKind(`${result.error?.code ?? ""}\n${result.stderr ?? ""}`);
    throw new Error(`${basename(command)} failed (${kind}) while ${operation}.`);
  }
}

/** Returns a lowercase digest for exact signed runner bytes. */
function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

/** Updates only the staged evidence fields changed by ephemeral signing of the exact runner. */
async function refreshSignedRunnerEvidence(evidencePath, runnerPath) {
  const evidence = JSON.parse(await readFile(evidencePath, "utf8"));
  const runner = await readFile(runnerPath);
  evidence.runnerBytes = runner.length;
  evidence.runnerSha256 = sha256(runner);
  await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
}

/** Signs staged nested code with the workflow's temporary identity and no Apple credential or network timestamp. */
async function signProductInputs(repository, outputRoot, target) {
  if (process.platform !== "darwin") throw new Error("The macOS Python product inputs require a macOS host.");
  const layout = productBundleLayout(resolve(outputRoot), target);
  const [runner, service, clientApplication] = credentialFreeSigningPlan(
    repository,
    layout,
    process.env.BOTTIE_EPHEMERAL_SIGNING_IDENTITY,
    process.env.BOTTIE_EPHEMERAL_SIGNING_KEYCHAIN,
  );
  runHostCommand("codesign", runner.arguments, "signing the staged runner");
  await refreshSignedRunnerEvidence(layout.evidence, layout.runner);
  runHostCommand("codesign", service.arguments, "signing the staged XPC service");
  runHostCommand("codesign", clientApplication.arguments, "signing the staged XPC client");
  for (const path of [layout.runner, layout.service, layout.clientApplication]) {
    runHostCommand("codesign", ["--verify", "--strict", "--verbose=2", path]);
  }
}

/** Inspects and then exercises only the exact signed code and runtime inside one packaged app. */
async function provePackaged(repository, applicationRoot, inspectionOutput) {
  if (process.platform !== "darwin") throw new Error("The packaged XPC containment proof requires a macOS host.");
  const manifest = validateRuntimeManifest(
    JSON.parse(await readFile(join(repository, "python-runner", "runtime-manifest.json"), "utf8")),
  );
  const packagedApplication = resolve(applicationRoot);
  const layout = packagedBundleLayout(packagedApplication);
  const inspection = await inspectPackagedPythonBundle(packagedApplication, "macos", manifest);
  const output = resolve(inspectionOutput);
  await mkdir(dirname(output), { recursive: true });
  await writeFile(output, `${JSON.stringify(inspection, null, 2)}\n`);
  for (const path of [layout.runner, layout.service, layout.application]) {
    runHostCommand("codesign", ["--verify", "--strict", "--verbose=2", path]);
  }
  const fixtureDirectory = await mkdtemp(join(tmpdir(), "bottie-packaged-python-proof-"));
  try {
    await exerciseProof(layout, fixtureDirectory);
  } finally {
    await rm(fixtureDirectory, { recursive: true, force: true });
  }
  return {
    appSandboxDeniedHostFixture: true,
    cancellation: true,
    clientExitKilledRunner: true,
    credentialFreeEphemeralSignaturesVerified: true,
    inspectedPackagedBytes: true,
    privatePipeExecution: true,
    status: "ok",
  };
}

/** Dispatches staged-input signing or the exact packaged-app smoke. */
async function main() {
  const repository = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const [mode, ...arguments_] = process.argv.slice(2);
  if (mode === "--sign-inputs" && arguments_.length === 2) {
    await signProductInputs(repository, arguments_[1], arguments_[0]);
    return;
  }
  if (mode === "--prove" && arguments_.length === 2) {
    console.log(JSON.stringify(await provePackaged(repository, arguments_[0], arguments_[1])));
    return;
  }
  throw new Error("Use --sign-inputs or --prove with exact inputs.");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`[bottie] ${error instanceof Error ? error.message : "The packaged XPC proof failed."}`);
    process.exitCode = 1;
  });
}
