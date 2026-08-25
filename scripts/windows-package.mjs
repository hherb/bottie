#!/usr/bin/env node

/** Builds, inspects, and smoke-tests Bottie's bounded unsigned Windows MSI package. */

import { createHash } from "node:crypto";
import { spawn, spawnSync } from "node:child_process";
import { copyFile, lstat, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import { parseAuthenticodeEvidence } from "./windows-signature.mjs";

const DEFAULT_MSI_DIRECTORY = "src-tauri/target/release/bundle/msi";
const WINDOWS_EXECUTABLE_NAME = "bottie.exe";
const SMOKE_IDENTIFIER = "com.bottie.packaging-smoke";
const SMOKE_PRODUCT_NAME = "bottie-packaging-smoke";
const SMOKE_STARTUP_TIMEOUT_MS = 120_000;
const SMOKE_SETTLE_MS = 3_000;
const SMOKE_POLL_MS = 100;
const TERMINATION_TIMEOUT_MS = 5_000;
const MAX_CAPTURED_OUTPUT_BYTES = 16_384;
const MAX_EMBEDDED_ICON_DIMENSION = 256;
const PE_OFFSET_POSITION = 0x3c;
const PE_MACHINE_OFFSET = 4;
const PE_SIGNATURE_BYTES = Buffer.from([0x50, 0x45, 0x00, 0x00]);
const PE_MACHINES = new Map([
  [0x014c, "x86"],
  [0x8664, "x86_64"],
  [0xaa64, "aarch64"],
]);
const REQUIRED_DISTRIBUTION_DOCUMENTS = new Map([
  ["LICENSE", "licence"],
  ["MODEL-NOTICE.txt", "modelNotice"],
  ["THIRD-PARTY-NOTICES.txt", "thirdPartyNotices"],
]);

/** Returns the exact locked, MSI-only Tauri arguments used by the package command. */
export function windowsBuildArguments() {
  return ["build", "--bundles", "msi", "--no-sign", "--ci", "--", "--locked"];
}

/** Returns a locked build that isolates smoke storage under a distinct application identity. */
export function windowsSmokeBuildArguments() {
  const config = JSON.stringify({ identifier: SMOKE_IDENTIFIER, productName: SMOKE_PRODUCT_NAME });
  return ["build", "--bundles", "msi", "--no-sign", "--ci", "--config", config, "--", "--locked"];
}

/** Returns a non-installing administrative MSI extraction command. */
export function msiAdministrativeInstallArguments(msiPath, targetDirectory) {
  return ["/a", msiPath, "/qn", "/norestart", `TARGETDIR=${targetDirectory}`];
}

/** Produces provider settings that can contact only the supplied isolated loopback endpoint. */
export function offlineProviderSettings(port) {
  return {
    omlxBaseUrl: `http://127.0.0.1:${port}/`,
    ollamaBaseUrl: `http://127.0.0.1:${port}/`,
    setupCompleted: true,
    lastProviderId: "omlx",
    lastModelId: "packaging-offline-smoke",
  };
}

/** Returns the bounded PowerShell probe for an installed executable's embedded icon. */
export function embeddedIconPowerShellScript() {
  return [
    "Add-Type -AssemblyName System.Drawing.Common",
    "$icon = [System.Drawing.Icon]::ExtractAssociatedIcon($env:BOTTIE_WINDOWS_INSPECT_PATH)",
    "if ($null -eq $icon) { throw 'The Bottie executable has no associated icon.' }",
    'Write-Output "$($icon.Width)x$($icon.Height)"',
  ].join("; ");
}

/** Parses bounded public dimensions returned by the installed executable icon probe. */
export function parseEmbeddedIconDimensions(output) {
  const match = /^(\d+)x(\d+)$/.exec(output);
  const width = Number(match?.[1]);
  const height = Number(match?.[2]);
  if (
    !match ||
    width < 1 ||
    height < 1 ||
    width > MAX_EMBEDDED_ICON_DIMENSION ||
    height > MAX_EMBEDDED_ICON_DIMENSION
  ) {
    throw new Error("The packaged Bottie executable returned invalid embedded-icon evidence.");
  }
  return { height, width };
}

/** Converts a host-relative path into a portable package-evidence path. */
function portableRelativePath(root, path) {
  return relative(root, path).split(sep).join("/");
}

/** Recursively records regular files using extraction-relative paths only. */
async function visitExtractedBundle(directory, root, files) {
  const entries = await readdir(directory, { withFileTypes: true });
  entries.sort((left, right) => left.name.localeCompare(right.name));
  for (const entry of entries) {
    const absolutePath = join(directory, entry.name);
    if (entry.isDirectory()) {
      await visitExtractedBundle(absolutePath, root, files);
    } else if (entry.isFile()) {
      const bytes = await readFile(absolutePath);
      files.push({
        path: portableRelativePath(root, absolutePath),
        sha256: createHash("sha256").update(bytes).digest("hex"),
        size: bytes.length,
      });
    }
  }
}

/** Inventories one administratively extracted MSI payload without exposing its host location. */
export async function inspectExtractedWindowsBundle(bundlePath) {
  const root = resolve(bundlePath);
  const status = await lstat(root);
  if (!status.isDirectory()) throw new Error("Expected an administratively extracted Windows bundle directory.");
  const extractedFiles = [];
  await visitExtractedBundle(root, root, extractedFiles);
  const executables = extractedFiles.filter((file) => basename(file.path).toLowerCase() === WINDOWS_EXECUTABLE_NAME);
  if (executables.length !== 1) throw new Error("The Windows bundle must contain exactly one Bottie executable.");
  const applicationDirectory = dirname(executables[0].path);
  const applicationRoot = join(root, ...applicationDirectory.split("/"));
  const files = [];
  await visitExtractedBundle(applicationRoot, applicationRoot, files);
  const nativeRuntimeAssets = files
    .filter((file) => file.path.toLowerCase().endsWith(".dll"))
    .map((file) => file.path)
    .sort();
  const digest = createHash("sha256");
  for (const file of files) digest.update(`${file.path}\0${file.sha256}\0`);
  const requiredDocuments = requiredDistributionDocuments(files);
  return {
    applicationDirectory,
    bundleDigest: digest.digest("hex"),
    executable: basename(executables[0].path),
    fileCount: files.length,
    totalBytes: files.reduce((total, file) => total + file.size, 0),
    nativeRuntimeAssets,
    requiredDocuments,
    files,
  };
}

/** Requires one exact packaged project licence, model notice, and third-party notice bundle. */
function requiredDistributionDocuments(files) {
  const documents = {};
  for (const [filename, key] of REQUIRED_DISTRIBUTION_DOCUMENTS) {
    const matches = files.filter((file) => basename(file.path) === filename);
    if (matches.length !== 1) throw new Error(`The Windows bundle must contain exactly one ${filename}.`);
    documents[key] = matches[0].sha256;
  }
  return documents;
}

/** Runs a host command and returns its output or one path-free failure. */
function runHostCommand(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, { encoding: "utf8", ...options });
  if (result.error || result.status !== 0) throw new Error(`${command} failed while collecting package evidence.`);
  return `${result.stdout ?? ""}${result.stderr ?? ""}`;
}

