#!/usr/bin/env node

/** Signs, notarizes, staples, and verifies Bottie's existing macOS application bundle. */

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import { inspectBundleFiles, macosUpdaterBuildArguments } from "./macos-package.mjs";
import { packagedBundleLayout } from "./macos-packaged-python-smoke.mjs";
import { requireMatchingMacosProtectedInspection } from "./macos-shipping-python-containment.mjs";
import { validateProtectedPythonInspection } from "./python-protected-package.mjs";
import { inspectPackagedPythonBundle, validateRuntimeManifest } from "./python-runtime-bundle.mjs";
import { bindUpdaterArtifactEvidence, exportUpdaterArtifact, signUpdaterArtifact } from "./updater-artifact.mjs";

const DEVELOPER_ID_APPLICATION_PREFIX = "Developer ID Application:";
const DEFAULT_BUNDLE_PATH = "src-tauri/target/release/bundle/macos/bottie.app";
const DEFAULT_EVIDENCE_PATH = "package/macos-distribution-evidence.json";
const ENTITLEMENTS_PATH = "src-tauri/Entitlements.plist";
const NOTARIZATION_ARCHIVE_PATH = "package/macos/bottie-notarization.zip";
const NOTARIZATION_TIMEOUT = "30m";
const UPDATER_ARCHIVE_PATH = "src-tauri/target/release/bundle/macos/bottie.app.tar.gz";

/** Returns the exact inside-out Developer ID signing plan for protected Python nested code. */
export function protectedPythonDistributionSigningPlan(repositoryRoot, applicationRoot, identity) {
  const layout = packagedBundleLayout(applicationRoot);
  const entitlementRoot = join(resolve(repositoryRoot), "macos-python-xpc");
  const signingPrefix = ["--force", "--sign", identity, "--options", "runtime", "--timestamp"];
  return [
    {
      arguments: [...signingPrefix, "--entitlements", join(entitlementRoot, "Runner.entitlements"), layout.runner],
      label: "packaged Python runner",
      path: layout.runner,
    },
    {
      arguments: [...signingPrefix, "--entitlements", join(entitlementRoot, "Service.entitlements"), layout.service],
      label: "packaged Python XPC service",
      path: layout.service,
    },
    {
      arguments: [...signingPrefix, layout.application],
      label: "packaged Python XPC client",
      path: layout.application,
    },
  ];
}

/** Selects one usable Developer ID Application identity without returning its certificate label. */
export function selectDeveloperIdApplicationIdentity(output, requestedIdentity = "") {
  const identities = [...output.matchAll(/^\s*\d+\)\s+([0-9A-F]{40})\s+"([^"]+)"/gim)]
    .map((match) => ({ fingerprint: match[1].toUpperCase(), label: match[2] }))
    .filter((identity) => identity.label.startsWith(DEVELOPER_ID_APPLICATION_PREFIX));
  if (requestedIdentity) {
    const normalized = requestedIdentity.toUpperCase();
    const selected = identities.find(
      (identity) => identity.fingerprint === normalized || identity.label === requestedIdentity,
    );
    if (!selected) {
      throw new Error("BOTTIE_APPLE_DISTRIBUTION_IDENTITY does not match a usable Developer ID Application identity.");
    }
    return selected.fingerprint;
  }
  if (identities.length === 0) {
    throw new Error("No usable Developer ID Application signing identity is available in the active keychains.");
  }
  if (identities.length !== 1) {
    throw new Error(
      "Set BOTTIE_APPLE_DISTRIBUTION_IDENTITY because multiple Developer ID Application identities are available.",
    );
  }
  return identities[0].fingerprint;
}

/** Returns the exact hardened-runtime Developer ID signing arguments for one application bundle. */
export function distributionSigningArguments(identity, entitlementsPath, bundlePath) {
  return [
    "--force",
    "--sign",
    identity,
    "--options",
    "runtime",
    "--timestamp",
    "--entitlements",
    entitlementsPath,
    bundlePath,
  ];
}

