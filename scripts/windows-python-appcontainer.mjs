#!/usr/bin/env node

/** Builds and exercises Bottie's development-only Windows Python AppContainer containment proof. */

import { spawnSync } from "node:child_process";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, resolve, win32 } from "node:path";
import { fileURLToPath } from "node:url";

const PROOF_EXECUTABLE = "bottie-python-appcontainer.exe";
const RUNNER_EXECUTABLE = "bottie-python-runner.exe";
const BUILD_TIMEOUT_MS = 10 * 60_000;
const PROOF_TIMEOUT_MS = 180_000;
const PARENT_EXIT_TIMEOUT_MS = 5_000;
const PARENT_EXIT_POLL_MS = 50;
const MAX_CAPTURED_OUTPUT_BYTES = 256 * 1_024;
const ORDINARY_REQUEST = JSON.stringify({ code: "print(6 * 7)", purpose: "Prove private-pipe execution" });

/** Returns canonical locations owned by one transient AppContainer profile. */
export function proofProfileLayout(profile) {
  const root = win32.join(profile, "proof");
  return {
    host: win32.join(root, PROOF_EXECUTABLE),
    root,
    runner: win32.join(root, RUNNER_EXECUTABLE),
    runtime: win32.join(root, "python"),
  };
}

/** Returns the locked release build arguments for the unchanged Rust runner. */
export function runnerBuildArguments(manifest) {
  return ["build", "--manifest-path", manifest, "--release", "--locked"];
}

/** Returns fixed warning-clean MSVC arguments for the native proof host. */
export function msvcCompilationArguments(source, output) {
  return [
    "/nologo",
    "/std:c++20",
    "/EHsc",
    "/W4",
    "/WX",
    "/DUNICODE",
    "/D_UNICODE",
    source,
    `/Fe:${output}`,
    "advapi32.lib",
    "ole32.lib",
    "userenv.lib",
  ];
}