/** Returns an exact path-free Authenticode classification for one package file. */
function inspectAuthenticode(path) {
  const script = [
    "$signature = Get-AuthenticodeSignature -LiteralPath $env:BOTTIE_WINDOWS_INSPECT_PATH",
    "$evidence = [pscustomobject]@{ status = $signature.Status.ToString(); " +
      "timestamped = ($null -ne $signature.TimeStamperCertificate) }",
    "$evidence | ConvertTo-Json -Compress",
  ].join("; ");
  const output = runHostCommand("pwsh.exe", ["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script], {
    env: { ...process.env, BOTTIE_WINDOWS_INSPECT_PATH: path },
  }).trim();
  return parseAuthenticodeEvidence(output);
}

/** Extracts only public dimensions from the installed executable's embedded icon resource. */
function inspectEmbeddedIcon(path) {
  const output = runHostCommand(
    "pwsh.exe",
    ["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", embeddedIconPowerShellScript()],
    { env: { ...process.env, BOTTIE_WINDOWS_INSPECT_PATH: path } },
  ).trim();
  return parseEmbeddedIconDimensions(output);
}

/** Reads the architecture from one Portable Executable without invoking host tooling. */
async function inspectPortableExecutableArchitecture(path) {
  const bytes = await readFile(path);
  if (bytes.length <= PE_OFFSET_POSITION + 4 || bytes.subarray(0, 2).toString("ascii") !== "MZ") {
    throw new Error("The packaged Bottie executable is not a valid PE image.");
  }
  const peOffset = bytes.readUInt32LE(PE_OFFSET_POSITION);
  if (bytes.length <= peOffset + PE_MACHINE_OFFSET + 2) throw new Error("The packaged PE header is truncated.");
  if (!bytes.subarray(peOffset, peOffset + PE_MACHINE_OFFSET).equals(PE_SIGNATURE_BYTES)) {
    throw new Error("The packaged Bottie executable has an invalid PE signature.");
  }
  const machine = bytes.readUInt16LE(peOffset + PE_MACHINE_OFFSET);
  return PE_MACHINES.get(machine) ?? `unknown-0x${machine.toString(16).padStart(4, "0")}`;
}