/** Resolves exactly one credential source without reading or serializing credential contents. */
export function resolveNotaryAuthentication(environment, repositoryRoot = "") {
  const profile = environment.BOTTIE_APPLE_NOTARY_PROFILE?.trim();
  const keyPath = environment.BOTTIE_APPLE_NOTARY_KEY_PATH?.trim();
  const keyId = environment.BOTTIE_APPLE_NOTARY_KEY_ID?.trim();
  const issuerId = environment.BOTTIE_APPLE_NOTARY_ISSUER_ID?.trim();
  const hasAnyApiKeyField = Boolean(keyPath || keyId || issuerId);
  const hasCompleteApiKey = Boolean(keyPath && keyId && issuerId);
  if (profile && hasAnyApiKeyField) throw new Error("Supply exactly one Apple notary credential mode.");
  if (profile) return ["--keychain-profile", profile];
  if (hasCompleteApiKey) {
    const relativeKeyPath = repositoryRoot ? relative(resolve(repositoryRoot), resolve(keyPath)) : "..";
    const isRepositoryPath =
      relativeKeyPath === "" ||
      (!isAbsolute(relativeKeyPath) && relativeKeyPath !== ".." && !relativeKeyPath.startsWith(`..${sep}`));
    if (isRepositoryPath) throw new Error("The Apple notary private key must stay outside the repository.");
    return ["--key", keyPath, "--key-id", keyId, "--issuer", issuerId];
  }
  if (hasAnyApiKeyField) throw new Error("Protected Apple notary API credentials are incomplete.");
  throw new Error("Apple notary credentials are unavailable on the invoking host.");
}

/** Returns one bounded structured notary submission command after the xcrun prefix. */
export function notarySubmitArguments(archivePath, authenticationArguments) {
  return [
    "notarytool",
    "submit",
    archivePath,
    ...authenticationArguments,
    "--wait",
    "--timeout",
    NOTARIZATION_TIMEOUT,
    "--no-progress",
    "--output-format",
    "json",
  ];
}

/** Reduces Apple's structured submission response to identity-free accepted-state evidence. */
export function parseNotaryResult(output) {
  let result;
  try {
    result = JSON.parse(output);
  } catch {
    throw new Error("Apple did not return a structured notarization result.");
  }
  if (!result || result.status !== "Accepted") throw new Error("Apple's notarization submission was not accepted.");
  return { accepted: true, status: "accepted" };
}

/** Returns the exact stapling or ticket-validation arguments after the xcrun prefix. */
export function staplerArguments(mode, bundlePath) {
  if (mode !== "staple" && mode !== "validate") throw new Error("Unsupported stapler mode.");
  return ["stapler", mode, "-v", bundlePath];
}

/** Returns the final-app archive command used before production updater signing. */
export function updaterArchiveArguments(bundlePath, archivePath) {
  return ["-czf", archivePath, "-C", dirname(bundlePath), basename(bundlePath)];
}

/** Maps one exact packaged architecture to Tauri's matching static-manifest target. */
export function macosUpdaterTarget(architectures) {
  if (architectures.length === 1 && architectures[0] === "arm64") return "darwin-aarch64";
  if (architectures.length === 1 && architectures[0] === "x86_64") return "darwin-x86_64";
  throw new Error("macOS updater evidence requires one single supported architecture.");
}

/** Reduces Gatekeeper output to accepted notarized-Developer-ID evidence without identities or paths. */
export function classifyGatekeeperAssessment(status, output) {
  const accepted = status === 0 && /source=Notarized Developer ID/i.test(output);
  return { accepted, source: accepted ? "notarized-developer-id" : "rejected" };
}

/** Runs a host command while keeping raw identity- and path-bearing output inside this process. */
function runHostCommand(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, { encoding: "utf8", ...options });
  if (result.error || result.status !== 0) throw new Error(`${command} failed during macOS distribution verification.`);
  return `${result.stdout ?? ""}${result.stderr ?? ""}`;
}

/** Runs a structured-output command and discards stderr before its JSON crosses the parser boundary. */
function runStructuredHostCommand(command, arguments_) {
  const result = spawnSync(command, arguments_, { encoding: "utf8" });
  if (result.error || result.status !== 0) throw new Error(`${command} failed during macOS distribution verification.`);
  return result.stdout ?? "";
}

/** Signs and independently verifies protected Python nested code before the outer app. */
function signProtectedPythonCode(repositoryRoot, bundlePath, identity) {
  for (const step of protectedPythonDistributionSigningPlan(repositoryRoot, bundlePath, identity)) {
    runHostCommand("/usr/bin/codesign", step.arguments);
    runHostCommand("/usr/bin/codesign", ["--verify", "--strict", "--verbose=2", step.path]);
  }
}

/** Builds the same locked unsigned application bundle used by local packaging. */
function buildUnsignedBundle(repositoryRoot) {
  const script = join(repositoryRoot, "scripts", "macos-development-signing.mjs");
  const result = spawnSync(process.execPath, [script, "--tauri", ...macosUpdaterBuildArguments()], {
    cwd: repositoryRoot,
    stdio: "inherit",
  });
  if (result.error || result.status !== 0) throw new Error("The locked unsigned macOS bundle build failed.");
}

/** Reads one public plist value without retaining a host path. */
function readPlistValue(bundlePath, key) {
  const plistPath = join(bundlePath, "Contents", "Info.plist");
  return runHostCommand("plutil", ["-extract", key, "raw", "-o", "-", plistPath]).trim();
}

