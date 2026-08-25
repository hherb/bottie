import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import {
  resolveSigningConfiguration,
  signingArguments,
  verificationArguments,
  verifiedLinuxDistributionEvidence,
} from "./linux-distribution.mjs";

const KEY_ID = "0123456789ABCDEF";
const DEB_PATH = "/tmp/bottie_0.9.0_amd64.deb";
const POLICIES_DIRECTORY = "/runner-temp/policies";
const KEYRINGS_DIRECTORY = "/runner-temp/keyrings";

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
  it("adds one origin signature with an explicit protected key identity", () => {
    expect(signingArguments(KEY_ID, DEB_PATH)).toEqual(["--sign=origin", `--default-key=${KEY_ID}`, DEB_PATH]);
  });

  it("verifies through caller-owned policy and keyring roots", () => {
    expect(verificationArguments(POLICIES_DIRECTORY, KEYRINGS_DIRECTORY, DEB_PATH)).toEqual([
      "--policies-dir",
      POLICIES_DIRECTORY,
      "--keyrings-dir",
      KEYRINGS_DIRECTORY,
      DEB_PATH,
    ]);
  });

  it("requires complete protected configuration outside the repository", () => {
    expect(
      resolveSigningConfiguration(
        {
          BOTTIE_LINUX_SIGNING_KEY_ID: KEY_ID.toLowerCase(),
          BOTTIE_LINUX_SIGNING_POLICIES_DIR: POLICIES_DIRECTORY,
          BOTTIE_LINUX_SIGNING_KEYRINGS_DIR: KEYRINGS_DIRECTORY,
        },
        "/repo",
      ),
    ).toEqual({
      keyId: KEY_ID,
      keyringsDirectory: KEYRINGS_DIRECTORY,
      policiesDirectory: POLICIES_DIRECTORY,
    });
    expect(() => resolveSigningConfiguration({}, "/repo")).toThrow(/configuration is unavailable/);
    expect(() =>
      resolveSigningConfiguration(
        {
          BOTTIE_LINUX_SIGNING_KEY_ID: KEY_ID,
          BOTTIE_LINUX_SIGNING_POLICIES_DIR: "/repo/private/policies",
          BOTTIE_LINUX_SIGNING_KEYRINGS_DIR: KEYRINGS_DIRECTORY,
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

    expect(workflow).toContain("workflow_dispatch:");
    expect(workflow).toContain("environment: linux-distribution");
    expect(workflow).toContain("BOTTIE_LINUX_SIGNING_PRIVATE_KEY_BASE64");
    expect(workflow).toContain("BOTTIE_LINUX_SIGNING_KEY_PASSPHRASE");
    expect(workflow).toContain("package/linux-package-evidence.json");
    expect(workflow).toContain('mkdir -p "$policies_directory/$fingerprint" "$keyrings_directory/$fingerprint"');
    expect(workflow).toContain('echo "::add-mask::$fingerprint"');
    expect(workflow).toContain('echo "::add-mask::$key_id"');
    expect(workflow).not.toContain('mkdir -p "$policies_directory/$key_id"');
    expect(workflow).toContain("if: always()");
    expect(workflow).not.toMatch(/pull_request:|push:|release:/);
    expect(workflow).not.toMatch(/package\/linux\/.*\.deb/);
  });
});
