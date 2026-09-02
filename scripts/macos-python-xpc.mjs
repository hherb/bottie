#!/usr/bin/env node

/** Builds and exercises Bottie's development-only macOS Python XPC containment proof. */

import { spawnSync } from "node:child_process";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { selectAppleDevelopmentIdentity } from "./macos-development-signing.mjs";

const APPLICATION_EXECUTABLE = "bottie-python-xpc-proof";
const APPLICATION_IDENTIFIER = "com.bottie.python-xpc-proof";
const RUNNER_EXECUTABLE = "bottie-python-runner";
const RUNNER_IDENTIFIER = "com.bottie.python-runner.inherited";
const SERVICE_EXECUTABLE = "bottie-python-xpc-service";
const SERVICE_IDENTIFIER = "com.bottie.python-runner";
const SIGNING_OPTIONS = ["--options", "runtime", "--timestamp=none"];
const SWIFT_MINIMUM_TARGET = "apple-macos14.0";
const PROOF_TIMEOUT_MS = 45_000;
const PARENT_EXIT_TIMEOUT_MS = 5_000;
const PARENT_EXIT_POLL_MS = 50;
const MAX_CAPTURED_OUTPUT_BYTES = 128 * 1_024;

/** Returns every canonical path in the transient proof bundle. */
export function proofBundleLayout(application) {
  const service = join(application, "Contents", "XPCServices", `${SERVICE_IDENTIFIER}.xpc`);
  return {
    application,
    applicationExecutable: join(application, "Contents", "MacOS", APPLICATION_EXECUTABLE),
    applicationInfo: join(application, "Contents", "Info.plist"),
    runner: join(service, "Contents", "Helpers", RUNNER_EXECUTABLE),
    runtime: join(service, "Contents", "Resources", "python"),
    service,
    serviceExecutable: join(service, "Contents", "MacOS", SERVICE_EXECUTABLE),
    serviceInfo: join(service, "Contents", "Info.plist"),
  };
}

/** Returns fixed Swift compiler arguments for one proof executable. */
export function swiftCompilationArguments(_kind, output, sources, architecture = "arm64") {
  return ["-parse-as-library", "-O", "-target", `${architecture}-${SWIFT_MINIMUM_TARGET}`, "-o", output, ...sources];
}

/** Returns exact inherited-runner signing arguments without recursive signing. */
export function runnerSigningArguments(identity, runner, entitlements) {
  return [
    "--force",
    "--sign",
    identity,
    "--identifier",
    RUNNER_IDENTIFIER,
    ...SIGNING_OPTIONS,
    "--entitlements",
    entitlements,
    runner,
  ];
}

/** Returns exact restricted-service signing arguments without recursive signing. */
export function serviceSigningArguments(identity, service, entitlements) {
  return ["--force", "--sign", identity, ...SIGNING_OPTIONS, "--entitlements", entitlements, service];
}

/** Returns exact outer-application signing arguments without recursive signing. */
export function applicationSigningArguments(identity, application) {
  return ["--force", "--sign", identity, ...SIGNING_OPTIONS, application];
}

/** Returns the fixed metadata for the private per-application XPC service. */
export function serviceBundleMetadata() {
  return {
    CFBundleExecutable: SERVICE_EXECUTABLE,
    CFBundleIdentifier: SERVICE_IDENTIFIER,
    CFBundleInfoDictionaryVersion: "6.0",
    CFBundleName: "Bottie Python Runner",
    CFBundlePackageType: "XPC!",
    CFBundleShortVersionString: "0.1.0",
    CFBundleVersion: "1",
    LSMinimumSystemVersion: "14.0",
    XPCService: { RunLoopType: "dispatch_main", ServiceType: "Application" },
  };
}

/** Returns fixed metadata for the otherwise inert development proof host. */
function applicationBundleMetadata() {
  return {
    CFBundleExecutable: APPLICATION_EXECUTABLE,
    CFBundleIdentifier: APPLICATION_IDENTIFIER,
    CFBundleInfoDictionaryVersion: "6.0",
    CFBundleName: "Bottie Python XPC Proof",
    CFBundlePackageType: "APPL",
    CFBundleShortVersionString: "0.1.0",
    CFBundleVersion: "1",
    LSMinimumSystemVersion: "14.0",
  };
}

/** Escapes one property-list string without accepting raw XML. */
function escapePlistString(value) {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

/** Encodes the small closed metadata value set used by proof bundles. */
function encodePlistValue(value) {
  if (typeof value === "string") return `<string>${escapePlistString(value)}</string>`;
  if (typeof value === "boolean") return value ? "<true/>" : "<false/>";
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return `<dict>${Object.entries(value)
      .map(([key, entry]) => `<key>${escapePlistString(key)}</key>${encodePlistValue(entry)}`)
      .join("")}</dict>`;
  }
  throw new Error("The proof bundle metadata contains an unsupported property-list value.");
}

