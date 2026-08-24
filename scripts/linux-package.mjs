#!/usr/bin/env node

/** Builds, inspects, and smoke-tests Bottie's bounded unsigned Linux DEB package. */

import { createHash } from "node:crypto";
import { spawn, spawnSync } from "node:child_process";
import { copyFile, lstat, mkdir, mkdtemp, readFile, readdir, readlink, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const DEFAULT_DEB_DIRECTORY = "src-tauri/target/release/bundle/deb";
const LINUX_EXECUTABLE_NAME = "bottie";
const SMOKE_IDENTIFIER = "com.bottie.packaging-smoke";
const SMOKE_PRODUCT_NAME = "bottie-packaging-smoke";
const SMOKE_STARTUP_TIMEOUT_MS = 120_000;
const SMOKE_SETTLE_MS = 3_000;
const SMOKE_POLL_MS = 100;
const TERMINATION_TIMEOUT_MS = 5_000;
const MAX_CAPTURED_OUTPUT_BYTES = 16_384;
const MAX_DESKTOP_ENTRY_BYTES = 65_536;
const ELF_MACHINE_OFFSET = 18;
const ELF_HEADER_MINIMUM_BYTES = 20;
const ELF_SIGNATURE_BYTES = Buffer.from([0x7f, 0x45, 0x4c, 0x46]);
const ELF_MACHINES = new Map([
  [0x03, "x86"],
  [0x28, "arm"],
  [0x3e, "x86_64"],
  [0xb7, "aarch64"],
]);
const INSTALLED_ICON_DIRECTORIES = ["32x32", "64x64", "128x128", "256x256@2"];

/** Returns the exact locked, DEB-only Tauri arguments used by the package command. */
export function linuxBuildArguments() {
  return ["build", "--bundles", "deb", "--no-sign", "--ci", "--", "--locked"];
}

/** Returns a locked build that isolates smoke storage under a distinct application identity. */
export function linuxSmokeBuildArguments() {
  const config = JSON.stringify({ identifier: SMOKE_IDENTIFIER, productName: SMOKE_PRODUCT_NAME });
  return ["build", "--bundles", "deb", "--no-sign", "--ci", "--config", config, "--", "--locked"];
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

/** Resolves the process-owned XDG roots and exact distinct-identity app paths used by smoke. */
export function smokeXdgDirectories(root) {
  return {
    cache: join(root, "cache"),
    config: join(root, "config"),
    data: join(root, "data"),
    runtime: join(root, "runtime"),
    support: join(root, "data", SMOKE_IDENTIFIER),
    settings: join(root, "config", SMOKE_IDENTIFIER, "providers.json"),
  };
}

/** Reads one closed Bottie icon identity from a packaged freedesktop launcher. */
export function packagedLinuxIconName(desktopEntry) {
  const names = desktopEntry
    .split(/\r?\n/)
    .filter((line) => line.startsWith("Icon="))
    .map((line) => line.slice("Icon=".length));
  if (names.length !== 1 || names[0] !== LINUX_EXECUTABLE_NAME) {
    throw new Error("The Linux desktop launcher has an invalid Bottie icon identity.");
  }
  return names[0];
}

/** Converts a host-relative path into a portable package-evidence path. */
function portableRelativePath(root, path) {
  return relative(root, path).split(sep).join("/");
}

/** Recursively records regular files and symbolic links using extraction-relative paths only. */
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
        type: "file",
      });
    } else if (entry.isSymbolicLink()) {
      const target = await readlink(absolutePath);
      files.push({
        path: portableRelativePath(root, absolutePath),
        sha256: createHash("sha256").update(target).digest("hex"),
        size: Buffer.byteLength(target),
        type: "symlink",
      });
    }
  }
}

