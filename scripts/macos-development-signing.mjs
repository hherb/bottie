#!/usr/bin/env node

/** Signs freshly linked macOS development executables before Cargo runs them. */

import { spawn, spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const APPLE_DEVELOPMENT_PREFIX = "Apple Development:";
const DEVELOPMENT_IDENTIFIER = "com.bottie.app.dev";
const SIGNATURE_PAGE_SIZE = "4096";
const RUNNER_ENVIRONMENTS = ["CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER", "CARGO_TARGET_X86_64_APPLE_DARWIN_RUNNER"];

/** Parses usable code-signing identities without returning certificate labels to callers. */
export function selectAppleDevelopmentIdentity(output, requestedIdentity = "") {
  const identities = [...output.matchAll(/^\s*\d+\)\s+([0-9A-F]{40})\s+"([^"]+)"/gim)]
    .map((match) => ({ fingerprint: match[1].toUpperCase(), label: match[2] }))
    .filter((identity) => identity.label.startsWith(APPLE_DEVELOPMENT_PREFIX));
  if (requestedIdentity) {
    const normalized = requestedIdentity.toUpperCase();
    const selected = identities.find(
      (identity) => identity.fingerprint === normalized || identity.label === requestedIdentity,
    );
    if (!selected) {
      throw new Error("BOTTIE_APPLE_SIGNING_IDENTITY does not match a usable Apple Development identity.");
    }
    return selected.fingerprint;
  }
  if (identities.length === 0) {
    throw new Error("No usable Apple Development signing identity is available in the active keychains.");
  }
  if (identities.length !== 1) {
    throw new Error("Set BOTTIE_APPLE_SIGNING_IDENTITY because multiple Apple Development identities are available.");
  }
  return identities[0].fingerprint;
}

/** Reports whether this Tauri invocation needs the macOS development Cargo runner. */
export function shouldConfigureDevelopmentSigning(platform, arguments_) {
  return platform === "darwin" && arguments_[0] === "dev";
}

/** Builds Cargo's literal runner command without recording a certificate identity. */
export function cargoRunnerValue(nodePath, scriptPath) {
  if (/\s/.test(nodePath) || /\s/.test(scriptPath)) {
    throw new Error("Bottie's macOS Cargo runner cannot use Node or repository paths containing whitespace.");
  }
  return `${nodePath} ${scriptPath} --cargo-runner`;
}

/** Resolves the executable entry point beside Tauri's exported package API. */
export function resolveTauriCliPath(packageEntryPath) {
  return join(dirname(packageEntryPath), "tauri.js");
}

/** Runs one child process and mirrors its terminal lifecycle. */
function runChild(command, arguments_, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, arguments_, { stdio: "inherit", ...options });
    const forwardInterrupt = () => {
      if (!child.killed) child.kill("SIGINT");
    };
    const forwardTermination = () => {
      if (!child.killed) child.kill("SIGTERM");
    };
    const removeSignalListeners = () => {
      process.removeListener("SIGINT", forwardInterrupt);
      process.removeListener("SIGTERM", forwardTermination);
    };
    process.once("SIGINT", forwardInterrupt);
    process.once("SIGTERM", forwardTermination);
    child.once("error", (error) => {
      removeSignalListeners();
      reject(error);
    });
    child.once("exit", (code, signal) => {
      removeSignalListeners();
      if (signal === "SIGINT") resolve(130);
      else if (signal === "SIGTERM") resolve(143);
      else if (signal) resolve(128);
      else resolve(code ?? 1);
    });
  });
}

/** Runs the real Tauri CLI while adding signing only to macOS development builds. */
async function runTauri(arguments_) {
  const environment = { ...process.env };
  if (shouldConfigureDevelopmentSigning(process.platform, arguments_)) {
    const runner = cargoRunnerValue(process.execPath, fileURLToPath(import.meta.url));
    for (const name of RUNNER_ENVIRONMENTS) {
      if (environment[name] && environment[name] !== runner) {
        throw new Error(`${name} is already set; Bottie will not replace an existing Cargo runner.`);
      }
      environment[name] = runner;
    }
  }
  const tauriCli = resolveTauriCliPath(fileURLToPath(import.meta.resolve("@tauri-apps/cli")));
  return runChild(process.execPath, [tauriCli, ...arguments_], { env: environment });
}

/** Signs and verifies the exact freshly linked executable before replacing the runner process. */
async function signAndRun(arguments_) {
  const [executable, ...executableArguments] = arguments_;
  if (!executable) throw new Error("Cargo did not supply an executable to the Bottie development runner.");
  const identities = spawnSync("security", ["find-identity", "-v", "-p", "codesigning"], {
    encoding: "utf8",
  });
  if (identities.status !== 0) throw new Error("Bottie could not inspect the active code-signing identities.");
  const identity = selectAppleDevelopmentIdentity(identities.stdout, process.env.BOTTIE_APPLE_SIGNING_IDENTITY);
  const signing = spawnSync(
    "codesign",
    [
      "--force",
      "--sign",
      identity,
      "--identifier",
      DEVELOPMENT_IDENTIFIER,
      "--options",
      "runtime",
      "--pagesize",
      SIGNATURE_PAGE_SIZE,
      "--timestamp=none",
      executable,
    ],
    { stdio: "inherit" },
  );
  if (signing.status !== 0) throw new Error("Bottie could not development-sign the freshly linked executable.");
  const verification = spawnSync("codesign", ["--verify", "--strict", executable], { stdio: "inherit" });
  if (verification.status !== 0) throw new Error("The development-signed Bottie executable failed verification.");
  return runChild(executable, executableArguments);
}

/** Dispatches package-script and Cargo-runner modes. */
async function main() {
  const [mode, ...arguments_] = process.argv.slice(2);
  if (mode === "--tauri") return runTauri(arguments_);
  if (mode === "--cargo-runner") return signAndRun(arguments_);
  throw new Error("Use this script through npm run tauri or Cargo's configured development runner.");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = await main();
  } catch (error) {
    console.error(`[bottie] ${error instanceof Error ? error.message : "Development signing failed."}`);
    process.exitCode = 1;
  }
}