/** Finds exactly one MSI below a Tauri bundle directory. */
export async function findSingleMsi(directory) {
  const files = [];
  await visitExtractedBundle(directory, directory, files);
  const installers = files.filter((file) => file.path.toLowerCase().endsWith(".msi"));
  if (installers.length !== 1) throw new Error("The locked Windows build must produce exactly one MSI.");
  return join(directory, ...installers[0].path.split("/"));
}

/** Extracts one MSI into a fresh directory without installing or registering it. */
function extractMsi(msiPath, targetDirectory) {
  runHostCommand("msiexec.exe", msiAdministrativeInstallArguments(msiPath, targetDirectory));
}

/** Collects installer, payload, architecture, signing, and native-runtime evidence. */
export async function inspectWindowsMsi(msiPath, extractedDirectory) {
  extractMsi(msiPath, extractedDirectory);
  const payload = await inspectExtractedWindowsBundle(extractedDirectory);
  const executablePath = join(extractedDirectory, ...payload.applicationDirectory.split("/"), payload.executable);
  const installerBytes = await readFile(msiPath);
  return {
    installer: {
      sha256: createHash("sha256").update(installerBytes).digest("hex"),
      signature: inspectAuthenticode(msiPath),
      size: installerBytes.length,
    },
    payload: {
      ...payload,
      architecture: await inspectPortableExecutableArchitecture(executablePath),
      embeddedIcon: inspectEmbeddedIcon(executablePath),
      signature: inspectAuthenticode(executablePath),
    },
  };
}

/** Starts a loopback endpoint that proves discovery occurred while rejecting all provider traffic. */
async function startOfflineEndpoint() {
  let connectionCount = 0;
  const server = createServer((socket) => {
    connectionCount += 1;
    socket.destroy();
  });
  await new Promise((resolvePromise, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolvePromise);
  });
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("Could not create the isolated provider endpoint.");
  return { server, port: address.port, connectionCount: () => connectionCount };
}

/** Waits for one bounded smoke condition. */
async function waitFor(predicate, timeoutMs, message) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await new Promise((resolvePromise) => setTimeout(resolvePromise, SMOKE_POLL_MS));
  }
  throw new Error(message);
}

/** Waits for a process exit event for a bounded interval. */
function waitForExit(child, timeoutMs) {
  if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve(true);
  return Promise.race([
    new Promise((resolvePromise) => child.once("exit", () => resolvePromise(true))),
    new Promise((resolvePromise) => setTimeout(() => resolvePromise(false), timeoutMs)),
  ]);
}

/** Terminates only the isolated Windows smoke process tree. */
async function terminateChild(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const result = spawnSync("taskkill.exe", ["/PID", String(child.pid), "/T", "/F"], { stdio: "ignore" });
  if (result.status !== 0 && child.exitCode === null && child.signalCode === null) child.kill();
  if (!(await waitForExit(child, TERMINATION_TIMEOUT_MS))) {
    throw new Error("The isolated smoke process did not terminate cleanly.");
  }
}

/** Returns an immutable read-only summary of the isolated smoke database. */
async function inspectSmokeDatabase(databasePath) {
  const { DatabaseSync } = await import("node:sqlite");
  const database = new DatabaseSync(databasePath, { readOnly: true });
  try {
    const quickCheck = database.prepare("PRAGMA quick_check").get().quick_check;
    const schemaVersion = database.prepare("PRAGMA user_version").get().user_version;
    const migrationCount = database.prepare("SELECT COUNT(*) AS count FROM schema_migrations").get().count;
    const profileCount = database.prepare("SELECT COUNT(*) AS count FROM profiles").get().count;
    const conversationCount = database.prepare("SELECT COUNT(*) AS count FROM conversations").get().count;
    return { conversationCount, migrationCount, profileCount, quickCheck, schemaVersion };
  } finally {
    database.close();
  }
}