/** Reads one supported architecture from a little-endian ELF header. */
async function inspectElfArchitecture(path) {
  const bytes = await readFile(path);
  if (bytes.length < ELF_HEADER_MINIMUM_BYTES || !bytes.subarray(0, 4).equals(ELF_SIGNATURE_BYTES)) {
    throw new Error("The packaged Bottie executable is not a valid ELF image.");
  }
  if (bytes[5] !== 1) throw new Error("The packaged Bottie executable uses an unsupported ELF byte order.");
  const machine = bytes.readUInt16LE(ELF_MACHINE_OFFSET);
  return ELF_MACHINES.get(machine) ?? `unknown-0x${machine.toString(16).padStart(4, "0")}`;
}

/** Inventories one extracted DEB payload without exposing its host location. */
export async function inspectExtractedLinuxBundle(bundlePath) {
  const root = resolve(bundlePath);
  const status = await lstat(root);
  if (!status.isDirectory()) throw new Error("Expected an extracted Linux bundle directory.");
  const files = [];
  await visitExtractedBundle(root, root, files);
  const executables = files.filter((file) => basename(file.path) === LINUX_EXECUTABLE_NAME && file.type === "file");
  if (executables.length !== 1) throw new Error("The Linux bundle must contain exactly one Bottie executable.");
  const nativeRuntimeAssets = files
    .filter((file) => /(^|\/)[^/]+\.so(?:\..+)?$/.test(file.path))
    .map((file) => file.path)
    .sort();
  const desktopEntries = files.filter(
    (file) => /^usr\/share\/applications\/[^/]+\.desktop$/.test(file.path) && file.type === "file",
  );
  if (desktopEntries.length !== 1 || desktopEntries[0].size > MAX_DESKTOP_ENTRY_BYTES) {
    throw new Error("The Linux bundle must contain one bounded desktop launcher.");
  }
  const desktopEntryPath = join(root, ...desktopEntries[0].path.split("/"));
  const iconName = packagedLinuxIconName(await readFile(desktopEntryPath, "utf8"));
  const expectedInstalledIcons = INSTALLED_ICON_DIRECTORIES.map(
    (directory) => `usr/share/icons/hicolor/${directory}/apps/${iconName}.png`,
  ).sort();
  const installedIcons = files
    .filter((file) => /^usr\/share\/icons\/hicolor\/\d+x\d+(?:@2)?\/apps\/[^/]+\.png$/.test(file.path))
    .map((file) => file.path)
    .sort();
  if (JSON.stringify(installedIcons) !== JSON.stringify(expectedInstalledIcons)) {
    throw new Error(`The Linux bundle has an invalid Bottie application icon set: ${JSON.stringify(installedIcons)}.`);
  }
  const digest = createHash("sha256");
  for (const file of files) digest.update(`${file.type}\0${file.path}\0${file.sha256}\0`);
  const executablePath = join(root, ...executables[0].path.split("/"));
  return {
    architecture: await inspectElfArchitecture(executablePath),
    bundleDigest: digest.digest("hex"),
    executable: executables[0].path,
    fileCount: files.length,
    installedIcons,
    totalBytes: files.reduce((total, file) => total + file.size, 0),
    nativeRuntimeAssets,
    files,
  };
}

/** Runs a host command and returns its output or one path-free failure. */
function runHostCommand(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, { encoding: "utf8", ...options });
  if (result.error || result.status !== 0) throw new Error(`${command} failed while collecting package evidence.`);
  return `${result.stdout ?? ""}${result.stderr ?? ""}`;
}

/** Returns normalized direct ELF shared-library requirements without host paths. */
function inspectElfDependencies(executablePath) {
  const output = runHostCommand("readelf", ["--dynamic", executablePath]);
  return [...output.matchAll(/\(NEEDED\)\s+Shared library: \[([^\]]+)\]/g)].map((match) => match[1]).sort();
}

/** Returns exact public DEB control metadata without maintainer scripts or host paths. */
function inspectDebianMetadata(debPath) {
  const field = (name) => runHostCommand("dpkg-deb", ["--field", debPath, name]).trim();
  return {
    architecture: field("Architecture"),
    dependencies: field("Depends"),
    package: field("Package"),
    version: field("Version"),
  };
}

