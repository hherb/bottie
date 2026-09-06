#!/usr/bin/env node

/** Stages and inspects an unsigned opt-in macOS package for protected Python validation. */

import { spawnSync } from "node:child_process";
import { cp, lstat, mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import { prepareProductBundle } from "./macos-python-xpc.mjs";
import { validateProtectedPythonInspection } from "./python-protected-package.mjs";
import { inspectPackagedPythonBundle, validateRuntimeManifest } from "./python-runtime-bundle.mjs";

const APPLICATION_PATH = "src-tauri/target/release/bundle/macos/bottie.app";
const DEVELOPMENT_INPUT_ROOT = "package/python-development";
const MAX_CAPTURED_OUTPUT_BYTES = 128 * 1_024;
const PROTECTED_INPUT_ROOT = "package/python-protected";
const PROTECTED_ENVIRONMENT_PATTERN = /^(?:APPLE_|TAURI_SIGNING_|BOTTIE_)/;
const RUNTIME_EVIDENCE = "python-runtime-evidence.json";
const RUNTIME_ROOT = "python-runtime";
const SUPPORTED_TARGETS = new Set(["aarch64-apple-darwin", "x86_64-apple-darwin"]);

/** Returns the exact unsigned Tauri build that composes updater and opt-in Python overlays. */
export function macosProtectedBuildArguments() {
  return [
    "build",
    "--bundles",
    "app",
    "--no-sign",
    "--ci",
    "--config",
    "src-tauri/tauri.updater.conf.json",
    "--config",
    "src-tauri/tauri.python-protected.macos.conf.json",
    "--",
    "--locked",
  ];
}

/** Removes every distribution, signing, notarization, and updater credential from child processes. */
export function credentialFreeMacosEnvironment(environment) {
  return Object.fromEntries(Object.entries(environment).filter(([name]) => !PROTECTED_ENVIRONMENT_PATTERN.test(name)));
}

/** Requires one ordinary file without following a symbolic link or exposing its path. */
async function requireRegularFile(path) {
  try {
    const status = await lstat(path);
    if (status.isFile() && !status.isSymbolicLink()) return;
  } catch {
    // Collapse absent and invalid inputs into one path-free failure.
  }
  throw new Error("The macOS protected Python staging input must be a regular file.");
}

/** Requires one ordinary directory without following a symbolic link or exposing its path. */
async function requireDirectory(path) {
  try {
    const status = await lstat(path);
    if (status.isDirectory() && !status.isSymbolicLink()) return;
  } catch {
    // Collapse absent and invalid inputs into one path-free failure.
  }
  throw new Error("The macOS protected Python runtime input must be a directory.");
}

/** Requires a runtime tree made only of ordinary directories and files. */
async function requirePlainTree(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (entry.isSymbolicLink() || (!entry.isDirectory() && !entry.isFile())) {
      throw new Error("The macOS protected Python runtime must contain only ordinary files and directories.");
    }
    if (entry.isDirectory()) await requirePlainTree(join(directory, entry.name));
  }
}

/** Rejects equal or nested roots before deleting the exact destination staging tree. */
function requireSeparateRoots(sourceRoot, outputRoot) {
  const source = resolve(sourceRoot);
  const output = resolve(outputRoot);
  const outputFromSource = relative(source, output);
  const sourceFromOutput = relative(output, source);
  const isNested = (value) => value === "" || (!value.startsWith(`..${sep}`) && value !== "..");
  if (isNested(outputFromSource) || isNested(sourceFromOutput)) {
    throw new Error("The macOS protected Python staging roots must be separate.");
  }
  return { output, source };
}

