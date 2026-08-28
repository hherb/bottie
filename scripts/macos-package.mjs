#!/usr/bin/env node

/** Builds, inspects, and smoke-tests Bottie's bounded unsigned macOS application bundle. */

import { createHash } from "node:crypto";
import { spawn, spawnSync } from "node:child_process";
import { lstat, mkdir, readFile, readdir, readlink, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { homedir } from "node:os";
import { dirname, isAbsolute, join, normalize, relative, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { selectAppleDevelopmentIdentity } from "./macos-development-signing.mjs";

const DEFAULT_BUNDLE_PATH = "src-tauri/target/release/bundle/macos/bottie.app";
const SMOKE_BUNDLE_PATH = "src-tauri/target/release/bundle/macos/bottie-packaging-smoke.app";
const DEVELOPMENT_EXECUTABLE_IDENTIFIER = "com.bottie.app.dev";
const SMOKE_IDENTIFIER = "com.bottie.packaging-smoke";
const SMOKE_PRODUCT_NAME = "bottie-packaging-smoke";
const REQUIRED_BUNDLE_ENTRIES = {
  executable: "Contents/MacOS/bottie",
  icon: "Contents/Resources/icon.icns",
  infoPlist: "Contents/Info.plist",
  licence: "Contents/Resources/LICENSE",
  modelNotice: "Contents/Resources/MODEL-NOTICE.txt",
  thirdPartyNotices: "Contents/Resources/THIRD-PARTY-NOTICES.txt",
};
const FRONTEND_ASSET_PREFIX = "Contents/Resources/_up_/";
const SMOKE_STARTUP_TIMEOUT_MS = 120_000;
const SMOKE_SETTLE_MS = 3_000;
const SMOKE_POLL_MS = 100;
const TERMINATION_TIMEOUT_MS = 5_000;
const MAX_CAPTURED_OUTPUT_BYTES = 16_384;

/** Returns the exact locked, app-only Tauri arguments used by the package command. */
export function macosBuildArguments() {
  return ["build", "--bundles", "app", "--no-sign", "--ci", "--", "--locked"];
}

/** Returns the protected distribution build that alone creates Tauri v2 updater artifacts. */
export function macosUpdaterBuildArguments() {
  return [
    "build",
    "--bundles",
    "app",
    "--no-sign",
    "--ci",
    "--config",
    "src-tauri/tauri.updater.conf.json",
    "--",
    "--locked",
  ];
}

/** Returns a locked build that isolates smoke storage under a distinct application identity. */
export function macosSmokeBuildArguments() {
  const config = JSON.stringify({ identifier: SMOKE_IDENTIFIER, productName: SMOKE_PRODUCT_NAME });
  return ["build", "--bundles", "app", "--no-sign", "--ci", "--config", config, "--", "--locked"];
}

/** Returns deterministic hardened-runtime development-signing arguments for one exact bundle. */
export function developmentSigningArguments(identity, bundlePath) {
  return ["--force", "--sign", identity, "--options", "runtime", "--pagesize", "4096", "--timestamp=none", bundlePath];
}

/** Returns the established pre-run development signature for one exact packaged executable. */
export function developmentExecutableSigningArguments(identity, executablePath) {
  return [
    "--force",
    "--sign",
    identity,
    "--identifier",
    DEVELOPMENT_EXECUTABLE_IDENTIFIER,
    "--options",
    "runtime",
    "--pagesize",
    "4096",
    "--timestamp=none",
    executablePath,
  ];
}

/** Classifies code-signing output without retaining certificate or team identities. */
export function classifyCodeSignature(output) {
  if (/not signed/i.test(output)) return "unsigned";
  if (/Signature=adhoc/i.test(output)) return "ad-hoc";
  return /Authority=|TeamIdentifier=(?!not set)/i.test(output) ? "identified" : "unknown";
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

/** Encodes one native database path for SQLite's no-write immutable inspection mode. */
export function sqliteImmutableUri(databasePath) {
  const url = pathToFileURL(databasePath);
  url.searchParams.set("immutable", "1");
  return url.href;
}

/** Recursively records regular files and symbolic links using bundle-relative paths only. */
async function visitBundle(directory, root, files, symlinks) {
  const entries = await readdir(directory, { withFileTypes: true });
  entries.sort((left, right) => left.name.localeCompare(right.name));
  for (const entry of entries) {
    const absolutePath = join(directory, entry.name);
    const relativePath = relative(root, absolutePath);
    if (entry.isDirectory()) {
      await visitBundle(absolutePath, root, files, symlinks);
    } else if (entry.isSymbolicLink()) {
      const target = await readlink(absolutePath);
      const normalizedTarget = normalize(target);
      if (isAbsolute(target) || normalizedTarget === ".." || normalizedTarget.startsWith(`..${sep}`)) {
        throw new Error("The macOS bundle contains an unsafe symbolic link target.");
      }
      symlinks.push({ path: relativePath, target });
    } else if (entry.isFile()) {
      const bytes = await readFile(absolutePath);
      files.push({
        path: relativePath,
        sha256: createHash("sha256").update(bytes).digest("hex"),
        size: bytes.length,
      });
    }
  }
}

/** Inventories one application bundle without exposing its host filesystem location. */
export async function inspectBundleFiles(bundlePath) {
  const root = resolve(bundlePath);
  const status = await lstat(root);
  if (!status.isDirectory() || !root.endsWith(".app")) throw new Error("Expected a macOS .app bundle directory.");
  const files = [];
  const symlinks = [];
  await visitBundle(root, root, files, symlinks);
  const paths = new Set(files.map((file) => file.path));
  const requiredEntries = Object.fromEntries(
    Object.entries(REQUIRED_BUNDLE_ENTRIES).map(([name, path]) => [name, paths.has(path)]),
  );
  if (Object.values(requiredEntries).includes(false)) throw new Error("The macOS bundle is missing a required entry.");
  const requiredDocuments = {
    licence: files.find((file) => file.path === REQUIRED_BUNDLE_ENTRIES.licence).sha256,
    modelNotice: files.find((file) => file.path === REQUIRED_BUNDLE_ENTRIES.modelNotice).sha256,
    thirdPartyNotices: files.find((file) => file.path === REQUIRED_BUNDLE_ENTRIES.thirdPartyNotices).sha256,
  };
  const nativeRuntimeAssets = [...paths]
    .filter((path) => path.startsWith("Contents/Frameworks/") || /\.(?:dylib|so)$/.test(path))
    .sort();
  const digest = createHash("sha256");
  for (const file of files) digest.update(`${file.path}\0${file.sha256}\0`);
  return {
    bundleDigest: digest.digest("hex"),
    fileCount: files.length,
    totalBytes: files.reduce((total, file) => total + file.size, 0),
    frontendAssetCount: files.filter((file) => file.path.startsWith(FRONTEND_ASSET_PREFIX)).length,
    nativeRuntimeAssets,
    requiredDocuments,
    requiredEntries,
    files,
    symlinks,
  };
}

/** Runs a host command and returns its output or one path-free failure. */
function runHostCommand(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, { encoding: "utf8", ...options });
  if (result.error || result.status !== 0) throw new Error(`${command} failed while collecting package evidence.`);
  return `${result.stdout ?? ""}${result.stderr ?? ""}`;
}

/** Reads the public bundle metadata needed by inspection and isolated smoke setup. */
function readBundleMetadata(bundlePath) {
  const plistPath = join(bundlePath, REQUIRED_BUNDLE_ENTRIES.infoPlist);
  const output = runHostCommand("plutil", ["-convert", "json", "-o", "-", plistPath]);
  const metadata = JSON.parse(output);
  for (const key of ["CFBundleExecutable", "CFBundleIdentifier", "CFBundleShortVersionString"]) {
    if (typeof metadata[key] !== "string" || !metadata[key]) throw new Error(`The bundle plist is missing ${key}.`);
  }
  return {
    executable: metadata.CFBundleExecutable,
    identifier: metadata.CFBundleIdentifier,
    minimumSystemVersion: metadata.LSMinimumSystemVersion ?? null,
    name: metadata.CFBundleName ?? metadata.CFBundleDisplayName ?? null,
    version: metadata.CFBundleShortVersionString,
  };
}

/** Summarizes signing state without returning certificate labels or team identifiers. */
function inspectSignature(bundlePath) {
  const result = spawnSync("codesign", ["--display", "--verbose=4", bundlePath], { encoding: "utf8" });
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
  const classification = classifyCodeSignature(output);
  const verification = spawnSync("codesign", ["--verify", "--strict", bundlePath], { encoding: "utf8" });
  return { classification, verifies: verification.status === 0 };
}

/** Development-signs and strictly verifies the bundle without exposing the selected identity. */
function developmentSignMacosBundle(bundlePath) {
  const identities = spawnSync("security", ["find-identity", "-v", "-p", "codesigning"], {
    encoding: "utf8",
  });
  if (identities.status !== 0) throw new Error("Bottie could not inspect the active code-signing identities.");
  const identity = selectAppleDevelopmentIdentity(identities.stdout, process.env.BOTTIE_APPLE_SIGNING_IDENTITY);
  runHostCommand("codesign", developmentSigningArguments(identity, bundlePath));
  runHostCommand("codesign", ["--verify", "--strict", bundlePath]);
}

/** Applies the repository's proven pre-run signature to the packaged executable only. */
function developmentSignPackagedExecutable(bundlePath) {
  const identities = spawnSync("security", ["find-identity", "-v", "-p", "codesigning"], {
    encoding: "utf8",
  });
  if (identities.status !== 0) throw new Error("Bottie could not inspect the active code-signing identities.");
  const identity = selectAppleDevelopmentIdentity(identities.stdout, process.env.BOTTIE_APPLE_SIGNING_IDENTITY);
  const metadata = readBundleMetadata(bundlePath);
  const executable = join(bundlePath, "Contents", "MacOS", metadata.executable);
  runHostCommand("codesign", developmentExecutableSigningArguments(identity, executable));
  runHostCommand("codesign", ["--verify", "--strict", executable]);
}

/** Collects bundle, metadata, signing, and executable-architecture evidence. */
async function inspectMacosBundle(bundlePath) {
  const files = await inspectBundleFiles(bundlePath);
  const metadata = readBundleMetadata(bundlePath);
  const executable = join(bundlePath, "Contents", "MacOS", metadata.executable);
  const architectures = runHostCommand("lipo", ["-archs", executable]).trim().split(/\s+/);
  return { metadata, architectures, signature: inspectSignature(bundlePath), ...files };
}

/** Starts a loopback endpoint that proves discovery occurred while always rejecting provider traffic. */
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

/** Waits for process termination and escalates only within the isolated smoke process. */
async function terminateChild(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  child.kill("SIGTERM");
  let exited = await Promise.race([
    new Promise((resolvePromise) => child.once("exit", () => resolvePromise(true))),
    new Promise((resolvePromise) => setTimeout(() => resolvePromise(false), TERMINATION_TIMEOUT_MS)),
  ]);
  if (!exited) {
    child.kill("SIGKILL");
    exited = await Promise.race([
      new Promise((resolvePromise) => child.once("exit", () => resolvePromise(true))),
      new Promise((resolvePromise) => setTimeout(() => resolvePromise(false), TERMINATION_TIMEOUT_MS)),
    ]);
  }
  if (!exited) throw new Error("The isolated smoke process did not terminate cleanly.");
}

/** Returns a quick, read-only summary of the isolated smoke database. */
function inspectSmokeDatabase(databasePath) {
  const query = [
    "PRAGMA quick_check;",
    "PRAGMA user_version;",
    "SELECT COUNT(*) FROM schema_migrations;",
    "SELECT COUNT(*) FROM profiles;",
    "SELECT COUNT(*) FROM conversations;",
  ].join(" ");
  const lines = runHostCommand("sqlite3", ["-readonly", sqliteImmutableUri(databasePath), query])
    .trim()
    .split("\n");
  if (lines.length !== 5) throw new Error("The isolated smoke database returned unexpected evidence.");
  return {
    quickCheck: lines[0],
    schemaVersion: Number(lines[1]),
    migrationCount: Number(lines[2]),
    profileCount: Number(lines[3]),
    conversationCount: Number(lines[4]),
  };
}

/** Launches the bundle with isolated macOS support directories and a rejecting provider endpoint. */
async function smokeMacosBundle(bundlePath) {
  const metadata = readBundleMetadata(bundlePath);
  if (metadata.identifier !== SMOKE_IDENTIFIER) {
    throw new Error("Refusing to smoke-test a bundle that could share Bottie's live application data.");
  }
  const endpoint = await startOfflineEndpoint();
  const supportDirectory = join(homedir(), "Library", "Application Support", metadata.identifier);
  const databasePath = join(supportDirectory, "bottie.sqlite3");
  const settingsPath = join(supportDirectory, "providers.json");
  const executable = join(bundlePath, "Contents", "MacOS", metadata.executable);
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
    child = spawn(executable, [], {
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    child.once("error", (error) => {
      childError = error;
    });
    let capturedBytes = 0;
    for (const stream of [child.stdout, child.stderr]) {
      stream.on("data", (chunk) => {
        capturedBytes = Math.min(MAX_CAPTURED_OUTPUT_BYTES, capturedBytes + chunk.length);
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
    console.error("[bottie] smoke: isolated store created.");
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
    console.error("[bottie] smoke: rejecting provider endpoint contacted.");
    if (child.exitCode !== null || child.signalCode !== null) {
      throw new Error("The packaged app exited during the bounded smoke window.");
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, SMOKE_SETTLE_MS));
    console.error("[bottie] smoke: packaged executable remained live through the settle window.");
    await terminateChild(child);
    const database = inspectSmokeDatabase(databasePath);
    return {
      capturedOutputBytes: capturedBytes,
      database,
      isolatedSupportDirectory: database.quickCheck === "ok",
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

/** Builds the unsigned bundle through the existing Tauri wrapper. */
function buildMacosBundle(repositoryRoot, arguments_ = macosBuildArguments()) {
  const script = join(repositoryRoot, "scripts", "macos-development-signing.mjs");
  const result = spawnSync(process.execPath, [script, "--tauri", ...arguments_], {
    cwd: repositoryRoot,
    stdio: "inherit",
  });
  if (result.status !== 0) throw new Error("The locked unsigned macOS bundle build failed.");
}

/** Dispatches package build, inspection, and smoke modes. */
async function main() {
  if (process.platform !== "darwin") throw new Error("The macOS package workflow requires a macOS host.");
  const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const [mode, suppliedPath] = process.argv.slice(2);
  const bundlePath = resolve(repositoryRoot, suppliedPath ?? DEFAULT_BUNDLE_PATH);
  if (mode === "--build") {
    buildMacosBundle(repositoryRoot);
    console.log(JSON.stringify(await inspectMacosBundle(bundlePath), null, 2));
  } else if (mode === "--development-sign") {
    developmentSignMacosBundle(bundlePath);
    console.log(JSON.stringify(await inspectMacosBundle(bundlePath), null, 2));
  } else if (mode === "--inspect") {
    console.log(JSON.stringify(await inspectMacosBundle(bundlePath), null, 2));
  } else if (mode === "--smoke") {
    const smokeBundlePath = resolve(repositoryRoot, SMOKE_BUNDLE_PATH);
    buildMacosBundle(repositoryRoot, macosSmokeBuildArguments());
    developmentSignMacosBundle(smokeBundlePath);
    const bundle = await inspectMacosBundle(smokeBundlePath);
    developmentSignPackagedExecutable(smokeBundlePath);
    console.log(
      JSON.stringify(
        {
          bundle,
          smoke: await smokeMacosBundle(smokeBundlePath),
        },
        null,
        2,
      ),
    );
  } else {
    throw new Error(
      "Use --build, --development-sign, --inspect, or --smoke with an optional repository-relative .app path.",
    );
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    await main();
  } catch (error) {
    console.error(`[bottie] ${error instanceof Error ? error.message : "The macOS package workflow failed."}`);
    process.exitCode = 1;
  }
}