/** Verifies the final Developer ID signature and reduces it to identity-free policy evidence. */
function inspectDistributionSignature(bundlePath) {
  runHostCommand("codesign", ["--verify", "--deep", "--strict", "--verbose=2", bundlePath]);
  const output = runHostCommand("codesign", ["--display", "--verbose=4", bundlePath]);
  const evidence = {
    classification: /Authority=Developer ID Application:/i.test(output) ? "developer-id-application" : "unknown",
    hardenedRuntime: /flags=.*runtime/i.test(output),
    secureTimestamp: /^Timestamp=(?!none)/im.test(output),
    verifies: true,
  };
  if (evidence.classification === "unknown" || !evidence.hardenedRuntime || !evidence.secureTimestamp) {
    throw new Error("The application signature does not satisfy Bottie's Developer ID distribution policy.");
  }
  return evidence;
}

/** Creates one canonical ZIP submission and returns only its hash and byte count. */
async function createNotarizationArchive(bundlePath, archivePath) {
  await mkdir(dirname(archivePath), { recursive: true });
  await rm(archivePath, { force: true });
  runHostCommand("ditto", ["-c", "-k", "--keepParent", bundlePath, archivePath]);
  const bytes = await readFile(archivePath);
  return { sha256: createHash("sha256").update(bytes).digest("hex"), size: bytes.length };
}

/** Recreates and signs the updater archive only after notarization and ticket stapling. */
async function createUpdaterArchive(repositoryRoot, bundlePath, archivePath) {
  await rm(archivePath, { force: true });
  await rm(`${archivePath}.sig`, { force: true });
  runHostCommand("tar", updaterArchiveArguments(bundlePath, archivePath));
  return signUpdaterArtifact(repositoryRoot, archivePath);
}

/** Submits the archive, staples its ticket, and verifies the final Gatekeeper decision. */
function notarizeAndVerify(bundlePath, archivePath, authenticationArguments) {
  const submissionOutput = runStructuredHostCommand(
    "xcrun",
    notarySubmitArguments(archivePath, authenticationArguments),
  );
  const submission = parseNotaryResult(submissionOutput);
  runHostCommand("xcrun", staplerArguments("staple", bundlePath));
  runHostCommand("xcrun", staplerArguments("validate", bundlePath));
  const assessment = spawnSync("spctl", ["--assess", "--type", "execute", "--verbose=4", bundlePath], {
    encoding: "utf8",
  });
  const gatekeeper = classifyGatekeeperAssessment(
    assessment.status,
    `${assessment.stdout ?? ""}${assessment.stderr ?? ""}`,
  );
  if (!gatekeeper.accepted) throw new Error("Gatekeeper did not accept the notarized Bottie application bundle.");
  return { gatekeeper, submission, ticketStapled: true, ticketValid: true };
}

/** Produces one path-safe, identity-free record of the final stapled bundle. */
async function createDistributionEvidence(bundlePath, entitlementsPath, archive, notarization, updater) {
  const executable = readPlistValue(bundlePath, "CFBundleExecutable");
  const architectures = runHostCommand("lipo", ["-archs", join(bundlePath, "Contents", "MacOS", executable)])
    .trim()
    .split(/\s+/);
  const entitlements = await readFile(entitlementsPath);
  return {
    schemaVersion: 1,
    artifact: await inspectBundleFiles(bundlePath),
    metadata: {
      architectures,
      identifier: readPlistValue(bundlePath, "CFBundleIdentifier"),
      version: readPlistValue(bundlePath, "CFBundleShortVersionString"),
    },
    notarization: { ...notarization, submittedArchive: archive },
    signing: {
      ...inspectDistributionSignature(bundlePath),
      entitlementsSha256: createHash("sha256").update(entitlements).digest("hex"),
    },
    updater: bindUpdaterArtifactEvidence(updater, macosUpdaterTarget(architectures)),
  };
}

/** Reads one required protected Python JSON input without reflecting its host path. */
async function readProtectedPythonJson(path) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch {
    throw new Error("Required protected macOS Python evidence is unavailable or malformed.");
  }
}

/** Inspects the exact protected Python bytes against the pinned runtime manifest. */
async function inspectProtectedPythonBundle(repositoryRoot, bundlePath) {
  const manifest = validateRuntimeManifest(
    await readProtectedPythonJson(join(repositoryRoot, "python-runner", "runtime-manifest.json")),
  );
  return inspectPackagedPythonBundle(bundlePath, "macos", manifest);
}

