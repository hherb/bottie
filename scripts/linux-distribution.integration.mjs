#!/usr/bin/env node

/** Exercises Bottie's real Ubuntu debsigs and debsig-verify path with an ephemeral test-only key. */

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmod, copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { openPgpVerificationArguments, signAndVerifyLinuxDistribution } from "./linux-distribution.mjs";

const REPOSITORY_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const COMMAND_TIMEOUT_MILLISECONDS = 60_000;
const EXPECTED_DIGEST_ALGORITHM_ID = "8";
const EXPECTED_WEAK_DIGEST_ALGORITHM_ID = "2";
const EXPECTED_SIGNATURE_MEMBER = "_gpgorigin";
const TEST_KEY_PASSPHRASE = "bottie-linux-distribution-integration-only";
const TEST_KEY_USER_ID = "Bottie Linux distribution integration test";
const UPPERCASE_FINGERPRINT_PATTERN = /^[A-F0-9]{40}$/;

/** Runs one required host command while retaining no command output on failure. */
function runTextCommand(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    encoding: "utf8",
    env: { ...process.env, ...options.environment },
    input: options.input,
    timeout: COMMAND_TIMEOUT_MILLISECONDS,
  });
  if (result.error || result.status !== 0) {
    throw new Error("A required Linux distribution integration command failed.");
  }
  return result.stdout ?? "";
}

/** Returns one host command's status without retaining its output. */
function commandStatus(command, arguments_, environment = {}) {
  const result = spawnSync(command, arguments_, {
    encoding: "utf8",
    env: { ...process.env, ...environment },
    timeout: COMMAND_TIMEOUT_MILLISECONDS,
  });
  return result.error ? null : result.status;
}

/** Extracts the primary fingerprint from GnuPG's stable colon format. */
function primaryFingerprint(listing) {
  let primaryKeySeen = false;
  for (const line of listing.split(/\r?\n/)) {
    const fields = line.split(":");
    if (fields[0] === "sec") {
      primaryKeySeen = true;
    } else if (primaryKeySeen && fields[0] === "fpr" && UPPERCASE_FINGERPRINT_PATTERN.test(fields[9] ?? "")) {
      return fields[9];
    }
  }
  throw new Error("The ephemeral integration key fingerprint is unavailable.");
}

/** Extracts the first detached signature digest identifier from packet diagnostics. */
function signatureDigestAlgorithm(packetListing) {
  const match = packetListing.match(/^\s*digest algo (\d+),/m);
  if (!match) throw new Error("The integration signature digest is unavailable.");
  return match[1];
}

/** Temporarily installs the environment consumed by debsigs and the protected wrapper. */
async function withEnvironment(values, operation) {
  const previous = new Map(Object.keys(values).map((key) => [key, process.env[key]]));
  for (const [key, value] of Object.entries(values)) process.env[key] = value;
  try {
    return await operation();
  } finally {
    for (const [key, value] of previous) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
  }
}

