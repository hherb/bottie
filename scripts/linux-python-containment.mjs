#!/usr/bin/env node

/** Builds and exercises Bottie's development-only Linux Python containment proof. */

import { spawn, spawnSync } from "node:child_process";
import { access, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const BUILD_TIMEOUT_MS = 10 * 60_000;
const PROOF_TIMEOUT_MS = 180_000;
const CANCELLATION_DELAY_MS = 5_000;
const PARENT_EXIT_TIMEOUT_MS = 5_000;
const PARENT_EXIT_POLL_MS = 50;
const MAX_CAPTURED_OUTPUT_BYTES = 256 * 1_024;
const ORDINARY_REQUEST = JSON.stringify({ code: "print(6 * 7)", purpose: "Prove private-pipe execution" });
const CANCELLATION_REQUEST = JSON.stringify({ code: "while True:\n    pass", purpose: "Prove caller cancellation" });
const REQUIRED_EVIDENCE = [
  "environmentIsolated",
  "execDenied",
  "landlockDeniedHostFixture",
  "networkDenied",
  "parentDeathSignal",
  "processCreationDenied",
  "resourceLimits",
  "runtimeReadable",
  "workspaceReadable",
];

/** Marks one deliberately bounded path-free diagnostic. */
export class ProofFailure extends Error {}

/** Keeps unexpected host errors from exposing paths or process details. */
export function safeProofFailure(error) {
  return error instanceof ProofFailure ? error.message : "The containment proof failed.";
}

/** Returns the locked release build arguments for the unchanged Rust runner. */
export function runnerBuildArguments(manifest) {
  return ["build", "--manifest-path", manifest, "--release", "--locked"];
}

/** Parses the exact path-free evidence contract and rejects added fields. */
export function parseContainmentEvidence(output) {
  let evidence;
  try {
    evidence = JSON.parse(output.trim());
  } catch {
    throw new ProofFailure("The native proof returned invalid JSON.");
  }
  const expectedKeys = ["status", ...REQUIRED_EVIDENCE].sort();
  const observedKeys =
    evidence && typeof evidence === "object" && !Array.isArray(evidence) ? Object.keys(evidence).sort() : [];
  if (
    observedKeys.length !== expectedKeys.length ||
    !observedKeys.every((key, index) => key === expectedKeys[index]) ||
    evidence.status !== "ok" ||
    !REQUIRED_EVIDENCE.every((key) => evidence[key] === true)
  ) {
    throw new ProofFailure("The native containment evidence was incomplete.");
  }
  return evidence;
}

function runHostCommand(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    encoding: "utf8",
    env: options.env,
    input: options.input,
    maxBuffer: MAX_CAPTURED_OUTPUT_BYTES,
    timeout: options.timeout ?? PROOF_TIMEOUT_MS,
  });
  if (result.error || result.status !== 0) {
    throw new ProofFailure(`${options.label ?? "The Linux proof command"} failed.`);
  }
  return result.stdout ?? "";
}

function parseExecutionResult(output) {
  try {
    const result = JSON.parse(output.trim());
    if (result?.status === "ok" && result.stdout?.trim() === "42") return result;
  } catch {
    // The fixed failure below intentionally drops raw output.
  }
  throw new ProofFailure("The contained runner did not return the expected bounded result.");
}

async function validateRuntime(runtime) {
  for (const required of ["python.wasm", "lib/python3.14/os.py", "LICENSE"]) {
    await access(resolve(runtime, required));
  }
}

function compileRunner(repository) {
  const manifest = resolve(repository, "python-runner", "Cargo.toml");
  runHostCommand("cargo", runnerBuildArguments(manifest), {
    label: "The locked Python runner build",
    timeout: BUILD_TIMEOUT_MS,
  });
  return resolve(repository, "python-runner", "target", "release", "bottie-python-runner");
}

function privateEnvironment(workspace) {
  return { TMPDIR: workspace };
}

function containedArguments(runtime, fixture) {
  const arguments_ = ["--linux-contained", "--runtime", runtime];
  if (fixture) arguments_.push("--linux-containment-probe", fixture);
  return arguments_;
}

async function cancelRunningChild(runner, runtime, workspace) {
  const child = spawn(runner, containedArguments(runtime), {
    env: privateEnvironment(workspace),
    stdio: ["pipe", "pipe", "pipe"],
  });
  child.stdin.end(CANCELLATION_REQUEST);
  await new Promise((resolvePromise) => setTimeout(resolvePromise, CANCELLATION_DELAY_MS));
  child.kill("SIGKILL");
  const outcome = await waitForChild(child, PROOF_TIMEOUT_MS);
  return outcome.signal === "SIGKILL";
}

