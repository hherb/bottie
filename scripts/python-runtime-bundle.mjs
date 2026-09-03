#!/usr/bin/env node

/** Builds and verifies Bottie's development-only CPython/WASI package inputs. */

import { createHash } from "node:crypto";
import { cp, lstat, mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { basename, dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const REPOSITORY_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const MANIFEST_PATH = join(REPOSITORY_ROOT, "python-runner", "runtime-manifest.json");
const PYTHON_LIBRARY_DIRECTORY = "python3.14";
const RUNTIME_DIRECTORY = "python-runtime";
const EVIDENCE_FILENAME = "python-runtime-evidence.json";
const SIDECAR_BASENAME = "bottie-python-runner";
const MAX_RUNTIME_FILES = 4_096;
const MAX_RUNTIME_BYTES = 128 * 1024 * 1024;
const SHA256_PATTERN = /^[a-f0-9]{64}$/;
const REVIEWED_URLS = {
  archive:
    "https://github.com/brettcannon/cpython-wasi-build/releases/download/v3.14.7/" + "python-3.14.7-wasi_sdk-24.zip",
  sbom: "https://www.python.org/ftp/python/3.14.7/Python-3.14.7.tar.xz.spdx.json",
  source: "https://www.python.org/ftp/python/3.14.7/Python-3.14.7.tar.xz",
  wasiSdk:
    "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-24/" + "wasi-sdk-24.0-x86_64-linux.tar.gz",
  wasmtime:
    "https://github.com/bytecodealliance/wasmtime/releases/download/v45.0.3/" + "wasmtime-v45.0.3-x86_64-linux.tar.xz",
};
const REVIEWED_REQUIRED_PATHS = ["python.wasm", "lib/python3.14/os.py", "LICENSE"];
const REVIEWED_STRIPPED_PATHS = [
  "curses",
  "ctypes/test",
  "ensurepip",
  "distutils",
  "lib2to3",
  "idlelib",
  "test",
  "multiprocessing",
  "tkinter",
  "turtledemo",
  "venv",
  "unittest/test",
];
const SUPPORTED_TARGETS = new Set([
  "aarch64-apple-darwin",
  "x86_64-apple-darwin",
  "x86_64-pc-windows-msvc",
  "x86_64-unknown-linux-gnu",
]);
const PACKAGED_LAYOUTS = {
  linux: {
    evidence: `usr/lib/bottie/${EVIDENCE_FILENAME}`,
    runtime: `usr/lib/bottie/${RUNTIME_DIRECTORY}`,
    sidecar: `usr/bin/${SIDECAR_BASENAME}`,
    targetSuffix: "-unknown-linux-gnu",
  },
  macos: {
    evidence: `Contents/Resources/${EVIDENCE_FILENAME}`,
    runtime: `Contents/Resources/${RUNTIME_DIRECTORY}`,
    sidecar: `Contents/MacOS/${SIDECAR_BASENAME}`,
    targetSuffix: "-apple-darwin",
  },
  windows: {
    evidence: EVIDENCE_FILENAME,
    runtime: RUNTIME_DIRECTORY,
    sidecar: `${SIDECAR_BASENAME}.exe`,
    targetSuffix: "-pc-windows-msvc",
  },
};

/** Returns one lowercase SHA-256 digest for bytes or text. */
function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

/** Requires one positive byte count and one lowercase SHA-256 digest. */
function validateImmutableInput(input, label) {
  if (!input || !URL.canParse(input.url) || !Number.isSafeInteger(input.bytes) || input.bytes <= 0) {
    throw new Error(`${label} must have one immutable URL and positive byte count.`);
  }
  if (!SHA256_PATTERN.test(input.sha256)) throw new Error(`${label} must have one lowercase SHA-256 digest.`);
}

/** Validates the closed source, build-tool, compatibility, and layout contract. */
export function validateRuntimeManifest(manifest) {
  if (manifest?.schemaVersion !== 2) throw new Error("The Python runtime manifest schema is unsupported.");
  if (manifest.pythonVersion !== "3.14.7") throw new Error("The reviewed CPython version is required.");
  if (manifest.wasiSdkVersion !== "24" || manifest.wasmtimeVersion !== "45.0.3") {
    throw new Error("The reviewed WASI SDK and Wasmtime versions are required.");
  }
  validateImmutableInput(manifest.source, "The official Python source");
  if (manifest.source.url !== REVIEWED_URLS.source || manifest.source.sbom?.url !== REVIEWED_URLS.sbom) {
    throw new Error("The reviewed official Python source and SBOM are required.");
  }
  if (!SHA256_PATTERN.test(manifest.source.licenceSha256)) {
    throw new Error("The official Python source licence digest is required.");
  }
  validateImmutableInput(manifest.source.sbom, "The official Python source SBOM");
  validateImmutableInput(manifest.buildTools?.wasiSdkLinuxX64, "The WASI SDK build input");
  validateImmutableInput(manifest.buildTools?.wasmtimeLinuxX64, "The Wasmtime build input");
  validateImmutableInput(manifest.archive, "The compatibility archive");
  if (
    manifest.buildTools.wasiSdkLinuxX64.url !== REVIEWED_URLS.wasiSdk ||
    manifest.buildTools.wasmtimeLinuxX64.url !== REVIEWED_URLS.wasmtime ||
    manifest.archive.url !== REVIEWED_URLS.archive
  ) {
    throw new Error("The reviewed build-tool and compatibility inputs are required.");
  }
  if (manifest.archive.role !== "compatibility-test-input") {
    throw new Error("The unofficial archive must remain compatibility-test input only.");
  }
  if (
    manifest.build?.target !== "wasm32-wasip1" ||
    manifest.build.sourceDateEpoch !== 1_785_888_000 ||
    JSON.stringify(manifest.build.command) !== JSON.stringify(["python3", "Tools/wasm/wasi", "build"])
  ) {
    throw new Error("The reviewed deterministic CPython/WASI build recipe is required.");
  }
  for (const key of ["requiredPaths", "build.strippedPaths"]) {
    const value = key === "requiredPaths" ? manifest.requiredPaths : manifest.build.strippedPaths;
    if (!Array.isArray(value) || value.length === 0 || value.some((path) => !isSafeRelativePath(path))) {
      throw new Error(`${key} must contain safe relative paths.`);
    }
  }
  if (
    JSON.stringify(manifest.requiredPaths) !== JSON.stringify(REVIEWED_REQUIRED_PATHS) ||
    JSON.stringify(manifest.build.strippedPaths) !== JSON.stringify(REVIEWED_STRIPPED_PATHS)
  ) {
    throw new Error("The reviewed runtime layout and stripping policy are required.");
  }
  return manifest;
}

/** Reads one regular file without following a symbolic link. */
async function readRegularFile(path, label) {
  const status = await lstat(path);
  if (!status.isFile() || status.isSymbolicLink()) throw new Error(`${label} must be a regular file.`);
  return readFile(path);
}

/** Accepts only normalized relative paths that cannot escape their payload root. */
function isSafeRelativePath(path) {
  return (
    typeof path === "string" &&
    path.length > 0 &&
    !path.startsWith("/") &&
    !path.includes("\\") &&
    path.split("/").every((part) => part && part !== "." && part !== "..")
  );
}

/** Walks one payload without following symbolic links or returning host paths. */
async function collectFiles(directory, root = directory, state = { files: [], totalBytes: 0 }) {
  const entries = await readdir(directory, { withFileTypes: true });
  entries.sort((left, right) => left.name.localeCompare(right.name));
  for (const entry of entries) {
    const absolutePath = join(directory, entry.name);
    const path = relative(root, absolutePath).split(sep).join("/");
    if (entry.isSymbolicLink()) throw new Error("The Python runtime must not contain a symbolic link.");
    if (entry.isDirectory()) {
      await collectFiles(absolutePath, root, state);
      continue;
    }
    if (!entry.isFile()) throw new Error("The Python runtime contains an unsupported filesystem entry.");
    const metadata = await lstat(absolutePath);
    state.totalBytes += metadata.size;
    if (state.totalBytes > MAX_RUNTIME_BYTES) throw new Error("The Python runtime byte count exceeds its bound.");
    const bytes = await readFile(absolutePath);
    state.files.push({ bytes: bytes.length, path, sha256: sha256(bytes) });
    if (state.files.length > MAX_RUNTIME_FILES) throw new Error("The Python runtime file count exceeds its bound.");
  }
  return state.files;
}

/** Produces bounded path-free evidence for one exact CPython/WASI runtime tree. */
export async function inspectPythonRuntime(runtimeRoot, uncheckedManifest) {
  const manifest = validateRuntimeManifest(uncheckedManifest);
  const root = resolve(runtimeRoot);
  const status = await lstat(root);
  if (!status.isDirectory()) throw new Error("The Python runtime input must be a directory.");
  const files = await collectFiles(root);
  const paths = new Set(files.map((file) => file.path));
  for (const path of manifest.requiredPaths) {
    if (!paths.has(path)) throw new Error("The Python runtime is missing a required file.");
  }
  for (const path of manifest.build.strippedPaths) {
    const prefix = `lib/${PYTHON_LIBRARY_DIRECTORY}/${path}`;
    if ([...paths].some((candidate) => candidate === prefix || candidate.startsWith(`${prefix}/`))) {
      throw new Error("The Python runtime contains a stripped development path.");
    }
  }
  if ([...paths].some((path) => path.includes("/__pycache__/") || path.endsWith(".pyc"))) {
    throw new Error("The Python runtime contains generated bytecode.");
  }
  const totalBytes = files.reduce((total, file) => total + file.bytes, 0);
  if (totalBytes > MAX_RUNTIME_BYTES) throw new Error("The Python runtime byte count exceeds its bound.");
  const licence = files.find((file) => file.path === "LICENSE");
  if (licence.sha256 !== manifest.source.licenceSha256) {
    throw new Error("The Python runtime licence does not match the official source.");
  }
  const digest = createHash("sha256");
  for (const file of files) digest.update(`${file.path}\0${file.bytes}\0${file.sha256}\0`);
  return {
    schemaVersion: 1,
    fileCount: files.length,
    licenceSha256: licence.sha256,
    pythonVersion: manifest.pythonVersion,
    pythonWasmSha256: files.find((file) => file.path === "python.wasm").sha256,
    runtimeTreeSha256: digest.digest("hex"),
    totalBytes,
    wasiSdkVersion: manifest.wasiSdkVersion,
  };
}

/** Finds exactly one generated CPython sysconfig module below the WASI build directory. */
async function findSysconfigModule(directory, matches = []) {
  const entries = await readdir(directory, { withFileTypes: true });
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) await findSysconfigModule(path, matches);
    if (entry.isFile() && /^_sysconfigdata_.*\.py$/.test(entry.name)) matches.push(path);
  }
  return matches;
}