/** Encodes one complete XML property list. */
function encodePlist(value) {
  return [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"',
    '  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">',
    '<plist version="1.0">',
    encodePlistValue(value),
    "</plist>",
    "",
  ].join("\n");
}

/** Runs one host command and never includes its arguments or raw output in failures. */
function runHostCommand(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    encoding: options.encoding ?? "utf8",
    env: options.env ?? process.env,
    input: options.input,
    maxBuffer: MAX_CAPTURED_OUTPUT_BYTES,
    timeout: options.timeout ?? PROOF_TIMEOUT_MS,
  });
  if (result.error || result.status !== 0) {
    throw new Error(`${command} failed while building or exercising the containment proof.`);
  }
  return { stdout: result.stdout ?? "", stderr: result.stderr ?? "" };
}

/** Selects the sole development identity while keeping its label out of evidence. */
function developmentIdentity() {
  const identities = runHostCommand("security", ["find-identity", "-v", "-p", "codesigning"]);
  return selectAppleDevelopmentIdentity(identities.stdout, process.env.BOTTIE_APPLE_SIGNING_IDENTITY);
}

/** Maps Node's supported macOS architecture names to Swift target triples. */
function swiftArchitecture() {
  if (process.arch === "arm64") return "arm64";
  if (process.arch === "x64") return "x86_64";
  throw new Error("The containment proof supports only Apple silicon or Intel macOS hosts.");
}

/** Validates the already-extracted runtime without downloading or changing it. */
async function validateRuntime(runtime) {
  for (const required of ["python.wasm", "lib/python3.14/os.py", "LICENSE"]) {
    const bytes = await readFile(join(runtime, required));
    if (bytes.length === 0) throw new Error("The configured CPython/WASI runtime is incomplete.");
  }
}

/** Creates the bundle directories and fixed metadata before compiling nested code. */
async function createBundle(layout) {
  for (const directory of [
    dirname(layout.applicationExecutable),
    dirname(layout.runner),
    layout.runtime,
    dirname(layout.serviceExecutable),
  ]) {
    await mkdir(directory, { recursive: true });
  }
  await writeFile(layout.applicationInfo, encodePlist(applicationBundleMetadata()));
  await writeFile(layout.serviceInfo, encodePlist(serviceBundleMetadata()));
}

/** Compiles the existing Rust runner and the two tiny Swift proof executables. */
function compileProof(repository, layout) {
  const sourceRoot = join(repository, "macos-python-xpc");
  const moduleCache = join(dirname(layout.application), "swift-module-cache");
  const swiftEnvironment = {
    ...process.env,
    CLANG_MODULE_CACHE_PATH: moduleCache,
    SWIFT_MODULECACHE_PATH: moduleCache,
  };
  runHostCommand("cargo", [
    "build",
    "--manifest-path",
    join(repository, "python-runner", "Cargo.toml"),
    "--release",
    "--locked",
    "--offline",
  ]);
  const architecture = swiftArchitecture();
  runHostCommand(
    "xcrun",
    [
      "swiftc",
      ...swiftCompilationArguments(
        "service",
        layout.serviceExecutable,
        [join(sourceRoot, "Shared.swift"), join(sourceRoot, "Service.swift")],
        architecture,
      ),
    ],
    { env: swiftEnvironment, timeout: PROOF_TIMEOUT_MS },
  );
  runHostCommand(
    "xcrun",
    [
      "swiftc",
      ...swiftCompilationArguments(
        "host",
        layout.applicationExecutable,
        [join(sourceRoot, "Shared.swift"), join(sourceRoot, "Host.swift")],
        architecture,
      ),
    ],
    { env: swiftEnvironment },
  );
}

/** Copies the exact helper and configured runtime into the transient proof bundle. */
async function copyRuntimeAndRunner(repository, runtime, layout) {
  const runner = join(repository, "python-runner", "target", "release", RUNNER_EXECUTABLE);
  await cp(runner, layout.runner);
  await rm(layout.runtime, { recursive: true, force: true });
  await cp(runtime, layout.runtime, { recursive: true, force: false, verbatimSymlinks: true });
}

/** Applies nested signatures inside out and verifies each code object independently. */
function signAndVerify(repository, layout) {
  const identity = developmentIdentity();
  const sourceRoot = join(repository, "macos-python-xpc");
  runHostCommand("codesign", runnerSigningArguments(identity, layout.runner, join(sourceRoot, "Runner.entitlements")));
  runHostCommand(
    "codesign",
    serviceSigningArguments(identity, layout.service, join(sourceRoot, "Service.entitlements")),
  );
  runHostCommand("codesign", applicationSigningArguments(identity, layout.application));
  for (const nestedCode of [layout.runner, layout.service, layout.application]) {
    runHostCommand("codesign", ["--verify", "--strict", "--verbose=2", nestedCode]);
  }
  runHostCommand("codesign", ["--verify", "--deep", "--strict", "--verbose=2", layout.application]);
}

