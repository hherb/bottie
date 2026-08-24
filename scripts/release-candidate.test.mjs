import { createHash } from "node:crypto";

import { describe, expect, it } from "vitest";

import { buildReleaseCandidateManifest, parseReleaseNotes } from "./release-candidate.mjs";

const VERSION = "0.9.0";
const SHA = "a".repeat(64);
const NOTES = `# Bottie 0.9.0 beta

Version: 0.9.0
Channel: beta

Tester-facing notes.
`;

/** Returns one fully accepted input set for focused gate mutations. */
function acceptedInputs() {
  const currentInputHashes = {
    "assets/icon.svg": SHA,
    "package.json": SHA,
    "static/favicon.png": SHA,
  };
  return {
    version: VERSION,
    releaseNotes: NOTES,
    applicationVersions: { cargo: VERSION, npm: VERSION, tauri: VERSION },
    dependencyInventory: {
      schemaVersion: 1,
      inputs: currentInputHashes,
      summary: { compatible: 1, "notice-required": 1, unknown: 0, "review-required": 0 },
      assets: [
        {
          name: "Bottie application icons and browser favicon",
          classification: "compatible",
          generationSources: { "assets/icon.svg": SHA },
          files: { "static/favicon.png": SHA },
        },
      ],
    },
    currentInputHashes: { ...currentInputHashes },
    requiredDocuments: { licence: SHA, notices: SHA },
    runtimeAssets: {
      schemaVersion: 1,
      manifestSha256: SHA,
      onnxRuntime: { licenceSha256: SHA, thirdPartyNoticesSha256: SHA },
      embeddingGemma: { revision: "b".repeat(40), terms: { sha256: SHA } },
    },
    runtimeAssetSources: { modelNotice: SHA, onnxRuntimeLicence: SHA, onnxRuntimeNotices: SHA },
    modelTermsAcceptance: {
      schemaVersion: 1,
      accepted: true,
      modelRevision: "b".repeat(40),
      termsSha256: SHA,
    },
    macosDistribution: {
      schemaVersion: 1,
      artifact: {
        bundleDigest: SHA,
        requiredDocuments: { licence: SHA, modelNotice: SHA, thirdPartyNotices: SHA },
        requiredEntries: {
          executable: true,
          icon: true,
          infoPlist: true,
          licence: true,
          modelNotice: true,
          thirdPartyNotices: true,
        },
      },
      metadata: { architectures: ["arm64"], identifier: "com.bottie.app", version: VERSION },
      notarization: {
        gatekeeper: { accepted: true, source: "notarized-developer-id" },
        submission: { accepted: true, status: "accepted" },
        ticketStapled: true,
        ticketValid: true,
      },
      signing: {
        classification: "developer-id-application",
        hardenedRuntime: true,
        secureTimestamp: true,
        verifies: true,
      },
    },
    windowsPackage: {
      schemaVersion: 1,
      version: VERSION,
      bundle: {
        installer: { sha256: SHA, signature: { classification: "identified", verifies: true }, size: 10 },
        payload: {
          applicationDirectory: "PFiles/bottie",
          architecture: "x86_64",
          bundleDigest: SHA,
          embeddedIcon: { height: 32, width: 32 },
          requiredDocuments: { licence: SHA, modelNotice: SHA, thirdPartyNotices: SHA },
          signature: { classification: "identified", verifies: true },
        },
      },
      smoke: acceptedSmoke(),
    },
    linuxPackage: {
      schemaVersion: 1,
      version: VERSION,
      bundle: {
        installer: {
          metadata: { architecture: "amd64", package: "bottie", version: VERSION },
          sha256: SHA,
          signature: { classification: "identified", verifies: true },
          size: 10,
        },
        payload: {
          architecture: "x86_64",
          bundleDigest: SHA,
          installedIcons: ["icon-32.png", "icon-64.png", "icon-128.png", "icon-512.png"],
          requiredDocuments: { licence: SHA, modelNotice: SHA, thirdPartyNotices: SHA },
        },
      },
      smoke: acceptedSmoke(),
    },
  };
}

/** Returns the exact normalized package-smoke success contract. */
function acceptedSmoke() {
  return {
    database: { conversationCount: 0, migrationCount: 21, profileCount: 1, quickCheck: "ok", schemaVersion: 21 },
    isolatedSupportDirectory: true,
    offlineProviderConnections: 1,
    remainedRunning: true,
    terminated: true,
  };
}

describe("release candidate gate", () => {
  it("parses one strict versioned beta release-notes source", () => {
    expect(parseReleaseNotes(NOTES)).toEqual({ channel: "beta", title: "Bottie 0.9.0 beta", version: VERSION });
    expect(() => parseReleaseNotes(NOTES.replace("Channel: beta", "Channel: stable"))).toThrow(/beta channel/);
    expect(() => parseReleaseNotes(NOTES.replace("Version: 0.9.0", "Version: latest"))).toThrow(/version/);
  });

  it("produces a deterministic accepted manifest from current signed evidence", () => {
    const inputs = acceptedInputs();

    const first = buildReleaseCandidateManifest(inputs);
    const second = buildReleaseCandidateManifest(inputs);

    expect(second).toEqual(first);
    expect(first.ready).toBe(true);
    expect(first.release).toEqual({
      channel: "beta",
      notesSha256: createHash("sha256").update(NOTES).digest("hex"),
      tag: "v0.9.0",
      title: "Bottie 0.9.0 beta",
      version: VERSION,
    });
    expect(first.gates.every((gate) => gate.passed)).toBe(true);
  });

  it("fails closed for missing, stale, unsigned, or unnotarized evidence", () => {
    const inputs = acceptedInputs();
    inputs.currentInputHashes["package.json"] = "b".repeat(64);
    inputs.macosDistribution.notarization.ticketValid = false;
    inputs.windowsPackage.bundle.installer.signature = { classification: "unsigned", verifies: false };
    inputs.linuxPackage = null;
    inputs.requiredDocuments.notices = null;
    inputs.modelTermsAcceptance = null;

    const manifest = buildReleaseCandidateManifest(inputs);
    const gates = Object.fromEntries(manifest.gates.map((gate) => [gate.id, gate.passed]));

    expect(manifest.ready).toBe(false);
    expect(gates).toMatchObject({
      "dependency-inventory-current": false,
      "licence-and-notices": false,
      "model-terms": false,
      "linux-package": false,
      "macos-distribution": false,
      "windows-distribution": false,
    });
  });

  it("does not retain host paths, certificate identities, or unselected evidence", () => {
    const inputs = acceptedInputs();
    inputs.macosDistribution.privatePath = "/Users/private/build/bottie.app";
    inputs.windowsPackage.signer = "Private Person (SECRETTEAM)";
    inputs.windowsPackage.bundle.payload.architecture = "/Users/private/architecture";
    inputs.linuxPackage.bundle.installer.metadata.package = "C:\\private\\package";

    const serialized = JSON.stringify(buildReleaseCandidateManifest(inputs));

    expect(serialized).not.toContain("/Users/private");
    expect(serialized).not.toContain("Private Person");
    expect(serialized).not.toContain("SECRETTEAM");
  });
});