/** Stages the reviewed runtime layout from one completed official CPython source build. */
export async function stageBuiltPythonRuntime(sourceRoot, outputRoot, uncheckedManifest) {
  const manifest = validateRuntimeManifest(uncheckedManifest);
  const output = resolve(outputRoot);
  await mkdir(output);
  const library = join(output, "lib", PYTHON_LIBRARY_DIRECTORY);
  await mkdir(library, { recursive: true });
  await cp(join(resolve(sourceRoot), "Lib"), library, { recursive: true });
  await cp(join(resolve(sourceRoot), "LICENSE"), join(output, "LICENSE"));
  const buildRoot = join(resolve(sourceRoot), "cross-build", manifest.build.target);
  await cp(join(buildRoot, "python.wasm"), join(output, "python.wasm"));
  const sysconfigModules = await findSysconfigModule(join(buildRoot, "build"));
  if (sysconfigModules.length !== 1)
    throw new Error("The CPython/WASI build must produce exactly one sysconfig module.");
  await cp(sysconfigModules[0], join(library, basename(sysconfigModules[0])));
  await mkdir(join(library, "lib-dynload"), { recursive: true });
  for (const path of manifest.build.strippedPaths) {
    await rm(join(library, ...path.split("/")), { recursive: true, force: true });
  }
  await removeBytecodeDirectories(output);
  return inspectPythonRuntime(output, manifest);
}