/** Keeps only bounded MSVC diagnostic codes and messages while removing host paths. */
export function safeMsvcDiagnostics(output) {
  const matches = output.match(/\b(?:fatal )?(?:error|warning) (?:C|LNK)\d{4,5}: [^\r\n]{1,240}/g) ?? [];
  return matches
    .slice(0, 4)
    .map((diagnostic) => diagnostic.replace(/[A-Za-z]:\\[^\s"']+/g, "[path]"))
    .join("; ");
}

/** Returns only a known path-free runner status for native-proof diagnostics. */
export function safeRunnerStatus(value) {
  const statuses = new Set([
    "internal_error",
    "invalid_request",
    "output_limit",
    "python_error",
    "resource_limit",
    "timed_out",
  ]);
  return statuses.has(value) ? value : "unexpected_result";
}

/** Requires the fixed successful result used by both baseline and contained execution. */
function requireOrdinaryResult(result, label) {
  if (result.status === "ok" && result.stdout.trim() === "42") return;
  const reason =
    result.status === "failed" && typeof result.reason === "string" ? result.reason : safeRunnerStatus(result.status);
  throw new Error(`${label} did not return the expected bounded result (${reason}).`);
}

/** Runs one native command without retaining its arguments or raw failure output. */
function runHostCommand(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    encoding: "utf8",
    input: options.input,
    maxBuffer: MAX_CAPTURED_OUTPUT_BYTES,
    timeout: options.timeout ?? PROOF_TIMEOUT_MS,
  });
  if (result.error || result.status !== 0) {
    const diagnostics = options.msvcDiagnostics
      ? safeMsvcDiagnostics(`${result.stdout ?? ""}\n${result.stderr ?? ""}`)
      : "";
    throw new Error(
      `${options.label ?? "The AppContainer proof command"} failed${diagnostics ? `: ${diagnostics}` : "."}`,
    );
  }
  return result.stdout ?? "";
}

/** Validates the already-extracted checksum-verified runtime without downloading it. */
async function validateRuntime(runtime) {
  for (const required of ["python.wasm", "lib/python3.14/os.py", "LICENSE"]) {
    if ((await readFile(resolve(runtime, required))).length === 0) {
      throw new Error("The configured CPython/WASI runtime is incomplete.");
    }
  }
}

/** Compiles the unchanged runner and native controller into a transient directory. */
function compileProof(repository, temporary) {
  const manifest = resolve(repository, "python-runner", "Cargo.toml");
  runHostCommand("cargo", runnerBuildArguments(manifest), {
    label: "The locked Python runner build",
    timeout: BUILD_TIMEOUT_MS,
  });
  const controller = resolve(temporary, PROOF_EXECUTABLE);
  runHostCommand(
    "cl.exe",
    msvcCompilationArguments(resolve(repository, "windows-python-appcontainer", "Proof.cpp"), controller),
    { label: "The native AppContainer controller build", msvcDiagnostics: true, timeout: BUILD_TIMEOUT_MS },
  );
  return { controller, runner: resolve(repository, "python-runner", "target", "release", RUNNER_EXECUTABLE) };
}

/** Parses one private path-bearing profile-preparation response. */
function parsePreparedProfile(output) {
  const response = JSON.parse(output.trim());
  if (!response || response.status !== "prepared" || typeof response.profilePath !== "string") {
    throw new Error("The AppContainer profile could not be prepared.");
  }
  if (!win32.isAbsolute(response.profilePath)) throw new Error("The AppContainer profile path was invalid.");
  return response.profilePath;
}

/** Parses one path-free proof result. */
function parseProofResult(output) {
  const response = JSON.parse(output.trim());
  if (!response || typeof response !== "object" || Array.isArray(response)) {
    throw new Error("The AppContainer proof returned an invalid result.");
  }
  return response;
}

/** Waits for kill-on-controller-close without sending a signal from the verifier. */
async function waitForProcessExit(processIdentifier) {
  const deadline = Date.now() + PARENT_EXIT_TIMEOUT_MS;
  while (Date.now() < deadline) {
    try {
      process.kill(processIdentifier, 0);
    } catch (error) {
      if (error?.code === "ESRCH") return;
      throw new Error("The proof could not inspect its isolated child process.");
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, PARENT_EXIT_POLL_MS));
  }
  throw new Error("The Job Object child survived its controller process.");
}

/** Exercises private pipes, cancellation, token/file denials, and kill-on-parent-close. */
async function exerciseProof(controller, moniker, layout, fixture) {
  const common = [moniker, layout.runner, layout.runtime];
  const probe = parseProofResult(
    runHostCommand(controller, ["probe", moniker, layout.host, fixture, layout.runtime], {
      label: "AppContainer denial probe",
    }),
  );
  const failedProbeChecks = [
    ["app_container", probe.appContainer],
    ["low_integrity", probe.lowIntegrity],
    ["privileges_stripped", probe.privilegesStripped],
    ["zero_capabilities", probe.capabilityCount === 0],
    ["host_fixture_denial", probe.hostFixtureDenied],
    ["runtime_read", probe.runtimeReadable],
  ]
    .filter(([, passed]) => passed !== true)
    .map(([name]) => name);
  const temporaryFailure = !probe.temporaryPathAvailable
    ? "temporary_path"
    : !probe.temporaryFileCreated
      ? `temporary_create_${Number.isSafeInteger(probe.temporaryCreateError) ? probe.temporaryCreateError : "unknown"}`
      : !probe.temporaryFileWritten
        ? "temporary_write"
        : !probe.temporaryFileDeleted
          ? "temporary_delete"
          : probe.temporaryWritable !== true
            ? "temporary_storage"
            : undefined;
  if (temporaryFailure) failedProbeChecks.push(temporaryFailure);
  if (probe.status !== "ok" || failedProbeChecks.length !== 0) {
    throw new Error(`The contained token or access probe failed (${failedProbeChecks.join(",")}).`);
  }

  const ordinary = parseProofResult(
    runHostCommand(controller, ["execute", ...common], { input: ORDINARY_REQUEST, label: "Private-pipe execution" }),
  );
  requireOrdinaryResult(ordinary, "Private-pipe execution");

  const infiniteRequest = JSON.stringify({ code: "while True:\n    pass", purpose: "Prove cancellation" });
  const cancelled = parseProofResult(
    runHostCommand(controller, ["cancel", ...common], { input: infiniteRequest, label: "Runner cancellation" }),
  );
  if (cancelled.status !== "cancelled") throw new Error("The Job Object did not cancel its runner.");

  const parent = parseProofResult(
    runHostCommand(controller, ["start-and-exit", ...common], {
      input: infiniteRequest,
      label: "Kill-on-controller-close proof",
    }),
  );
  if (parent.status !== "started" || !Number.isSafeInteger(parent.pid) || parent.pid <= 0) {
    throw new Error("The parent-close proof returned an invalid child identifier.");
  }
  await waitForProcessExit(parent.pid);
}

