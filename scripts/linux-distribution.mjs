#!/usr/bin/env node

/** Adds and independently verifies Bottie's protected Linux DEB distribution signature. */

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { closeSync, openSync, readFileSync, statSync } from "node:fs";
import { lstat, readFile, readdir, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import { bindUpdaterArtifactEvidence, signUpdaterArtifact } from "./updater-artifact.mjs";

const DEFAULT_ARTIFACT_DIRECTORY = "package/linux";
const DEFAULT_EVIDENCE_PATH = "package/linux-package-evidence.json";
const KEY_ID_ENVIRONMENT = "BOTTIE_LINUX_SIGNING_KEY_ID";
const KEYRINGS_DIRECTORY_ENVIRONMENT = "BOTTIE_LINUX_SIGNING_KEYRINGS_DIR";
const EMBEDDED_SIGNATURE_PATH_ENVIRONMENT = "BOTTIE_LINUX_EMBEDDED_SIGNATURE_PATH";
const PAYLOAD_PATH_ENVIRONMENT = "BOTTIE_LINUX_SIGNING_PAYLOAD_PATH";
const POLICIES_DIRECTORY_ENVIRONMENT = "BOTTIE_LINUX_SIGNING_POLICIES_DIR";
const PUBLIC_KEYRING_PATH_ENVIRONMENT = "BOTTIE_LINUX_SIGNING_PUBLIC_KEYRING_PATH";
const VERIFICATION_GNUPG_HOME_ENVIRONMENT = "BOTTIE_LINUX_VERIFICATION_GNUPG_HOME";
const CONTROL_MEMBER_PATTERN = /^control\.tar(?:\.gz|\.xz)?$/;
const DATA_MEMBER_PATTERN = /^data\.tar(?:\.gz|\.xz|\.bz2|\.lzma)?$/;
const SHA256_PATTERN = /^[a-f0-9]{64}$/;
const SIGNING_KEY_PATTERN = /^(?:[A-F0-9]{16}|[A-F0-9]{40})$/;

/** Returns the exact embedded-origin signing command for one DEB. */
export function signingArguments(keyId, debPath) {
  return ["--sign=origin", `--default-key=${keyId}`, debPath];
}

/** Returns the independent verification command using caller-owned trust roots. */
export function verificationArguments(policiesDirectory, keyringsDirectory, debPath) {
  return ["--policies-dir", policiesDirectory, "--keyrings-dir", keyringsDirectory, debPath];
}

/** Returns stock GnuPG's exact detached-signature verification arguments. */
export function openPgpVerificationArguments(publicKeyringPath, signaturePath, payloadPath) {
  return [
    "--no-options",
    "--no-default-keyring",
    "--batch",
    "--no-secmem-warning",
    "--no-permission-warning",
    "--no-mdc-warning",
    "--no-auto-check-trustdb",
    "--weak-digest",
    "RIPEMD160",
    "--weak-digest",
    "SHA1",
    "--keyring",
    publicKeyringPath,
    "--verify",
    signaturePath,
    payloadPath,
  ];
}

/** Maps debsig-verify's documented statuses into fixed identity- and path-free failures. */
export function verificationFailureMessage(status) {
  switch (status) {
    case 10:
      return "Linux distribution origin signature is unavailable.";
    case 11:
      return "Linux distribution verification policy root is unavailable.";
    case 12:
      return "Linux distribution verification policy did not select the signature.";
    case 13:
      return "Linux distribution signature verification failed.";
    case 14:
      return "Linux distribution verification backend failed.";
    default:
      return "Linux distribution verification failed.";
  }
}

/** Selects the exact unsigned DEB members that debsig-verify authenticates, in verifier order. */
export function canonicalDebianPayloadMembers(archiveMembers) {
  if (archiveMembers.some((member) => member.startsWith("_gpg"))) {
    throw new Error("Linux distribution signing requires an unsigned Debian archive.");
  }
  if (archiveMembers.filter((member) => member === "debian-binary").length !== 1) {
    throw new Error("Linux distribution signing requires exactly one debian-binary member.");
  }
  const controlMembers = archiveMembers.filter((member) => CONTROL_MEMBER_PATTERN.test(member));
  if (controlMembers.length !== 1) {
    throw new Error("Linux distribution signing requires exactly one supported control archive.");
  }
  const dataMembers = archiveMembers.filter((member) => DATA_MEMBER_PATTERN.test(member));
  if (dataMembers.length !== 1) {
    throw new Error("Linux distribution signing requires exactly one supported data archive.");
  }
  return ["debian-binary", controlMembers[0], dataMembers[0]];
}

/** Returns one independent extraction command per canonical payload member. */
export function canonicalDebianPayloadExtractionArguments(debPath, archiveMembers) {
  return canonicalDebianPayloadMembers(archiveMembers).map((member) => ["p", debPath, member]);
}

/** Reports whether one resolved path is inside the repository checkout. */
function isRepositoryPath(repositoryRoot, candidate) {
  const relativePath = relative(resolve(repositoryRoot), resolve(candidate));
  return (
    relativePath === "" || (!isAbsolute(relativePath) && relativePath !== ".." && !relativePath.startsWith(`..${sep}`))
  );
}

/** Resolves protected signing identity and public verification roots without reading private key bytes. */
export function resolveSigningConfiguration(environment, repositoryRoot) {
  const keyId = environment[KEY_ID_ENVIRONMENT]?.trim().toUpperCase();
  const embeddedSignaturePath = environment[EMBEDDED_SIGNATURE_PATH_ENVIRONMENT]?.trim();
  const policiesDirectory = environment[POLICIES_DIRECTORY_ENVIRONMENT]?.trim();
  const keyringsDirectory = environment[KEYRINGS_DIRECTORY_ENVIRONMENT]?.trim();
  const payloadPath = environment[PAYLOAD_PATH_ENVIRONMENT]?.trim();
  const publicKeyringPath = environment[PUBLIC_KEYRING_PATH_ENVIRONMENT]?.trim();
  const verificationGnupgHome = environment[VERIFICATION_GNUPG_HOME_ENVIRONMENT]?.trim();
  if (
    !keyId ||
    !embeddedSignaturePath ||
    !policiesDirectory ||
    !keyringsDirectory ||
    !payloadPath ||
    !publicKeyringPath ||
    !verificationGnupgHome
  ) {
    throw new Error("Protected Linux signing configuration is unavailable.");
  }
  if (!SIGNING_KEY_PATTERN.test(keyId)) throw new Error("The Linux signing key identity is invalid.");
  for (const protectedPath of [
    embeddedSignaturePath,
    policiesDirectory,
    keyringsDirectory,
    payloadPath,
    publicKeyringPath,
    verificationGnupgHome,
  ]) {
    if (!isAbsolute(protectedPath) || isRepositoryPath(repositoryRoot, protectedPath)) {
      throw new Error("Linux signing and verification paths must stay outside the repository.");
    }
  }
  return {
    embeddedSignaturePath,
    keyId,
    keyringsDirectory,
    payloadPath,
    policiesDirectory,
    publicKeyringPath,
    verificationGnupgHome,
  };
}

/** Returns a copy of unsigned package evidence bound to independently verified signed bytes. */
export function verifiedLinuxDistributionEvidence(evidence, signedPackage) {
  const installer = evidence?.bundle?.installer;
  if (
    evidence?.schemaVersion !== 1 ||
    evidence.version !== "0.9.0" ||
    installer?.metadata?.package !== "bottie" ||
    installer.signature?.classification !== "unsigned" ||
    installer.signature?.verifies !== false
  ) {
    throw new Error("Linux distribution signing requires unsigned Bottie package evidence.");
  }
  if (
    !SHA256_PATTERN.test(signedPackage?.sha256) ||
    !Number.isSafeInteger(signedPackage?.size) ||
    signedPackage.size <= 0
  ) {
    throw new Error("Verified Linux distribution bytes are invalid.");
  }
  return {
    ...evidence,
    bundle: {
      ...evidence.bundle,
      installer: {
        ...installer,
        sha256: signedPackage.sha256,
        signature: { classification: "identified", verifies: true },
        size: signedPackage.size,
      },
    },
  };
}

/** Runs one host tool while discarding identity-, path-, and credential-bearing output. */
function runHostCommand(command, arguments_, failureMessage, environment = {}) {
  const result = spawnSync(command, arguments_, {
    encoding: "utf8",
    env: { ...process.env, ...environment },
  });
  if (result.error || result.status !== 0) {
    throw new Error(typeof failureMessage === "function" ? failureMessage(result.status) : failureMessage);
  }
  return `${result.stdout ?? ""}${result.stderr ?? ""}`;
}

/** Requires exactly one origin signature whose embedded bytes match the signer output. */
function requireOriginSignature(debPath, expectedSignaturePath) {
  const signatures = runHostCommand("ar", ["t", debPath], "Linux distribution archive inspection failed.")
    .split(/\r?\n/)
    .filter((member) => member.startsWith("_gpg"));
  if (signatures.length !== 1 || signatures[0] !== "_gpgorigin") {
    throw new Error("The Linux distribution archive does not contain exactly one origin signature.");
  }
  const extracted = spawnSync("ar", ["p", debPath, "_gpgorigin"]);
  try {
    if (
      extracted.error ||
      extracted.status !== 0 ||
      !Buffer.isBuffer(extracted.stdout) ||
      !extracted.stdout.equals(readFileSync(expectedSignaturePath))
    ) {
      throw new Error();
    }
  } catch {
    throw new Error("The embedded Linux origin signature differs from the signer output.");
  }
}

/** Writes the verifier's exact three-member payload without depending on archive member order. */
function writeCanonicalDebianPayload(debPath, payloadPath) {
  const archiveMembers = runHostCommand("ar", ["t", debPath], "Linux distribution archive inspection failed.")
    .split(/\r?\n/)
    .filter(Boolean);
  const extractions = canonicalDebianPayloadExtractionArguments(debPath, archiveMembers);
  const payloadDescriptor = openSync(payloadPath, "w", 0o600);
  try {
    for (const arguments_ of extractions) {
      const result = spawnSync("ar", arguments_, {
        stdio: ["ignore", payloadDescriptor, "ignore"],
      });
      if (result.error || result.status !== 0) {
        throw new Error("Linux distribution canonical payload extraction failed.");
      }
    }
  } finally {
    closeSync(payloadDescriptor);
  }
  if (statSync(payloadPath).size === 0) {
    throw new Error("Linux distribution canonical payload extraction failed.");
  }
}

/** Signs, requires the embedded origin member, then independently verifies one DEB. */
export function signAndVerifyLinuxDistribution(
  configuration,
  debPath,
  commandRunner = runHostCommand,
  signatureRequirement = requireOriginSignature,
  payloadWriter = writeCanonicalDebianPayload,
) {
  payloadWriter(debPath, configuration.payloadPath);
  commandRunner("debsigs", signingArguments(configuration.keyId, debPath), "Linux distribution signing failed.");
  signatureRequirement(debPath, configuration.embeddedSignaturePath);
  commandRunner(
    "/usr/bin/gpg",
    openPgpVerificationArguments(
      configuration.publicKeyringPath,
      configuration.embeddedSignaturePath,
      configuration.payloadPath,
    ),
    "Embedded Linux origin signature verification failed.",
    { GNUPGHOME: configuration.verificationGnupgHome },
  );
  commandRunner(
    "debsig-verify",
    verificationArguments(configuration.policiesDirectory, configuration.keyringsDirectory, debPath),
    verificationFailureMessage,
    { DEBSIG_GNUPG_PROGRAM: "/usr/bin/gpg", GNUPGHOME: configuration.verificationGnupgHome },
  );
}

/** Finds exactly one regular DEB in the protected workflow's bounded artifact directory. */
async function findSingleDeb(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const packages = entries.filter((entry) => entry.isFile() && entry.name.toLowerCase().endsWith(".deb"));
  if (packages.length !== 1) throw new Error("Linux distribution signing requires exactly one DEB artifact.");
  const debPath = join(directory, packages[0].name);
  if (!(await lstat(debPath)).isFile()) throw new Error("The Linux distribution artifact is unavailable.");
  return debPath;
}

/** Resolves a required path below the ignored repository package directory. */
function resolvePackagePath(repositoryRoot, suppliedPath, description) {
  const resolvedPath = resolve(repositoryRoot, suppliedPath);
  const packageRoot = resolve(repositoryRoot, "package");
  const relativePath = relative(packageRoot, resolvedPath);
  if (relativePath === "" || relativePath === ".." || relativePath.startsWith(`..${sep}`)) {
    throw new Error(`${description} must stay inside the repository package directory.`);
  }
  return resolvedPath;
}

/** Hashes the exact signed package without retaining its filename or host path. */
async function signedPackageSummary(debPath) {
  const bytes = await readFile(debPath);
  return { sha256: createHash("sha256").update(bytes).digest("hex"), size: bytes.length };
}

/** Signs, independently verifies, and replaces only the signature fields in bounded package evidence. */
async function runLinuxDistribution(repositoryRoot) {
  const configuration = resolveSigningConfiguration(process.env, repositoryRoot);
  const artifactDirectory = resolvePackagePath(
    repositoryRoot,
    process.env.BOTTIE_LINUX_ARTIFACT_DIRECTORY?.trim() || DEFAULT_ARTIFACT_DIRECTORY,
    "Linux distribution artifacts",
  );
  const evidencePath = resolvePackagePath(
    repositoryRoot,
    process.env.BOTTIE_LINUX_EVIDENCE_PATH?.trim() || DEFAULT_EVIDENCE_PATH,
    "Linux distribution evidence",
  );
  const evidence = JSON.parse(await readFile(evidencePath, "utf8"));
  const debPath = await findSingleDeb(artifactDirectory);
  signAndVerifyLinuxDistribution(configuration, debPath);
  const updater = await signUpdaterArtifact(repositoryRoot, debPath);
  const signedPackage = await signedPackageSummary(debPath);
  const verifiedEvidence = {
    ...verifiedLinuxDistributionEvidence(evidence, signedPackage),
    updater: bindUpdaterArtifactEvidence(updater, "linux-x86_64", signedPackage.sha256),
  };
  await writeFile(evidencePath, `${JSON.stringify(verifiedEvidence, null, 2)}\n`, { mode: 0o600 });
}

/** Accepts only the deliberate protected-runner mode on Linux. */
async function main() {
  if (process.platform !== "linux") throw new Error("Linux distribution validation requires a Linux host.");
  if (process.argv.slice(2).length !== 1 || process.argv[2] !== "--run") throw new Error("Use the exact --run mode.");
  const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  await runLinuxDistribution(repositoryRoot);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    await main();
  } catch (error) {
    console.error(`[bottie] ${error instanceof Error ? error.message : "Linux distribution validation failed."}`);
    process.exitCode = 1;
  }
}
