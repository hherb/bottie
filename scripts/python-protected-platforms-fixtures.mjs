/** Shared closed fixtures for protected Python platform and release evidence tests. */

import { bindProtectedPythonPackage, protectedInspectionSha256 } from "./python-protected-package.mjs";
import { bindProtectedPythonDistributionEnvelope } from "./python-protected-platforms.mjs";
import { bindPythonReleaseCandidate, buildSourceMarker } from "./python-release-candidate.mjs";

export const SOURCE_SHA = "a".repeat(40);
export const OTHER_SHA = "b".repeat(40);
export const SHA = "c".repeat(64);
export const OTHER_OUTER_SHA = "9".repeat(64);
export const PLATFORMS = ["linux", "macos", "windows"];

/** Returns the shared reviewed runtime identity recorded by every package inspection. */
function runtime(fileCount = 539, pythonWasmSha256 = "d".repeat(64)) {
  return {
    schemaVersion: 1,
    fileCount,
    licenceSha256: SHA,
    pythonVersion: "3.14.7",
    pythonWasmSha256,
    runtimeTreeSha256: "e".repeat(64),
    totalBytes: 24_000_000 + (fileCount - 539) * 1_000,
    wasiSdkVersion: "24",
  };
}

/** Returns one complete package inspection for a supported platform. */
function inspection(platform, protectedPackage = false, baseFileCount = 539, pythonWasmSha256 = "d".repeat(64)) {
  const targets = {
    linux: "x86_64-unknown-linux-gnu",
    macos: "aarch64-apple-darwin",
    windows: "x86_64-pc-windows-msvc",
  };
  const transports = {
    linux: [],
    macos: [
      "Contents/Helpers/BottiePythonXPCClient.app/Contents/Info.plist",
      "Contents/Helpers/BottiePythonXPCClient.app/Contents/MacOS/bottie-python-xpc-client",
      "Contents/Helpers/BottiePythonXPCClient.app/Contents/XPCServices/" +
        "com.bottie.python-runner.xpc/Contents/Info.plist",
      "Contents/Helpers/BottiePythonXPCClient.app/Contents/XPCServices/" +
        "com.bottie.python-runner.xpc/Contents/MacOS/bottie-python-xpc-service",
    ],
    windows: ["bottie-python-appcontainer.exe"],
  };
  const packagedRuntime = runtime(baseFileCount, pythonWasmSha256);
  if (platform === "windows") {
    packagedRuntime.fileCount = baseFileCount + 1;
    packagedRuntime.runtimeTreeSha256 = "f".repeat(64);
    packagedRuntime.totalBytes = 34_000_000;
  }
  return {
    bundled: true,
    nativeTransports: transports[platform].map((path, index) => ({
      bytes: index + (protectedPackage ? 2_058 : 10),
      path,
      sha256: `${index + (protectedPackage ? 6 : 1)}`.repeat(64),
    })),
    runnerBytes: protectedPackage ? 14_001_024 : 14_000_000,
    runnerSha256: protectedPackage ? "6".repeat(64) : SHA,
    runtime: packagedRuntime,
    target: targets[platform],
  };
}

/** Returns the complete closed development containment record for one platform. */
function developmentContainment(platform) {
  if (platform === "macos") {
    return {
      appSandboxDeniedHostFixture: true,
      cancellation: true,
      clientExitKilledRunner: true,
      credentialFreeEphemeralSignaturesVerified: true,
      inspectedPackagedBytes: true,
      privatePipeExecution: true,
      status: "ok",
    };
  }
  if (platform === "windows") {
    return {
      appContainerDeniedHostFixture: true,
      appContainerLowIntegrity: true,
      appContainerNoCapabilities: true,
      cancellation: true,
      installedDevelopmentBundle: true,
      jobCloseKilledRunner: true,
      privatePipeExecution: true,
      privilegesStripped: true,
      resourceLimits: true,
      status: "ok",
    };
  }
  return {
    cancellation: true,
    environmentIsolated: true,
    execDenied: true,
    landlockDeniedHostFixture: true,
    networkDenied: true,
    parentCloseKilledRunner: true,
    parentDeathSignal: true,
    processCreationDenied: true,
    resourceLimits: true,
    runtimeReadable: true,
    status: "ok",
    workspaceReadable: true,
  };
}

/** Returns the complete closed shipping containment record for one platform. */
function shippingContainment(platform, packageInspection) {
  const common = {
    inspectionSha256: protectedInspectionSha256(packageInspection),
    platform,
    schemaVersion: 1,
    sourceSha: SOURCE_SHA,
    status: "ok",
    target: packageInspection.target,
  };
  if (platform === "macos") {
    return {
      ...common,
      appSandboxDeniedHostFixture: true,
      cancellation: true,
      clientExitKilledRunner: true,
      inspectedProtectedPackage: true,
      privatePipeExecution: true,
    };
  }
  if (platform === "windows") {
    return {
      ...common,
      appContainerDeniedHostFixture: true,
      appContainerLowIntegrity: true,
      appContainerNoCapabilities: true,
      cancellation: true,
      installedProtectedPackage: true,
      jobCloseKilledRunner: true,
      privatePipeExecution: true,
      privilegesStripped: true,
      resourceLimits: true,
    };
  }
  return {
    ...common,
    cancellation: true,
    environmentIsolated: true,
    execDenied: true,
    installedProtectedPackage: true,
    landlockDeniedHostFixture: true,
    networkDenied: true,
    parentCloseKilledRunner: true,
    parentDeathSignal: true,
    processCreationDenied: true,
    resourceLimits: true,
    runtimeReadable: true,
    workspaceReadable: true,
  };
}