/** Builds and runs one transient credential-free proof without changing Bottie's product path. */
async function prove() {
  if (process.platform !== "win32") throw new Error("The AppContainer containment proof requires Windows.");
  const repository = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const runtimeValue = process.env.BOTTIE_PYTHON_WASI_RUNTIME;
  if (!runtimeValue) throw new Error("Set BOTTIE_PYTHON_WASI_RUNTIME to the checksum-verified extracted runtime.");
  const runtime = resolve(runtimeValue);
  await validateRuntime(runtime);
  const temporary = await mkdtemp(resolve(tmpdir(), "bottie-python-appcontainer-proof-"));
  const moniker = `bottie.python.runner.proof.${process.pid}`;
  let controller;
  let prepared = false;
  try {
    const compiled = compileProof(repository, temporary);
    controller = compiled.controller;
    const baseline = parseProofResult(
      runHostCommand(compiled.runner, ["--runtime", runtime], {
        input: ORDINARY_REQUEST,
        label: "The uncontained runner control",
      }),
    );
    requireOrdinaryResult(baseline, "The uncontained runner control");
    const profile = parsePreparedProfile(
      runHostCommand(controller, ["prepare", moniker], { label: "The transient AppContainer profile" }),
    );
    prepared = true;
    const layout = proofProfileLayout(profile);
    await mkdir(layout.root, { recursive: true });
    const builtRunner = resolve(repository, "python-runner", "target", "release", RUNNER_EXECUTABLE);
    await Promise.all([
      cp(controller, layout.host),
      cp(builtRunner, layout.runner),
      cp(runtime, layout.runtime, { recursive: true }),
    ]);
    const fixture = resolve(temporary, "host-owned-denial-fixture.txt");
    await writeFile(fixture, "AppContainer access must be denied.");
    await exerciseProof(controller, moniker, layout, fixture);
    console.log(
      JSON.stringify({
        appContainerDeniedHostFixture: true,
        appContainerLowIntegrity: true,
        appContainerNoCapabilities: true,
        cancellation: true,
        jobCloseKilledRunner: true,
        privatePipeExecution: true,
        resourceLimits: true,
        privilegesStripped: true,
        status: "ok",
      }),
    );
  } finally {
    try {
      if (prepared && controller) {
        runHostCommand(controller, ["cleanup", moniker], { label: "The transient AppContainer cleanup" });
      }
    } finally {
      await rm(temporary, { recursive: true, force: true });
    }
  }
}

/** Dispatches the sole native-proof mode. */
async function main() {
  if (process.argv.length !== 3 || process.argv[2] !== "--prove") {
    throw new Error("Use this script through npm run python:appcontainer:prove.");
  }
  await prove();
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`[bottie] ${error instanceof Error ? error.message : "The containment proof failed."}`);
    process.exitCode = 1;
  });
}