/** Removes build-generated bytecode while retaining authored Python sources. */
async function removeBytecodeDirectories(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory() && entry.name === "__pycache__") {
      await rm(path, { recursive: true, force: true });
    } else if (entry.isDirectory()) {
      await removeBytecodeDirectories(path);
    } else if (entry.isFile() && entry.name.endsWith(".pyc")) {
      await rm(path);
    }
  }
}

/** Returns Tauri's required target-suffixed source filename for one sidecar. */
export function sidecarSourceName(target) {
  if (!SUPPORTED_TARGETS.has(target)) throw new Error("The Python runner target is unsupported.");
  const extension = target.includes("windows") ? ".exe" : "";
  return `${SIDECAR_BASENAME}-${target}${extension}`;
}

/** Copies the exact runtime and native helper into an ignored Tauri bundle-input directory. */
export async function preparePythonBundleInputs({ manifest, outputRoot, runnerPath, runtimeRoot, target }) {
  validateRuntimeManifest(manifest);
  sidecarSourceName(target);
  const output = resolve(outputRoot);
  await mkdir(output);
  const runtimeDestination = join(output, RUNTIME_DIRECTORY);
  await cp(resolve(runtimeRoot), runtimeDestination, { recursive: true });
  const runtime = await inspectPythonRuntime(runtimeDestination, manifest);
  const runnerSource = resolve(runnerPath);
  const runner = await readRegularFile(runnerSource, "The Python runner input");
  const evidence = {
    schemaVersion: 1,
    manifestSha256: sha256(`${JSON.stringify(manifest)}\n`),
    runnerBytes: runner.length,
    runnerSha256: sha256(runner),
    runtime,
    target,
  };
  await cp(runnerSource, join(output, sidecarSourceName(target)));
  await writeFile(join(output, EVIDENCE_FILENAME), `${JSON.stringify(evidence, null, 2)}\n`);
  return evidence;
}