/** Returns one accepted development candidate for all three platforms. */
function releaseCandidate(baseFileCount = 539, pythonWasmSha256 = "d".repeat(64)) {
  return bindPythonReleaseCandidate(
    SOURCE_SHA,
    Object.fromEntries(
      PLATFORMS.map((platform) => {
        const packageInspection = inspection(platform, false, baseFileCount, pythonWasmSha256);
        return [
          platform,
          {
            containment: developmentContainment(platform),
            inspection: packageInspection,
            installedInspection: platform === "macos" ? undefined : structuredClone(packageInspection),
            marker: buildSourceMarker(platform, SOURCE_SHA),
          },
        ];
      }),
    ),
  );
}

/** Returns one complete set of independently accepted protected-platform comparisons. */
export function comparisons(baseFileCount = 539, pythonWasmSha256 = "d".repeat(64)) {
  const candidate = releaseCandidate(baseFileCount, pythonWasmSha256);
  return Object.fromEntries(
    PLATFORMS.map((platform) => {
      const packageInspection = inspection(platform, true, baseFileCount, pythonWasmSha256);
      return [
        platform,
        bindProtectedPythonPackage(
          SOURCE_SHA,
          platform,
          candidate,
          packageInspection,
          shippingContainment(platform, packageInspection),
        ),
      ];
    }),
  );
}

/** Returns one complete final outer-distribution record from a protected platform run. */
export function outerDistributionEvidence(platform, artifactSha256 = SHA) {
  const updater = {
    artifact: { sha256: artifactSha256, size: 10 },
    publicKeySha256: SHA,
    schemaVersion: 1,
    signature: { format: "minisign", sha256: SHA, verifies: true },
    target: { linux: "linux-x86_64", macos: "darwin-aarch64", windows: "windows-x86_64" }[platform],
  };
  const smoke = {
    database: { conversationCount: 0, migrationCount: 21, profileCount: 1, quickCheck: "ok", schemaVersion: 21 },
    isolatedSupportDirectory: true,
    offlineProviderConnections: 1,
    remainedRunning: true,
    terminated: true,
  };
  const requiredDocuments = { licence: SHA, modelNotice: SHA, thirdPartyNotices: SHA };
  if (platform === "macos") {
    return {
      schemaVersion: 1,
      artifact: {
        bundleDigest: artifactSha256,
        requiredDocuments,
        requiredEntries: {
          executable: true,
          icon: true,
          infoPlist: true,
          licence: true,
          modelNotice: true,
          thirdPartyNotices: true,
        },
      },
      metadata: { architectures: ["arm64"], identifier: "com.bottie.app", version: "0.9.0" },
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
      updater,
    };
  }
  const installer = {
    sha256: artifactSha256,
    signature: { classification: "identified", timestamped: true, verifies: true },
    size: 10,
  };
  if (platform === "windows") {
    return {
      schemaVersion: 1,
      version: "0.9.0",
      bundle: {
        installer,
        payload: {
          architecture: "x86_64",
          bundleDigest: artifactSha256,
          requiredDocuments,
          signature: { classification: "identified", timestamped: true, verifies: true },
        },
      },
      smoke,
      updater,
    };
  }
  return {
    schemaVersion: 1,
    version: "0.9.0",
    bundle: {
      installer: { ...installer, metadata: { architecture: "amd64", package: "bottie", version: "0.9.0" } },
      payload: {
        architecture: "x86_64",
        bundleDigest: artifactSha256,
        installedIcons: ["icon-32.png", "icon-64.png", "icon-128.png", "icon-512.png"],
        requiredDocuments,
      },
    },
    smoke: { ...smoke, isolatedSupportDirectory: undefined, isolatedXdgDirectories: true },
    updater,
  };
}

/** Returns the three same-run envelopes consumed by protected platform aggregation. */
export function envelopes() {
  const acceptedComparisons = comparisons();
  return Object.fromEntries(
    PLATFORMS.map((platform) => [
      platform,
      bindProtectedPythonDistributionEnvelope(
        SOURCE_SHA,
        platform,
        acceptedComparisons[platform],
        outerDistributionEvidence(platform),
      ),
    ]),
  );
}

/** Returns one normalized ordinary release-candidate manifest with every existing release gate passed. */
export function ordinaryReleaseCandidate(platformEnvelopes = envelopes()) {
  const gateIds = [
    "release-notes",
    "version-alignment",
    "dependency-inventory-current",
    "dependency-review",
    "licence-and-notices",
    "runtime-assets",
    "model-terms",
    "artwork",
    "macos-distribution",
    "windows-package",
    "windows-distribution",
    "linux-package",
    "linux-distribution",
  ];
  return {
    schemaVersion: 1,
    release: {
      channel: "beta",
      notesSha256: SHA,
      tag: "v0.9.0",
      title: "Bottie 0.9.0 beta",
      version: "0.9.0",
    },
    inputs: {
      dependencyInventorySha256: SHA,
      licenceSha256: SHA,
      modelTermsSha256: SHA,
      noticesSha256: SHA,
      runtimeAssetsSha256: SHA,
    },
    artifacts: Object.fromEntries(
      PLATFORMS.map((platform) => [platform, structuredClone(platformEnvelopes[platform].outerDistribution)]),
    ),
    gates: gateIds.map((id) => ({ id, passed: true })),
    ready: true,
  };
}
