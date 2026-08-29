import { readFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import {
  LINUX_DISTRIBUTION_INTEGRATION_STAGES,
  linuxDistributionFixtureBuildArguments,
  linuxDistributionIntegrationFailureMessage,
} from "./linux-distribution-integration-contract.mjs";
import {
  canonicalDebianPayloadMembers,
  canonicalDebianPayloadExtractionArguments,
  openPgpVerificationArguments,
  resolveSigningConfiguration,
  signAndVerifyLinuxDistribution,
  signingArguments,
  verificationFailureMessage,
  verificationArguments,
  verifiedLinuxDistributionEvidence,
} from "./linux-distribution.mjs";

const KEY_ID = "0123456789ABCDEF";
const PUBLISHED_FINGERPRINT = "5C1D104ACE472474CE21070B065CFE6D5D9FD8A4";
const DEB_PATH = "/tmp/bottie_0.9.0_amd64.deb";
const POLICIES_DIRECTORY = "/runner-temp/policies";
const KEYRINGS_DIRECTORY = "/runner-temp/keyrings";
const EMBEDDED_SIGNATURE_PATH = "/runner-temp/embedded-origin-signature";
const PAYLOAD_PATH = "/runner-temp/canonical-deb-payload";
const PUBLIC_KEYRING_PATH = "/runner-temp/keyrings/fingerprint/bottie.gpg";
const VERIFICATION_GNUPG_HOME = "/runner-temp/verification-gnupg";
const SIGNING_WRAPPER_PATH = fileURLToPath(new URL("./linux-debsigs-gpg-wrapper.sh", import.meta.url));

describe("Linux distribution integration contract", () => {
  it("forces the fixture into the archive format supported by legacy debsigs", () => {
    expect(linuxDistributionFixtureBuildArguments("/tmp/package-root", "/tmp/bottie.deb")).toEqual([
      "--root-owner-group",
      "-Zxz",
      "--build",
      "/tmp/package-root",
      "/tmp/bottie.deb",
    ]);
  });

  it("reports only allowlisted integration stages", () => {
    expect(linuxDistributionIntegrationFailureMessage(LINUX_DISTRIBUTION_INTEGRATION_STAGES.fixturePackage)).toBe(
      "[bottie] credential-free Linux distribution integration failed at fixture-package.",
    );
    expect(linuxDistributionIntegrationFailureMessage("/runner/private/signing-key.asc")).toBe(
      "[bottie] credential-free Linux distribution integration failed at unknown.",
    );
    expect(linuxDistributionIntegrationFailureMessage(LINUX_DISTRIBUTION_INTEGRATION_STAGES.cleanup)).toBe(
      "[bottie] credential-free Linux distribution integration failed at cleanup.",
    );
  });
});

/** Returns minimal unsigned package evidence from the isolated Linux package workflow. */
function unsignedEvidence() {
  return {
    schemaVersion: 1,
    version: "0.9.0",
    bundle: {
      installer: {
        metadata: { architecture: "amd64", package: "bottie", version: "0.9.0" },
        sha256: "a".repeat(64),
        signature: { classification: "unsigned", verifies: false },
        size: 10,
      },
      payload: { bundleDigest: "b".repeat(64) },
    },
    smoke: { terminated: true },
  };
}

describe("Linux distribution signing", () => {
  it("keeps updater signing in the protected manual Linux environment", async () => {
    const workflow = await readFile(
      new URL("../.github/workflows/linux-distribution-validation.yml", import.meta.url),
      "utf8",
    );

    expect(workflow).toContain("environment: linux-distribution");
    expect(workflow).toContain("BOTTIE_UPDATER_SIGNING_PRIVATE_KEY");
    expect(workflow).toContain("BOTTIE_UPDATER_SIGNING_PRIVATE_KEY_PASSWORD");
    const signingStep = workflow.indexOf("- name: Sign and independently verify Linux distribution");
    expect(workflow.slice(0, signingStep)).not.toContain("BOTTIE_UPDATER_SIGNING_PRIVATE_KEY");
    expect(workflow).not.toMatch(/push:|pull_request:|release:/);
  });

  it("binds verified updater evidence to the exact embedded-OpenPGP-signed DEB", async () => {
    const script = await readFile(new URL("./linux-distribution.mjs", import.meta.url), "utf8");

    expect(script).toContain('bindUpdaterArtifactEvidence(updater, "linux-x86_64", signedPackage.sha256)');
    expect(script.indexOf("signAndVerifyLinuxDistribution(configuration, debPath)")).toBeLessThan(
      script.indexOf('bindUpdaterArtifactEvidence(updater, "linux-x86_64", signedPackage.sha256)'),
    );
  });

  it("selects exactly one canonical Debian payload in verifier order", () => {
    expect(canonicalDebianPayloadMembers(["data.tar.xz", "debian-binary", "control.tar.gz"])).toEqual([
      "debian-binary",
      "control.tar.gz",
      "data.tar.xz",
    ]);
    expect(() =>
      canonicalDebianPayloadMembers(["debian-binary", "control.tar.gz", "control.tar.xz", "data.tar.xz"]),
    ).toThrow(/exactly one supported control archive/);
    expect(() => canonicalDebianPayloadMembers(["debian-binary", "control.tar.zst", "data.tar.zst"])).toThrow(
      /supported control archive/,
    );
    expect(() =>
      canonicalDebianPayloadMembers(["debian-binary", "control.tar.gz", "data.tar.xz", "_gpgorigin"]),
    ).toThrow(/unsigned Debian archive/);
  });

  it("extracts canonical members separately instead of trusting archive order", () => {
    expect(
      canonicalDebianPayloadExtractionArguments(DEB_PATH, ["data.tar.xz", "control.tar.gz", "debian-binary"]),
    ).toEqual([
      ["p", DEB_PATH, "debian-binary"],
      ["p", DEB_PATH, "control.tar.gz"],
      ["p", DEB_PATH, "data.tar.xz"],
    ]);
  });

  it("adds one origin signature with an explicit protected key identity", () => {
    expect(signingArguments(KEY_ID, DEB_PATH)).toEqual(["--sign=origin", `--default-key=${KEY_ID}`, DEB_PATH]);
  });

  it.skipIf(process.platform === "win32")(
    "rejects unexpected legacy signer arguments before reading protected paths",
    () => {
      const unexpectedArguments = [
        ["--openpgp", "--detach-sign", `--default-key=${KEY_ID}`],
        ["--gnupg", "--detach-sign", "--default-key", KEY_ID],
        ["--detach-sign", "--openpgp", "--default-key", KEY_ID],
        ["--openpgp", "--detach-sign", "--default-key", "FEDCBA9876543210"],
        ["--openpgp", "--detach-sign", "--default-key", KEY_ID, "--output", "/tmp/injected"],
        ["--openpgp", "--detach-sign", "--default-key", KEY_ID, "--digest-algo", "SHA1"],
      ];

      for (const arguments_ of unexpectedArguments) {
        const result = spawnSync("bash", [SIGNING_WRAPPER_PATH, ...arguments_], {
          encoding: "utf8",
          env: { ...process.env, BOTTIE_LINUX_SIGNING_KEY_ID: KEY_ID },
          input: "",
        });
        expect(result.status).toBe(1);
        expect(result.stdout).toBe("");
        expect(result.stderr).toBe("Unexpected legacy debsigs GnuPG arguments.\n");
      }

      const normalizedResult = spawnSync(
        "bash",
        [SIGNING_WRAPPER_PATH, "--openpgp", "--detach-sign", "--default-key", KEY_ID],
        {
          encoding: "utf8",
          env: {
            ...process.env,
            BOTTIE_LINUX_DEBSIGS_PAYLOAD_PATH: "/dev/null",
            BOTTIE_LINUX_EMBEDDED_SIGNATURE_PATH: "/dev/null",
            BOTTIE_LINUX_SIGNING_KEY_ID: KEY_ID.toLowerCase(),
            BOTTIE_LINUX_SIGNING_PASSPHRASE_PATH: "/dev/null",
            BOTTIE_LINUX_SIGNING_PAYLOAD_PATH: "/dev/null",
            BOTTIE_LINUX_SIGNING_PUBLIC_KEYRING_PATH: "/dev/null",
          },
          input: "",
        },
      );
      expect(normalizedResult.status).toBe(1);
      expect(normalizedResult.stdout).toBe("");
      expect(normalizedResult.stderr).toBe("Linux origin signature creation failed.\n");
    },
  );

  it("verifies through caller-owned policy and keyring roots", () => {
    expect(verificationArguments(POLICIES_DIRECTORY, KEYRINGS_DIRECTORY, DEB_PATH)).toEqual([
      "--policies-dir",
      POLICIES_DIRECTORY,
      "--keyrings-dir",
      KEYRINGS_DIRECTORY,
      DEB_PATH,
    ]);
    expect(verificationFailureMessage(10)).toBe("Linux distribution origin signature is unavailable.");
    expect(verificationFailureMessage(11)).toBe("Linux distribution verification policy root is unavailable.");
    expect(verificationFailureMessage(12)).toBe("Linux distribution verification policy did not select the signature.");
    expect(verificationFailureMessage(13)).toBe("Linux distribution signature verification failed.");
    expect(verificationFailureMessage(14)).toBe("Linux distribution verification backend failed.");
    expect(verificationFailureMessage(1)).toBe("Linux distribution verification failed.");
    expect(openPgpVerificationArguments(PUBLIC_KEYRING_PATH, EMBEDDED_SIGNATURE_PATH, PAYLOAD_PATH)).toEqual([
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
      PUBLIC_KEYRING_PATH,
      "--verify",
      EMBEDDED_SIGNATURE_PATH,
      PAYLOAD_PATH,
    ]);
  });

  it("distinguishes signing, archive inspection, and independent verification", () => {
    const operations = [];
    const configuration = {
      embeddedSignaturePath: EMBEDDED_SIGNATURE_PATH,
      keyId: KEY_ID,
      keyringsDirectory: KEYRINGS_DIRECTORY,
      payloadPath: PAYLOAD_PATH,
      policiesDirectory: POLICIES_DIRECTORY,
      publicKeyringPath: PUBLIC_KEYRING_PATH,
      verificationGnupgHome: VERIFICATION_GNUPG_HOME,
    };

    signAndVerifyLinuxDistribution(
      configuration,
      DEB_PATH,
      (command, arguments_, failureMessage, environment) =>
        operations.push({
          command,
          arguments_,
          environment,
          failureMessage: typeof failureMessage === "function" ? failureMessage(13) : failureMessage,
        }),
      (debPath, signaturePath) =>
        operations.push({ command: "inspect-origin-signature", arguments_: [debPath, signaturePath] }),
      (debPath, payloadPath) =>
        operations.push({ command: "write-canonical-payload", arguments_: [debPath, payloadPath] }),
    );

    expect(operations).toEqual([
      { command: "write-canonical-payload", arguments_: [DEB_PATH, PAYLOAD_PATH] },
      {
        command: "debsigs",
        arguments_: ["--sign=origin", `--default-key=${KEY_ID}`, DEB_PATH],
        environment: undefined,
        failureMessage: "Linux distribution signing failed.",
      },
      {
        command: "inspect-origin-signature",
        arguments_: [DEB_PATH, EMBEDDED_SIGNATURE_PATH],
      },
      {
        command: "/usr/bin/gpg",
        arguments_: openPgpVerificationArguments(PUBLIC_KEYRING_PATH, EMBEDDED_SIGNATURE_PATH, PAYLOAD_PATH),
        environment: { GNUPGHOME: VERIFICATION_GNUPG_HOME },
        failureMessage: "Embedded Linux origin signature verification failed.",
      },
      {
        command: "debsig-verify",
        arguments_: ["--policies-dir", POLICIES_DIRECTORY, "--keyrings-dir", KEYRINGS_DIRECTORY, DEB_PATH],
        environment: { DEBSIG_GNUPG_PROGRAM: "/usr/bin/gpg", GNUPGHOME: VERIFICATION_GNUPG_HOME },
        failureMessage: "Linux distribution signature verification failed.",
      },
    ]);
  });

  it("requires complete protected configuration outside the repository", () => {
    expect(
      resolveSigningConfiguration(
        {
          BOTTIE_LINUX_SIGNING_KEY_ID: KEY_ID.toLowerCase(),
          BOTTIE_LINUX_EMBEDDED_SIGNATURE_PATH: EMBEDDED_SIGNATURE_PATH,
          BOTTIE_LINUX_SIGNING_POLICIES_DIR: POLICIES_DIRECTORY,
          BOTTIE_LINUX_SIGNING_KEYRINGS_DIR: KEYRINGS_DIRECTORY,
          BOTTIE_LINUX_SIGNING_PAYLOAD_PATH: PAYLOAD_PATH,
          BOTTIE_LINUX_SIGNING_PUBLIC_KEYRING_PATH: PUBLIC_KEYRING_PATH,
          BOTTIE_LINUX_VERIFICATION_GNUPG_HOME: VERIFICATION_GNUPG_HOME,
        },
        "/repo",
      ),
    ).toEqual({
      embeddedSignaturePath: EMBEDDED_SIGNATURE_PATH,
      keyId: KEY_ID,
      keyringsDirectory: KEYRINGS_DIRECTORY,
      payloadPath: PAYLOAD_PATH,
      policiesDirectory: POLICIES_DIRECTORY,
      publicKeyringPath: PUBLIC_KEYRING_PATH,
      verificationGnupgHome: VERIFICATION_GNUPG_HOME,
    });
    expect(() => resolveSigningConfiguration({}, "/repo")).toThrow(/configuration is unavailable/);
    expect(() =>
      resolveSigningConfiguration(
        {
          BOTTIE_LINUX_SIGNING_KEY_ID: KEY_ID,
          BOTTIE_LINUX_EMBEDDED_SIGNATURE_PATH: EMBEDDED_SIGNATURE_PATH,
          BOTTIE_LINUX_SIGNING_POLICIES_DIR: "/repo/private/policies",
          BOTTIE_LINUX_SIGNING_KEYRINGS_DIR: KEYRINGS_DIRECTORY,
          BOTTIE_LINUX_SIGNING_PAYLOAD_PATH: PAYLOAD_PATH,
          BOTTIE_LINUX_SIGNING_PUBLIC_KEYRING_PATH: PUBLIC_KEYRING_PATH,
          BOTTIE_LINUX_VERIFICATION_GNUPG_HOME: VERIFICATION_GNUPG_HOME,
        },
        "/repo",
      ),
    ).toThrow(/outside the repository/);
    expect(() =>
      resolveSigningConfiguration(
        {
          BOTTIE_LINUX_SIGNING_KEY_ID: KEY_ID,
          BOTTIE_LINUX_EMBEDDED_SIGNATURE_PATH: EMBEDDED_SIGNATURE_PATH,
          BOTTIE_LINUX_SIGNING_POLICIES_DIR: POLICIES_DIRECTORY,
          BOTTIE_LINUX_SIGNING_KEYRINGS_DIR: KEYRINGS_DIRECTORY,
          BOTTIE_LINUX_SIGNING_PAYLOAD_PATH: PAYLOAD_PATH,
          BOTTIE_LINUX_SIGNING_PUBLIC_KEYRING_PATH: PUBLIC_KEYRING_PATH,
          BOTTIE_LINUX_VERIFICATION_GNUPG_HOME: "/repo/private/verification-gnupg",
        },
        "/repo",
      ),
    ).toThrow(/outside the repository/);
  });

  it("records only independently verified signature state and signed-package bytes", () => {
    const evidence = verifiedLinuxDistributionEvidence(unsignedEvidence(), {
      sha256: "c".repeat(64),
      size: 12,
    });

    expect(evidence.bundle.installer).toMatchObject({
      sha256: "c".repeat(64),
      signature: { classification: "identified", verifies: true },
      size: 12,
    });
    expect(JSON.stringify(evidence)).not.toContain(KEY_ID);
    expect(() =>
      verifiedLinuxDistributionEvidence(
        {
          ...unsignedEvidence(),
          bundle: {
            ...unsignedEvidence().bundle,
            installer: {
              ...unsignedEvidence().bundle.installer,
              signature: { classification: "identified", verifies: true },
            },
          },
        },
        { sha256: "c".repeat(64), size: 12 },
      ),
    ).toThrow(/unsigned Bottie package evidence/);
  });

  it("keeps protected CI manual, environment-gated, evidence-only, and self-cleaning", async () => {
    const workflow = await readFile(
      new URL("../.github/workflows/linux-distribution-validation.yml", import.meta.url),
      "utf8",
    );
    const smokeWorkflow = await readFile(
      new URL("../.github/workflows/linux-package-smoke.yml", import.meta.url),
      "utf8",
    );
    const packageMetadata = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
    const signingWrapper = await readFile(new URL("./linux-debsigs-gpg-wrapper.sh", import.meta.url), "utf8");

    expect(workflow).toContain("workflow_dispatch:");
    expect(workflow).toContain("environment: linux-distribution");
    expect(workflow).toContain("BOTTIE_LINUX_SIGNING_PRIVATE_KEY_BASE64");
    expect(workflow).toContain("BOTTIE_LINUX_SIGNING_KEY_PASSPHRASE");
    expect(workflow).toContain("distribution/linux/bottie-linux-signing-public.asc");
    expect(workflow).toContain("distribution/linux/bottie.pol");
    expect(workflow).toContain("package/linux-package-evidence.json");
    expect(packageMetadata.scripts["package:linux:distribution:test:integration"]).toBe(
      "node scripts/linux-distribution.integration.mjs",
    );
    expect(workflow).toContain("npm run package:linux:distribution:test:integration");
    expect(smokeWorkflow).toContain("scripts/linux-distribution.integration.mjs");
    expect(smokeWorkflow).toContain("scripts/linux-distribution-integration-contract.mjs");
    expect(smokeWorkflow).toContain("scripts/linux-debsigs-gpg-wrapper.sh");
    expect(smokeWorkflow).toContain("distribution/linux/**");
    expect(smokeWorkflow).toContain("npm run package:linux:distribution:test:integration");
    expect(workflow).toContain('mkdir -p "$policies_directory/$fingerprint" "$keyrings_directory/$fingerprint"');
    expect(workflow).toContain('echo "::add-mask::$fingerprint"');
    expect(workflow).toContain('echo "::add-mask::$key_id"');
    expect(workflow).toContain('printf \'%s\\n\' "bottie-linux-signing-probe-v1" > "$signing_probe_path"');
    expect(workflow).toContain('--output "$signature_probe_path" --detach-sign "$signing_probe_path"');
    expect(workflow).toContain("Protected Linux signing key cannot create a signature.");
    expect(workflow).toContain("Protected Linux signing key does not match the published public key.");
    expect(workflow).toContain('public_keyring_path="$keyrings_directory/$fingerprint/bottie.gpg"');
    expect(workflow).toContain("Published Linux public keyring cannot verify protected signatures.");
    expect(workflow).toContain("BOTTIE_LINUX_VERIFICATION_GNUPG_HOME=$verification_gnupg_home");
    expect(workflow).toContain('install -m 700 scripts/linux-debsigs-gpg-wrapper.sh "$wrapper_directory/gpg"');
    expect(signingWrapper).toContain('cat > "$debsigs_payload_path"');
    expect(signingWrapper).toContain("EXPECTED_DEBSIGS_ARGUMENT_COUNT=4");
    expect(signingWrapper).toContain('"$1" != "--openpgp"');
    expect(signingWrapper).toContain('"$2" != "--detach-sign"');
    expect(signingWrapper).toContain('"$3" != "--default-key"');
    expect(signingWrapper).toContain('"$4" != "$expected_key_id"');
    expect(signingWrapper).toContain("Unexpected legacy debsigs GnuPG arguments.");
    expect(signingWrapper).toContain("--no-options --batch --armor --pinentry-mode loopback");
    expect(signingWrapper).toContain('--local-user "$expected_key_id"');
    expect(signingWrapper).toContain("SHA256_DIGEST_ALGORITHM=SHA256");
    expect(signingWrapper).toContain('--digest-algo "$SHA256_DIGEST_ALGORITHM"');
    expect(signingWrapper).toContain("SHA256_DIGEST_ALGORITHM_ID=8");
    expect(signingWrapper).toContain("Linux origin signature creation failed.");
    expect(signingWrapper).toContain("Linux origin signature did not use SHA-256.");
    expect(signingWrapper).toContain("exit 1");
    expect(signingWrapper).not.toContain('"$@"');
    expect(signingWrapper).not.toContain("gpg --openpgp");
    expect(signingWrapper).toContain(
      '/usr/bin/gpgv --keyring "$public_keyring_path" "$embedded_signature_path" "$canonical_payload_path"',
    );
    expect(workflow).not.toContain('--export "$fingerprint"');
    expect(workflow).not.toContain('mkdir -p "$policies_directory/$key_id"');
    expect(workflow).toContain("if: always()");
    expect(workflow).not.toMatch(/pull_request:|push:|release:/);
    expect(workflow).not.toMatch(/package\/linux\/.*\.deb/);
    for (const linuxWorkflow of [workflow, smokeWorkflow]) {
      expect(linuxWorkflow).toContain("actions/checkout@v6");
      expect(linuxWorkflow).toContain("actions/setup-node@v6");
      expect(linuxWorkflow).toContain("actions/upload-artifact@v7");
      expect(linuxWorkflow).not.toMatch(/actions\/(?:checkout|setup-node|upload-artifact)@v4/);
    }
  });

  it("publishes a public-only verification root and matching install policy", async () => {
    const publicKey = await readFile(
      new URL("../distribution/linux/bottie-linux-signing-public.asc", import.meta.url),
      "utf8",
    );
    const policy = await readFile(new URL("../distribution/linux/bottie.pol", import.meta.url), "utf8");
    const verificationGuide = await readFile(new URL("../distribution/linux/README.md", import.meta.url), "utf8");

    expect(publicKey).toContain("-----BEGIN PGP PUBLIC KEY BLOCK-----");
    expect(publicKey).not.toContain("PRIVATE KEY");
    expect(policy.match(new RegExp(PUBLISHED_FINGERPRINT, "g"))).toHaveLength(3);
    expect(policy).toContain('File="bottie.gpg"');
    expect(policy).not.toContain('File="bottie.pgp"');
    expect(verificationGuide.replaceAll(" ", "")).toContain(PUBLISHED_FINGERPRINT);
    expect(verificationGuide).toContain("24 August 2028");
    expect(verificationGuide).toContain("debsig-verify ./bottie_0.9.0_amd64.deb");
  });
});