/** Copies only the reviewed unsigned runner, runtime, and evidence into a fresh staging root. */
export async function stageMacosProtectedInputs(sourceRoot, outputRoot, target) {
  if (!SUPPORTED_TARGETS.has(target)) throw new Error("The macOS protected Python target is unsupported.");
  const { output, source } = requireSeparateRoots(sourceRoot, outputRoot);
  const runner = `bottie-python-runner-${target}`;
  await requireRegularFile(join(source, runner));
  await requireRegularFile(join(source, RUNTIME_EVIDENCE));
  await requireDirectory(join(source, RUNTIME_ROOT));
  await requirePlainTree(join(source, RUNTIME_ROOT));
  await rm(output, { recursive: true, force: true });
  await mkdir(output, { recursive: true });
  await cp(join(source, runner), join(output, runner));
  await cp(join(source, RUNTIME_EVIDENCE), join(output, RUNTIME_EVIDENCE));
  await cp(join(source, RUNTIME_ROOT), join(output, RUNTIME_ROOT), {
    recursive: true,
    verbatimSymlinks: true,
  });
  return { evidence: RUNTIME_EVIDENCE, runner, runtime: RUNTIME_ROOT };
}

/** Runs one credential-free host command without retaining arguments, output, or paths. */
function runHostCommand(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    encoding: "utf8",
    maxBuffer: MAX_CAPTURED_OUTPUT_BYTES,
    ...options,
  });
  if (result.error || result.status !== 0) {
    throw new Error("A credential-free macOS protected-package command failed.");
  }
  return result.stdout?.trim() ?? "";
}

/** Reads one required JSON input without reflecting its host path. */
async function readJson(path) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch {
    throw new Error("Required macOS protected Python evidence is unavailable or malformed.");
  }
}

/** Writes one private path-free inspection document. */
async function writeJson(path, value) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
}

/** Builds and inspects the exact unsigned app without credentials or containment claims. */
export async function buildAndInspectMacosProtectedPackage(repository) {
  if (process.platform !== "darwin") throw new Error("The macOS protected Python producer requires macOS.");
  const environment = {
    ...credentialFreeMacosEnvironment(process.env),
    CFLAGS: "-mmacosx-version-min=14.0",
    CXXFLAGS: "-mmacosx-version-min=14.0",
    MACOSX_DEPLOYMENT_TARGET: "14.0",
  };
  const target = runHostCommand("rustc", ["--print", "host-tuple"], { cwd: repository, env: environment });
  const developmentRoot = join(repository, DEVELOPMENT_INPUT_ROOT);
  const protectedRoot = join(repository, PROTECTED_INPUT_ROOT);
  await stageMacosProtectedInputs(developmentRoot, protectedRoot, target);
  await prepareProductBundle(repository, protectedRoot, target, environment);
  const application = join(repository, APPLICATION_PATH);
  await rm(application, { recursive: true, force: true });
  const wrapper = join(repository, "scripts", "macos-development-signing.mjs");
  runHostCommand(process.execPath, [wrapper, "--tauri", ...macosProtectedBuildArguments()], {
    cwd: repository,
    env: environment,
    stdio: "inherit",
  });
  const runtimeManifest = validateRuntimeManifest(
    await readJson(join(repository, "python-runner", "runtime-manifest.json")),
  );
  return inspectPackagedPythonBundle(application, "macos", runtimeManifest);
}

/** Produces an unsigned app inspection accepted by the comparison contract, without containment claims. */
async function produceMacosProtectedInspection(repository, sourceSha, candidatePath, outputPath) {
  const inspection = await buildAndInspectMacosProtectedPackage(repository);
  const accepted = validateProtectedPythonInspection(sourceSha, "macos", await readJson(candidatePath), inspection);
  await writeJson(outputPath, accepted);
  return accepted;
}

/** Dispatches the single credential-free staging and inspection mode. */
async function main() {
  const [mode, ...arguments_] = process.argv.slice(2);
  if (mode !== "--produce" || arguments_.length !== 3) {
    throw new Error("Use --produce with source revision, candidate evidence, and inspection output.");
  }
  const repository = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  await produceMacosProtectedInspection(repository, arguments_[0], resolve(arguments_[1]), resolve(arguments_[2]));
  console.log("[bottie] Credential-free macOS protected Python inspection accepted.");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`[bottie] ${error instanceof Error ? error.message : "macOS protected Python staging failed."}`);
    process.exitCode = 1;
  });
}