/** Launches an extracted distinct-identity payload with one rejecting provider endpoint. */
export async function smokeWindowsBundle(extractedDirectory, inspection) {
  const roamingAppData = process.env.APPDATA;
  if (!roamingAppData) throw new Error("The Windows roaming application-data directory is unavailable.");
  const supportDirectory = join(roamingAppData, SMOKE_IDENTIFIER);
  const databasePath = join(supportDirectory, "bottie.sqlite3");
  const settingsPath = join(supportDirectory, "providers.json");
  const executablePath = join(
    extractedDirectory,
    ...inspection.payload.applicationDirectory.split("/"),
    inspection.payload.executable,
  );
  const endpoint = await startOfflineEndpoint();
  let child;
  let childError;
  let createdSupportDirectory = false;
  try {
    try {
      await lstat(supportDirectory);
      throw new Error("The packaging-smoke support directory already exists; refusing to replace it.");
    } catch (error) {
      if (!(error instanceof Error) || !error.message.includes("ENOENT")) throw error;
    }
    await mkdir(supportDirectory);
    createdSupportDirectory = true;
    await writeFile(settingsPath, `${JSON.stringify(offlineProviderSettings(endpoint.port), null, 2)}\n`);
    child = spawn(executablePath, [], { env: process.env, stdio: ["ignore", "pipe", "pipe"] });
    child.once("error", (error) => {
      childError = error;
    });
    let capturedOutputBytes = 0;
    for (const stream of [child.stdout, child.stderr]) {
      stream.on("data", (chunk) => {
        capturedOutputBytes = Math.min(MAX_CAPTURED_OUTPUT_BYTES, capturedOutputBytes + chunk.length);
      });
    }
    await waitFor(
      async () => {
        if (childError) throw new Error("The packaged app could not start.");
        if (child.exitCode !== null || child.signalCode !== null) {
          throw new Error("The packaged app exited before creating its isolated store.");
        }
        try {
          return (await lstat(databasePath)).isFile();
        } catch {
          return false;
        }
      },
      SMOKE_STARTUP_TIMEOUT_MS,
      "The packaged app did not create its isolated store before the smoke deadline.",
    );
    console.error("[bottie] smoke: isolated Windows store created.");
    await waitFor(
      () => {
        if (childError) throw new Error("The packaged app could not start.");
        if (child.exitCode !== null || child.signalCode !== null) {
          throw new Error("The packaged app exited before offline-provider discovery.");
        }
        return endpoint.connectionCount() > 0;
      },
      SMOKE_STARTUP_TIMEOUT_MS,
      "The packaged app did not exercise isolated offline-provider discovery.",
    );
    console.error("[bottie] smoke: rejecting Windows provider endpoint contacted.");
    await new Promise((resolvePromise) => setTimeout(resolvePromise, SMOKE_SETTLE_MS));
    if (child.exitCode !== null || child.signalCode !== null) {
      throw new Error("The packaged app exited during the bounded smoke window.");
    }
    console.error("[bottie] smoke: packaged Windows executable remained live through the settle window.");
    await terminateChild(child);
    return {
      capturedOutputBytes,
      database: await inspectSmokeDatabase(databasePath),
      isolatedSupportDirectory: true,
      offlineProviderConnections: endpoint.connectionCount(),
      remainedRunning: true,
      terminated: true,
    };
  } finally {
    if (child) await terminateChild(child);
    await new Promise((resolvePromise) => endpoint.server.close(resolvePromise));
    if (createdSupportDirectory) await rm(supportDirectory, { recursive: true, force: true });
  }
}

/** Builds a locked Windows MSI through the existing cross-platform Tauri wrapper. */
export function buildWindowsBundle(repositoryRoot, arguments_, targetDirectory) {
  const script = join(repositoryRoot, "scripts", "macos-development-signing.mjs");
  const result = spawnSync(process.execPath, [script, "--tauri", ...arguments_], {
    cwd: repositoryRoot,
    env: targetDirectory ? { ...process.env, CARGO_TARGET_DIR: targetDirectory } : process.env,
    stdio: "inherit",
  });
  if (result.status !== 0) throw new Error("The locked unsigned Windows MSI build failed.");
}