/** Parses one path-free JSON object emitted by the proof host. */
function parseProofOutput(output) {
  const parsed = JSON.parse(output.trim());
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("The containment proof returned an invalid result.");
  }
  return parsed;
}

/** Returns whether one process identifier has already disappeared. */
function processHasExited(processIdentifier) {
  try {
    process.kill(processIdentifier, 0);
    return false;
  } catch (error) {
    if (error?.code === "ESRCH") return true;
    throw new Error("The containment proof could not inspect its isolated child process.");
  }
}

/** Waits for kill-on-client-exit without sending a signal from the verifier. */
async function waitForProcessExit(processIdentifier) {
  const deadline = Date.now() + PARENT_EXIT_TIMEOUT_MS;
  while (Date.now() < deadline) {
    if (processHasExited(processIdentifier)) return;
    await new Promise((resolvePromise) => setTimeout(resolvePromise, PARENT_EXIT_POLL_MS));
  }
  throw new Error("The XPC-owned runner survived its client process.");
}

/** Runs one proof-host mode with request bytes supplied only over stdin. */
function runProofHost(layout, mode, request, extraArguments = []) {
  return runHostCommand(layout.applicationExecutable, [mode, ...extraArguments], {
    input: request,
    timeout: PROOF_TIMEOUT_MS,
  }).stdout;
}

/** Exercises execution, cancellation, client-exit cleanup, and the outer sandbox denial. */
async function exerciseProof(layout, fixtureDirectory) {
  const ordinaryRequest = JSON.stringify({ code: "print(6 * 7)", purpose: "Prove private-pipe execution" });
  const ordinary = parseProofOutput(runProofHost(layout, "execute", ordinaryRequest));
  if (ordinary.status !== "ok" || ordinary.stdout.trim() !== "42") {
    throw new Error("Private-pipe execution did not return the expected bounded result.");
  }

  const infiniteRequest = JSON.stringify({ code: "while True:\n    pass", purpose: "Prove cancellation" });
  const cancelled = parseProofOutput(runProofHost(layout, "cancel", infiniteRequest));
  if (cancelled.status !== "cancelled") throw new Error("The XPC service did not cancel its runner.");

  const fixture = join(fixtureDirectory, "host-owned-denial-fixture.txt");
  await writeFile(fixture, "the restricted service must not read this fixture");
  const denied = parseProofOutput(runProofHost(layout, "probe", undefined, [fixture]));
  if (denied.status !== "denied") throw new Error("The XPC service could read the host-owned denial fixture.");

  const parent = parseProofOutput(runProofHost(layout, "start-and-exit", infiniteRequest));
  if (parent.status !== "started" || !Number.isSafeInteger(parent.pid) || parent.pid <= 0) {
    throw new Error("The parent-exit proof did not return a valid isolated child identifier.");
  }
  await waitForProcessExit(parent.pid);
}

/** Builds and runs one transient signed proof without changing Bottie's product bundle. */
async function prove() {
  if (process.platform !== "darwin") throw new Error("The XPC containment proof requires macOS.");
  const repository = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const runtimeValue = process.env.BOTTIE_PYTHON_WASI_RUNTIME;
  if (!runtimeValue) throw new Error("Set BOTTIE_PYTHON_WASI_RUNTIME to the checksum-verified extracted runtime.");
  const runtime = resolve(runtimeValue);
  await validateRuntime(runtime);
  const temporary = await mkdtemp(join(tmpdir(), "bottie-python-xpc-proof-"));
  const layout = proofBundleLayout(join(temporary, "BottiePythonXPCProof.app"));
  try {
    await createBundle(layout);
    compileProof(repository, layout);
    await copyRuntimeAndRunner(repository, runtime, layout);
    signAndVerify(repository, layout);
    await exerciseProof(layout, temporary);
    console.log(
      JSON.stringify({
        appSandboxDeniedHostFixture: true,
        cancellation: true,
        clientExitKilledRunner: true,
        nestedSignaturesVerified: true,
        privatePipeExecution: true,
        status: "ok",
      }),
    );
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

/** Dispatches the sole protected native-proof mode. */
async function main() {
  if (process.argv.length !== 3 || process.argv[2] !== "--prove") {
    throw new Error("Use this script through npm run python:xpc:prove.");
  }
  await prove();
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`[bottie] ${error instanceof Error ? error.message : "The containment proof failed."}`);
    process.exitCode = 1;
  });
}