/** Classifies the DEB archive signature from its portable archive members. */
function inspectDebianSignature(debPath) {
  const members = runHostCommand("ar", ["t", debPath]).split(/\r?\n/).filter(Boolean);
  const verifies = members.some((member) => member.startsWith("_gpg"));
  return { classification: verifies ? "identified" : "unsigned", verifies };
}

/** Finds exactly one DEB below a Tauri bundle directory. */
async function findSingleDeb(directory) {
  const files = [];
  await visitExtractedBundle(directory, directory, files);
  const installers = files.filter((file) => file.path.toLowerCase().endsWith(".deb") && file.type === "file");
  if (installers.length !== 1) throw new Error("The locked Linux build must produce exactly one DEB.");
  return join(directory, ...installers[0].path.split("/"));
}

/** Extracts one DEB into a fresh directory without installing or registering it. */
function extractDeb(debPath, targetDirectory) {
  runHostCommand("dpkg-deb", ["--extract", debPath, targetDirectory]);
}

/** Collects installer, payload, architecture, dependency, signing, and native-runtime evidence. */
async function inspectLinuxDeb(debPath, extractedDirectory) {
  extractDeb(debPath, extractedDirectory);
  const payload = await inspectExtractedLinuxBundle(extractedDirectory);
  const executablePath = join(extractedDirectory, ...payload.executable.split("/"));
  const installerBytes = await readFile(debPath);
  return {
    installer: {
      metadata: inspectDebianMetadata(debPath),
      sha256: createHash("sha256").update(installerBytes).digest("hex"),
      signature: inspectDebianSignature(debPath),
      size: installerBytes.length,
    },
    payload: { ...payload, elfDependencies: inspectElfDependencies(executablePath) },
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

/** Terminates only the isolated Linux smoke process group. */
async function terminateChild(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  process.kill(-child.pid, "SIGTERM");
  if (!(await waitForExit(child, TERMINATION_TIMEOUT_MS))) {
    process.kill(-child.pid, "SIGKILL");
    if (!(await waitForExit(child, TERMINATION_TIMEOUT_MS))) {
      throw new Error("The isolated smoke process did not terminate cleanly.");
    }
  }
}

/** Returns a read-only summary of the isolated smoke database. */
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

/** Launches an extracted distinct-identity payload with isolated XDG roots and one rejecting provider endpoint. */
async function smokeLinuxBundle(extractedDirectory, inspection, smokeRoot) {
  const paths = smokeXdgDirectories(smokeRoot);
  const databasePath = join(paths.support, "bottie.sqlite3");
  const executablePath = join(extractedDirectory, ...inspection.payload.executable.split("/"));
  const endpoint = await startOfflineEndpoint();
  let child;
  let childError;
  try {
    await Promise.all([
      mkdir(dirname(paths.settings), { recursive: true }),
      mkdir(paths.cache, { recursive: true }),
      mkdir(paths.runtime, { mode: 0o700, recursive: true }),
    ]);
    await writeFile(paths.settings, `${JSON.stringify(offlineProviderSettings(endpoint.port), null, 2)}\n`);
    child = spawn(executablePath, [], {
      detached: true,
      env: {
        ...process.env,
        XDG_CACHE_HOME: paths.cache,
        XDG_CONFIG_HOME: paths.config,
        XDG_DATA_HOME: paths.data,
        XDG_RUNTIME_DIR: paths.runtime,
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
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
    console.error("[bottie] smoke: isolated Linux store created.");
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
    console.error("[bottie] smoke: rejecting Linux provider endpoint contacted.");
    await new Promise((resolvePromise) => setTimeout(resolvePromise, SMOKE_SETTLE_MS));
    if (child.exitCode !== null || child.signalCode !== null) {
      throw new Error("The packaged app exited during the bounded smoke window.");
    }
    console.error("[bottie] smoke: packaged Linux executable remained live through the settle window.");
    await terminateChild(child);
    return {
      capturedOutputBytes,
      database: await inspectSmokeDatabase(databasePath),
      isolatedXdgDirectories: true,
      offlineProviderConnections: endpoint.connectionCount(),
      remainedRunning: true,
      terminated: true,
    };
  } finally {
    if (child) await terminateChild(child);
    await new Promise((resolvePromise) => endpoint.server.close(resolvePromise));
  }
}

/** Builds a locked Linux DEB through the existing cross-platform Tauri wrapper. */
function buildLinuxBundle(repositoryRoot, arguments_, targetDirectory) {
  const script = join(repositoryRoot, "scripts", "macos-development-signing.mjs");
  const result = spawnSync(process.execPath, [script, "--tauri", ...arguments_], {
    cwd: repositoryRoot,
    env: targetDirectory ? { ...process.env, CARGO_TARGET_DIR: targetDirectory } : process.env,
    stdio: "inherit",
  });
  if (result.status !== 0) throw new Error("The locked unsigned Linux DEB build failed.");
}

/** Builds, extracts, inspects, and smoke-tests one isolated DEB. */
async function runLinuxSmoke(repositoryRoot) {
  const temporaryRoot = await mkdtemp(join(tmpdir(), "bottie-linux-smoke-"));
  try {
    const targetDirectory = join(temporaryRoot, "target");
    const extractedDirectory = join(temporaryRoot, "extracted");
    const smokeRoot = join(temporaryRoot, "xdg");
    await mkdir(extractedDirectory);
    buildLinuxBundle(repositoryRoot, linuxSmokeBuildArguments(), targetDirectory);
    const debPath = await findSingleDeb(join(targetDirectory, "release", "bundle", "deb"));
    if (process.env.BOTTIE_LINUX_ARTIFACT_DIRECTORY) {
      const artifactDirectory = resolve(repositoryRoot, process.env.BOTTIE_LINUX_ARTIFACT_DIRECTORY);
      await mkdir(artifactDirectory, { recursive: true });
      await copyFile(debPath, join(artifactDirectory, basename(debPath)));
    }
    const bundle = await inspectLinuxDeb(debPath, extractedDirectory);
    return { bundle, smoke: await smokeLinuxBundle(extractedDirectory, bundle, smokeRoot) };
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
}

/** Emits path-free evidence to stdout and optionally to a caller-selected report file. */
async function emitEvidence(repositoryRoot, evidence) {
  const output = `${JSON.stringify(evidence, null, 2)}\n`;
  if (process.env.BOTTIE_LINUX_EVIDENCE_PATH) {
    const evidencePath = resolve(repositoryRoot, process.env.BOTTIE_LINUX_EVIDENCE_PATH);
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
  if (process.platform !== "linux") throw new Error("The Linux package workflow requires a Linux host.");
  const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const [mode, suppliedPath] = process.argv.slice(2);
  if (mode === "--smoke") {
    await emitEvidence(repositoryRoot, await versionedEvidence(repositoryRoot, await runLinuxSmoke(repositoryRoot)));
    return;
  }
  const temporaryRoot = await mkdtemp(join(tmpdir(), "bottie-linux-inspect-"));
  try {
    const debDirectory = resolve(repositoryRoot, DEFAULT_DEB_DIRECTORY);
    if (mode === "--build") buildLinuxBundle(repositoryRoot, linuxBuildArguments());
    const debPath = suppliedPath ? resolve(repositoryRoot, suppliedPath) : await findSingleDeb(debDirectory);
    await emitEvidence(
      repositoryRoot,
      await versionedEvidence(repositoryRoot, await inspectLinuxDeb(debPath, temporaryRoot)),
    );
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    await main();
  } catch (error) {
    console.error(`[bottie] ${error instanceof Error ? error.message : "The Linux package workflow failed."}`);
    process.exitCode = 1;
  }
}