/** Builds and verifies one tiny DEB through the real Linux distribution toolchain. */
async function runIntegration() {
  if (process.platform !== "linux") {
    throw new Error("The Linux distribution integration requires Linux.");
  }

  const integrationRoot = await mkdtemp(join(tmpdir(), "bottie-linux-distribution-integration-"));
  const signingHome = join(integrationRoot, "signing-gnupg");
  const verificationHome = join(integrationRoot, "verification-gnupg");
  try {
    const packageRoot = join(integrationRoot, "package-root");
    const controlDirectory = join(packageRoot, "DEBIAN");
    const payloadDirectory = join(packageRoot, "usr", "share", "bottie-integration");
    const debPath = join(integrationRoot, "bottie-integration_0.0.0_all.deb");
    const passphrasePath = join(integrationRoot, "passphrase");
    const wrapperDirectory = join(integrationRoot, "bin");
    const wrapperPath = join(wrapperDirectory, "gpg");
    const canonicalPayloadPath = join(integrationRoot, "canonical-deb-payload");
    const debsigsPayloadPath = join(integrationRoot, "debsigs-payload");
    const embeddedSignaturePath = join(integrationRoot, "embedded-origin-signature");
    const policiesDirectory = join(integrationRoot, "policies");
    const keyringsDirectory = join(integrationRoot, "keyrings");

    await mkdir(controlDirectory, { recursive: true });
    await mkdir(payloadDirectory, { recursive: true });
    await mkdir(signingHome, { mode: 0o700, recursive: true });
    await mkdir(verificationHome, { mode: 0o700, recursive: true });
    await mkdir(wrapperDirectory, { mode: 0o700, recursive: true });
    await writeFile(
      join(controlDirectory, "control"),
      [
        "Package: bottie-integration",
        "Version: 0.0.0",
        "Section: utils",
        "Priority: optional",
        "Architecture: all",
        "Maintainer: Bottie integration test",
        "Description: Credential-free Linux distribution signing fixture",
        "",
      ].join("\n"),
      { mode: 0o600 },
    );
    await writeFile(join(payloadDirectory, "payload.txt"), "bottie-linux-distribution-integration-v1\n", {
      mode: 0o600,
    });
    await writeFile(passphrasePath, TEST_KEY_PASSPHRASE, { mode: 0o600 });
    await copyFile(join(REPOSITORY_ROOT, "scripts", "linux-debsigs-gpg-wrapper.sh"), wrapperPath);
    await chmod(wrapperPath, 0o700);

    runTextCommand("/usr/bin/gpg", [
      "--no-options",
      "--batch",
      "--homedir",
      signingHome,
      "--pinentry-mode",
      "loopback",
      "--passphrase-file",
      passphrasePath,
      "--quick-generate-key",
      TEST_KEY_USER_ID,
      "rsa2048",
      "sign",
      "1d",
    ]);
    const fingerprint = primaryFingerprint(
      runTextCommand("/usr/bin/gpg", [
        "--no-options",
        "--batch",
        "--homedir",
        signingHome,
        "--with-colons",
        "--list-secret-keys",
      ]),
    );
    const keyId = fingerprint.slice(-16);
    const policyRoot = join(policiesDirectory, fingerprint);
    const keyringRoot = join(keyringsDirectory, fingerprint);
    const publicKeyringPath = join(keyringRoot, "bottie.gpg");
    await mkdir(policyRoot, { mode: 0o700, recursive: true });
    await mkdir(keyringRoot, { mode: 0o700, recursive: true });
    runTextCommand("/usr/bin/gpg", [
      "--no-options",
      "--batch",
      "--homedir",
      signingHome,
      "--yes",
      "--output",
      publicKeyringPath,
      "--export",
      fingerprint,
    ]);

    const productionPolicy = await readFile(join(REPOSITORY_ROOT, "distribution", "linux", "bottie.pol"), "utf8");
    const productionFingerprints = [...new Set(productionPolicy.match(/[A-F0-9]{40}/g) ?? [])];
    assert.equal(productionFingerprints.length, 1, "The checked-in Linux policy must have one fingerprint.");
    await writeFile(
      join(policyRoot, "bottie.pol"),
      productionPolicy.replaceAll(productionFingerprints[0], fingerprint),
      { mode: 0o600 },
    );
    runTextCommand("/usr/bin/dpkg-deb", ["--root-owner-group", "--build", packageRoot, debPath]);

    const configuration = {
      embeddedSignaturePath,
      keyId,
      keyringsDirectory,
      payloadPath: canonicalPayloadPath,
      policiesDirectory,
      publicKeyringPath,
      verificationGnupgHome: verificationHome,
    };
    await withEnvironment(
      {
        BOTTIE_LINUX_DEBSIGS_PAYLOAD_PATH: debsigsPayloadPath,
        BOTTIE_LINUX_EMBEDDED_SIGNATURE_PATH: embeddedSignaturePath,
        BOTTIE_LINUX_SIGNING_KEY_ID: keyId,
        BOTTIE_LINUX_SIGNING_PASSPHRASE_PATH: passphrasePath,
        BOTTIE_LINUX_SIGNING_PAYLOAD_PATH: canonicalPayloadPath,
        BOTTIE_LINUX_SIGNING_PUBLIC_KEYRING_PATH: publicKeyringPath,
        GNUPGHOME: signingHome,
        PATH: `${wrapperDirectory}:${process.env.PATH ?? ""}`,
      },
      async () => signAndVerifyLinuxDistribution(configuration, debPath),
    );

    const signatureMembers = runTextCommand("/usr/bin/ar", ["t", debPath])
      .split(/\r?\n/)
      .filter((member) => member.startsWith("_gpg"));
    assert.deepEqual(signatureMembers, [EXPECTED_SIGNATURE_MEMBER], "The integration DEB needs one origin signature.");
    const packetListing = runTextCommand(
      "/usr/bin/gpg",
      ["--no-options", "--batch", "--list-packets", embeddedSignaturePath],
      { environment: { GNUPGHOME: verificationHome } },
    );
    assert.equal(
      signatureDigestAlgorithm(packetListing),
      EXPECTED_DIGEST_ALGORITHM_ID,
      "The integration origin signature must use SHA-256.",
    );

    const weakSignaturePath = join(integrationRoot, "weak-sha1-signature");
    const weakSigningStatus = commandStatus("/usr/bin/gpg", [
      "--no-options",
      "--batch",
      "--homedir",
      signingHome,
      "--pinentry-mode",
      "loopback",
      "--passphrase-file",
      passphrasePath,
      "--local-user",
      keyId,
      "--digest-algo",
      "SHA1",
      "--output",
      weakSignaturePath,
      "--detach-sign",
      canonicalPayloadPath,
    ]);
    assert.equal(weakSigningStatus, 0, "The integration must create a SHA-1 regression fixture.");
    const weakPacketListing = runTextCommand(
      "/usr/bin/gpg",
      ["--no-options", "--batch", "--list-packets", weakSignaturePath],
      { environment: { GNUPGHOME: verificationHome } },
    );
    assert.equal(
      signatureDigestAlgorithm(weakPacketListing),
      EXPECTED_WEAK_DIGEST_ALGORITHM_ID,
      "The negative integration fixture must use SHA-1.",
    );
    const weakVerificationStatus = commandStatus(
      "/usr/bin/gpg",
      openPgpVerificationArguments(publicKeyringPath, weakSignaturePath, canonicalPayloadPath),
      { GNUPGHOME: verificationHome },
    );
    assert.ok(
      Number.isInteger(weakVerificationStatus) && weakVerificationStatus !== 0,
      "The hardened verifier must reject a SHA-1 signature.",
    );
  } finally {
    commandStatus("/usr/bin/gpgconf", ["--homedir", signingHome, "--kill", "gpg-agent"]);
    commandStatus("/usr/bin/gpgconf", ["--homedir", verificationHome, "--kill", "gpg-agent"]);
    await rm(integrationRoot, { force: true, recursive: true });
  }
}

try {
  await runIntegration();
  console.log("[bottie] credential-free Linux distribution integration passed.");
} catch {
  console.error("[bottie] credential-free Linux distribution integration failed.");
  process.exitCode = 1;
}
