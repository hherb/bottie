/** Protected Tauri updater signing for exact final platform-distribution bytes. */

import { spawnSync } from "node:child_process";
import { lstat, rm } from "node:fs/promises";
import { isAbsolute, join, relative, resolve, sep } from "node:path";

const PRIVATE_KEY_CONTENT_ENVIRONMENT = "TAURI_SIGNING_PRIVATE_KEY";
const PRIVATE_KEY_PATH_ENVIRONMENT = "TAURI_SIGNING_PRIVATE_KEY_PATH";
const PRIVATE_KEY_PASSWORD_ENVIRONMENT = "TAURI_SIGNING_PRIVATE_KEY_PASSWORD";

/** Validates one protected private-key source and password without reading either value. */
export function requireUpdaterSigningEnvironment(environment, repositoryRoot) {
  const privateKey = environment[PRIVATE_KEY_CONTENT_ENVIRONMENT]?.trim();
  const privateKeyPath = environment[PRIVATE_KEY_PATH_ENVIRONMENT]?.trim();
  const password = environment[PRIVATE_KEY_PASSWORD_ENVIRONMENT];
  if (Boolean(privateKey) === Boolean(privateKeyPath) || !password) {
    throw new Error("Protected updater signing credentials are unavailable or ambiguous.");
  }
  if (privateKeyPath) {
    if (!isAbsolute(privateKeyPath)) throw new Error("The updater private-key path must be absolute.");
    const relativePath = relative(resolve(repositoryRoot), resolve(privateKeyPath));
    const insideRepository =
      relativePath === "" ||
      (!isAbsolute(relativePath) && relativePath !== ".." && !relativePath.startsWith(`..${sep}`));
    if (insideRepository) throw new Error("The updater private key must stay outside the repository.");
  }
  return { source: privateKeyPath ? "protected-path" : "protected-content" };
}

/** Returns the credential-free Tauri CLI arguments for one exact final artifact. */
export function updaterSigningArguments(artifactPath) {
  return ["--tauri", "signer", "sign", artifactPath];
}

/** Signs one final platform artifact and returns only its adjacent signature path. */
export async function signUpdaterArtifact(repositoryRoot, artifactPath, environment = process.env) {
  requireUpdaterSigningEnvironment(environment, repositoryRoot);
  const signaturePath = `${artifactPath}.sig`;
  await requireRegularFile(artifactPath);
  await rm(signaturePath, { force: true });
  const wrapper = join(repositoryRoot, "scripts", "macos-development-signing.mjs");
  const result = spawnSync(process.execPath, [wrapper, ...updaterSigningArguments(artifactPath)], {
    cwd: repositoryRoot,
    env: environment,
    stdio: ["ignore", "ignore", "inherit"],
  });
  if (result.error || result.status !== 0) throw new Error("Tauri updater signing failed.");
  await requireRegularFile(signaturePath);
  return signaturePath;
}

/** Requires one regular protected artifact while retaining no path in the failure. */
async function requireRegularFile(path) {
  try {
    if ((await lstat(path)).isFile()) return;
  } catch {
    // Reduce absent and unreadable artifacts to one fixed failure.
  }
  throw new Error("The protected updater artifact is unavailable.");
}