function waitForChild(child, timeout) {
  return new Promise((resolvePromise, reject) => {
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      reject(new ProofFailure("The contained runner exceeded its outer timeout."));
    }, timeout);
    child.once("error", () => {
      clearTimeout(timer);
      reject(new ProofFailure("The contained runner could not be started."));
    });
    child.once("exit", (code, signal) => {
      clearTimeout(timer);
      resolvePromise({ code, signal });
    });
  });
}

/** Waits for kill-on-parent-close without sending a signal from the verifier. */
export async function waitForProcessExit(processIdentifier) {
  const deadline = Date.now() + PARENT_EXIT_TIMEOUT_MS;
  while (Date.now() < deadline) {
    try {
      process.kill(processIdentifier, 0);
    } catch (error) {
      if (error?.code === "ESRCH") return true;
      throw error;
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, PARENT_EXIT_POLL_MS));
  }
  return false;
}

async function proveParentClose(script, runner, runtime, workspace) {
  const parent = spawn(process.execPath, [script, "--parent-close-child", runner, runtime, workspace], {
    env: {},
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  parent.stdout.setEncoding("utf8");
  parent.stdout.on("data", (chunk) => {
    output += chunk;
  });
  const outcome = await waitForChild(parent, PROOF_TIMEOUT_MS);
  const processIdentifier = Number.parseInt(output.trim(), 10);
  if (outcome.code !== 0 || !Number.isSafeInteger(processIdentifier) || processIdentifier <= 1) return false;
  return waitForProcessExit(processIdentifier);
}

async function runParentCloseChild(runner, runtime, workspace) {
  const child = spawn(runner, [...containedArguments(runtime), "--linux-parent-close-proof"], {
    env: privateEnvironment(workspace),
    stdio: ["pipe", "pipe", "pipe"],
  });
  child.stdin.end(ORDINARY_REQUEST);
  await new Promise((resolvePromise, reject) => {
    let output = "";
    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      output += chunk;
      if (!output.includes("\n")) return;
      try {
        parseExecutionResult(output);
        resolvePromise();
      } catch (error) {
        reject(error);
      }
    });
    child.once("error", reject);
  });
  process.stdout.write(`${child.pid}\n`);
  child.stdout.destroy();
  child.stderr.destroy();
  child.unref();
}

async function prove() {
  if (process.platform !== "linux") throw new ProofFailure("The Linux containment proof requires Linux.");
  const repository = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const runtime = process.env.BOTTIE_PYTHON_WASI_RUNTIME;
  if (!runtime) throw new ProofFailure("The checksum-verified Python runtime is not configured.");
  await validateRuntime(runtime);
  const runner = compileRunner(repository);
  const temporary = await mkdtemp(resolve(tmpdir(), "bottie-linux-python-proof-"));
  const fixture = resolve(temporary, "host-owned-fixture");
  const workspace = resolve(temporary, "workspace");
  await writeFile(fixture, "host-owned proof fixture\n", { mode: 0o600 });
  await mkdir(workspace, { mode: 0o700 });
  try {
    const evidence = parseContainmentEvidence(
      runHostCommand(runner, containedArguments(runtime, fixture), {
        env: privateEnvironment(workspace),
        input: ORDINARY_REQUEST,
        label: "The native containment probe",
      }),
    );
    parseExecutionResult(
      runHostCommand(runner, containedArguments(runtime), {
        env: privateEnvironment(workspace),
        input: ORDINARY_REQUEST,
        label: "The private-pipe execution proof",
      }),
    );
    const cancellation = await cancelRunningChild(runner, runtime, workspace);
    const parentCloseKilledRunner = await proveParentClose(fileURLToPath(import.meta.url), runner, runtime, workspace);
    if (!cancellation || !parentCloseKilledRunner) {
      throw new ProofFailure("The process-lifecycle containment evidence was incomplete.");
    }
    process.stdout.write(`${JSON.stringify({ ...evidence, cancellation, parentCloseKilledRunner })}\n`);
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

if (process.argv[2] === "--parent-close-child") {
  runParentCloseChild(process.argv[3], process.argv[4], process.argv[5]).catch(() => {
    process.exitCode = 1;
  });
} else if (process.argv[2] === "--prove") {
  prove().catch((error) => {
    process.stderr.write(`${safeProofFailure(error)}\n`);
    process.exitCode = 1;
  });
}