/** Combines the real product-package inspection with a separately isolated smoke outcome. */
export function combineWindowsPackageEvidence(bundle, smoke) {
  if (bundle?.payload?.applicationDirectory !== "PFiles/bottie") {
    throw new Error("Release evidence must describe the real Bottie package.");
  }
  return { bundle, smoke };
}

/** Builds and inspects the product MSI, then smoke-tests a separate application identity. */
async function runWindowsSmoke(repositoryRoot) {
  const temporaryRoot = await mkdtemp(join(tmpdir(), "bottie-windows-smoke-"));
  try {
    const packageTargetDirectory = join(temporaryRoot, "package-target");
    const packageExtractedDirectory = join(temporaryRoot, "package-extracted");
    const smokeTargetDirectory = join(temporaryRoot, "smoke-target");
    const smokeExtractedDirectory = join(temporaryRoot, "smoke-extracted");
    await mkdir(packageExtractedDirectory);
    await mkdir(smokeExtractedDirectory);

    buildWindowsBundle(repositoryRoot, windowsBuildArguments(), packageTargetDirectory);
    const msiPath = await findSingleMsi(join(packageTargetDirectory, "release", "bundle", "msi"));
    if (process.env.BOTTIE_WINDOWS_ARTIFACT_DIRECTORY) {
      const artifactDirectory = resolve(repositoryRoot, process.env.BOTTIE_WINDOWS_ARTIFACT_DIRECTORY);
      await mkdir(artifactDirectory, { recursive: true });
      await copyFile(msiPath, join(artifactDirectory, basename(msiPath)));
    }
    const bundle = await inspectWindowsMsi(msiPath, packageExtractedDirectory);

    buildWindowsBundle(repositoryRoot, windowsSmokeBuildArguments(), smokeTargetDirectory);
    const smokeMsiPath = await findSingleMsi(join(smokeTargetDirectory, "release", "bundle", "msi"));
    const smokeBundle = await inspectWindowsMsi(smokeMsiPath, smokeExtractedDirectory);
    const smoke = await smokeWindowsBundle(smokeExtractedDirectory, smokeBundle);
    return combineWindowsPackageEvidence(bundle, smoke);
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
}

/** Emits path-free evidence to stdout and optionally to a caller-selected report file. */
async function emitEvidence(repositoryRoot, evidence) {
  const output = `${JSON.stringify(evidence, null, 2)}\n`;
  if (process.env.BOTTIE_WINDOWS_EVIDENCE_PATH) {
    const evidencePath = resolve(repositoryRoot, process.env.BOTTIE_WINDOWS_EVIDENCE_PATH);
    await mkdir(dirname(evidencePath), { recursive: true });
    await writeFile(evidencePath, output);
  }
  console.log(output.trimEnd());
}

/** Adds the checked-out application version and evidence schema without exposing a source path. */
export function versionedPackageEvidence(version, evidence) {
  return { ...evidence, schemaVersion: 1, version };
}

/** Reads the checked-out application version before emitting package evidence. */
async function versionedEvidence(repositoryRoot, evidence) {
  const config = JSON.parse(await readFile(join(repositoryRoot, "src-tauri", "tauri.conf.json"), "utf8"));
  return versionedPackageEvidence(config.version, evidence);
}

/** Dispatches package build, inspection, and smoke modes. */
async function main() {
  if (process.platform !== "win32") throw new Error("The Windows package workflow requires a Windows host.");
  const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const [mode, suppliedPath] = process.argv.slice(2);
  if (mode === "--smoke") {
    await emitEvidence(repositoryRoot, await versionedEvidence(repositoryRoot, await runWindowsSmoke(repositoryRoot)));
    return;
  }
  const temporaryRoot = await mkdtemp(join(tmpdir(), "bottie-windows-inspect-"));
  try {
    const msiDirectory = resolve(repositoryRoot, DEFAULT_MSI_DIRECTORY);
    if (mode === "--build") buildWindowsBundle(repositoryRoot, windowsBuildArguments());
    const msiPath = suppliedPath ? resolve(repositoryRoot, suppliedPath) : await findSingleMsi(msiDirectory);
    await emitEvidence(
      repositoryRoot,
      await versionedEvidence(repositoryRoot, await inspectWindowsMsi(msiPath, temporaryRoot)),
    );
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    await main();
  } catch (error) {
    console.error(`[bottie] ${error instanceof Error ? error.message : "The Windows package workflow failed."}`);
    process.exitCode = 1;
  }
}