/** Verifies the exact development-only helper/runtime after platform package extraction. */
export async function inspectPackagedPythonBundle(packageRoot, platform, uncheckedManifest) {
  const manifest = validateRuntimeManifest(uncheckedManifest);
  const layout = PACKAGED_LAYOUTS[platform];
  if (!layout) throw new Error("The Python package platform is unsupported.");
  const root = resolve(packageRoot);
  const evidence = JSON.parse(
    (await readRegularFile(join(root, ...layout.evidence.split("/")), "The packaged Python evidence")).toString("utf8"),
  );
  if (
    evidence.schemaVersion !== 1 ||
    evidence.manifestSha256 !== sha256(`${JSON.stringify(manifest)}\n`) ||
    !SUPPORTED_TARGETS.has(evidence.target) ||
    !evidence.target.endsWith(layout.targetSuffix)
  ) {
    throw new Error("The packaged Python evidence does not match its manifest or platform.");
  }
  const runtime = await inspectPythonRuntime(join(root, ...layout.runtime.split("/")), manifest);
  const runner = await readRegularFile(join(root, ...layout.sidecar.split("/")), "The packaged Python runner");
  if (JSON.stringify(runtime) !== JSON.stringify(evidence.runtime)) {
    throw new Error("The packaged Python runtime does not match its build evidence.");
  }
  if (sha256(runner) !== evidence.runnerSha256 || runner.length !== evidence.runnerBytes) {
    throw new Error("The packaged Python runner does not match its build evidence.");
  }
  return { bundled: true, runnerBytes: runner.length, runnerSha256: sha256(runner), runtime, target: evidence.target };
}

/** Loads the checked-in runtime provenance manifest. */
async function loadManifest() {
  return validateRuntimeManifest(JSON.parse(await readFile(MANIFEST_PATH, "utf8")));
}

/** Dispatches runtime staging, sidecar preparation, and extracted-package inspection. */
async function main() {
  const [mode, ...arguments_] = process.argv.slice(2);
  const manifest = await loadManifest();
  let evidence;
  if (mode === "--stage-built" && arguments_.length === 2) {
    evidence = await stageBuiltPythonRuntime(arguments_[0], arguments_[1], manifest);
  } else if (mode === "--prepare" && arguments_.length === 4) {
    evidence = await preparePythonBundleInputs({
      manifest,
      outputRoot: arguments_[3],
      runnerPath: arguments_[2],
      runtimeRoot: arguments_[1],
      target: arguments_[0],
    });
  } else if (mode === "--inspect-package" && arguments_.length === 2) {
    evidence = await inspectPackagedPythonBundle(arguments_[1], arguments_[0], manifest);
  } else if (mode === "--inspect-runtime" && arguments_.length === 1) {
    evidence = await inspectPythonRuntime(arguments_[0], manifest);
  } else {
    throw new Error("Use --stage-built, --prepare, --inspect-package, or --inspect-runtime with exact inputs.");
  }
  console.log(JSON.stringify(evidence, null, 2));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    await main();
  } catch (error) {
    console.error(`[bottie] ${error instanceof Error ? error.message : "Python runtime preparation failed."}`);
    process.exitCode = 1;
  }
}