/** Loads and revalidates the carried candidate and unsigned protected inspection. */
async function loadProtectedPythonContext(repositoryRoot, bundlePath, configuration) {
  const candidate = await readProtectedPythonJson(configuration.candidatePath);
  const suppliedInspection = validateProtectedPythonInspection(
    configuration.sourceSha,
    "macos",
    candidate,
    await readProtectedPythonJson(configuration.inspectionPath),
  );
  requireMatchingMacosProtectedInspection(
    suppliedInspection,
    await inspectProtectedPythonBundle(repositoryRoot, bundlePath),
  );
  return { candidate };
}

/** Writes the final candidate-validated signed inspection with private permissions. */
async function writeProtectedPythonInspection(path, inspection) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(inspection, null, 2)}\n`, { mode: 0o600 });
}

/** Runs the credential-dependent distribution contract without publishing the application. */
async function runDistribution(repositoryRoot, protectedPython = null) {
  const bundlePath = resolve(repositoryRoot, DEFAULT_BUNDLE_PATH);
  const entitlementsPath = resolve(repositoryRoot, ENTITLEMENTS_PATH);
  const archivePath = resolve(repositoryRoot, NOTARIZATION_ARCHIVE_PATH);
  const evidencePath = resolve(repositoryRoot, DEFAULT_EVIDENCE_PATH);
  await rm(evidencePath, { force: true });
  if (protectedPython) await rm(protectedPython.outputPath, { force: true });
  const protectedContext = protectedPython
    ? await loadProtectedPythonContext(repositoryRoot, bundlePath, protectedPython)
    : null;
  if (!protectedContext) buildUnsignedBundle(repositoryRoot);
  const identities = spawnSync("security", ["find-identity", "-v", "-p", "codesigning"], { encoding: "utf8" });
  if (identities.error || identities.status !== 0) {
    throw new Error("Bottie could not inspect the active code-signing identities.");
  }
  const identity = selectDeveloperIdApplicationIdentity(
    identities.stdout,
    process.env.BOTTIE_APPLE_DISTRIBUTION_IDENTITY,
  );
  const authenticationArguments = resolveNotaryAuthentication(process.env, repositoryRoot);
  if (protectedContext) signProtectedPythonCode(repositoryRoot, bundlePath, identity);
  runHostCommand("codesign", distributionSigningArguments(identity, entitlementsPath, bundlePath));
  inspectDistributionSignature(bundlePath);
  try {
    const archive = await createNotarizationArchive(bundlePath, archivePath);
    const notarization = notarizeAndVerify(bundlePath, archivePath, authenticationArguments);
    const updater = await createUpdaterArchive(
      repositoryRoot,
      bundlePath,
      resolve(repositoryRoot, UPDATER_ARCHIVE_PATH),
    );
    const evidence = await createDistributionEvidence(bundlePath, entitlementsPath, archive, notarization, updater);
    await exportUpdaterArtifact(
      repositoryRoot,
      resolve(repositoryRoot, UPDATER_ARCHIVE_PATH),
      macosUpdaterTarget(evidence.metadata.architectures),
      evidence.metadata.version,
    );
    await mkdir(dirname(evidencePath), { recursive: true });
    await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
    if (protectedContext) {
      const inspection = validateProtectedPythonInspection(
        protectedPython.sourceSha,
        "macos",
        protectedContext.candidate,
        await inspectProtectedPythonBundle(repositoryRoot, bundlePath),
      );
      await writeProtectedPythonInspection(protectedPython.outputPath, inspection);
    }
    return evidence;
  } finally {
    await rm(archivePath, { force: true });
  }
}

/** Dispatches the single explicit distribution-validation mode. */
async function main() {
  if (process.platform !== "darwin") throw new Error("The macOS distribution workflow requires a macOS host.");
  const [mode, ...arguments_] = process.argv.slice(2);
  const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  if (mode === "--run" && arguments_.length === 0) {
    console.log(JSON.stringify(await runDistribution(repositoryRoot), null, 2));
    return;
  }
  if (mode === "--run-python" && arguments_.length === 4) {
    const [sourceSha, candidatePath, inspectionPath, outputPath] = arguments_;
    console.log(
      JSON.stringify(
        await runDistribution(repositoryRoot, {
          candidatePath: resolve(candidatePath),
          inspectionPath: resolve(inspectionPath),
          outputPath: resolve(outputPath),
          sourceSha,
        }),
        null,
        2,
      ),
    );
    return;
  }
  throw new Error("Use the exact --run mode or --run-python with source, candidate, inspection, and output.");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    await main();
  } catch (error) {
    console.error(`[bottie] ${error instanceof Error ? error.message : "macOS distribution validation failed."}`);
    process.exitCode = 1;
  }
}
