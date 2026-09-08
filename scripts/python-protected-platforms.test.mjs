import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import { bindProtectedPythonPackage, protectedInspectionSha256 } from "./python-protected-package.mjs";
import { bindProtectedPythonPlatforms } from "./python-protected-platforms.mjs";
import { bindProtectedPythonReleaseEligibility } from "./python-protected-release.mjs";
import { bindPythonReleaseCandidate, buildSourceMarker } from "./python-release-candidate.mjs";

const SOURCE_SHA = "a".repeat(40);
const OTHER_SHA = "b".repeat(40);
const SHA = "c".repeat(64);
const PLATFORMS = ["linux", "macos", "windows"];

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
function comparisons(baseFileCount = 539, pythonWasmSha256 = "d".repeat(64)) {
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

/** Returns one normalized ordinary release-candidate manifest with every existing release gate passed. */
function ordinaryReleaseCandidate() {
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
    artifacts: { linux: { digest: SHA }, macos: { digest: SHA }, windows: { digest: SHA } },
    gates: gateIds.map((id) => ({ id, passed: true })),
    ready: true,
  };
}

describe("protected Python platform aggregation", () => {
  it("deterministically binds one complete same-revision protected platform set", () => {
    const evidence = bindProtectedPythonPlatforms(SOURCE_SHA, comparisons());

    expect(bindProtectedPythonPlatforms(SOURCE_SHA, comparisons())).toEqual(evidence);
    expect(evidence).toMatchObject({
      schemaVersion: 1,
      sourceSha: SOURCE_SHA,
      status: "accepted",
      runtimeCore: {
        schemaVersion: 1,
        licenceSha256: SHA,
        pythonVersion: "3.14.7",
        pythonWasmSha256: "d".repeat(64),
        wasiSdkVersion: "24",
      },
    });
    expect(evidence.platforms.map(({ platform }) => platform)).toEqual(PLATFORMS);
    expect(evidence.platforms[1].nativeTransports[0].sha256).toBe("6".repeat(64));
    expect(evidence.platforms[2].runner.sha256).toBe("6".repeat(64));
    expect(JSON.stringify(evidence)).not.toMatch(/Users|C:\\\\|\/private\/|credential|rawOutput/);
  });

  it("rejects missing, mixed-revision, or mixed-candidate platform records", () => {
    const missing = comparisons();
    delete missing.windows;
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, missing)).toThrow(/three platforms/);

    const mixedRevision = comparisons();
    mixedRevision.macos.sourceSha = OTHER_SHA;
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, mixedRevision)).toThrow(/source revision/);

    const mixedCandidate = comparisons();
    mixedCandidate.windows.releaseCandidateSha256 = "0".repeat(64);
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, mixedCandidate)).toThrow(/release candidate/);
  });

  it("recomputes canonical inspection and containment bindings", () => {
    const changedInspection = comparisons();
    changedInspection.macos.runner.bytes += 1;
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, changedInspection)).toThrow(/inspection/);

    const changedContainment = comparisons();
    changedContainment.linux.containment.cancellation = false;
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, changedContainment)).toThrow(/containment/);

    const pathBearing = comparisons();
    pathBearing.windows.containment.privatePath = "C:\\private\\runner.exe";
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, pathBearing)).toThrow(/not closed/);
  });

  it("requires one shared runtime core while retaining platform-specific layouts", () => {
    const changedCore = comparisons();
    const otherCore = comparisons(539, "0".repeat(64));
    otherCore.windows.releaseCandidateSha256 = changedCore.windows.releaseCandidateSha256;
    changedCore.windows = otherCore.windows;
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, changedCore)).toThrow(/runtime core/);

    const changedLayout = comparisons();
    const otherLayout = comparisons(540);
    otherLayout.windows.releaseCandidateSha256 = changedLayout.windows.releaseCandidateSha256;
    changedLayout.windows = otherLayout.windows;
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, changedLayout)).toThrow(/Windows.*layout/);
  });

  it("exposes only a credential-free local aggregate command", async () => {
    const packageManifest = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
    const dependencyConfig = await readFile(new URL("./dependency-inventory-config.mjs", import.meta.url), "utf8");

    expect(packageManifest.scripts["python:protected:bind-platforms"]).toBe(
      "node scripts/python-protected-platforms.mjs --bind",
    );
    expect(dependencyConfig).toContain('"scripts/python-protected-platforms.mjs"');
  });

  it("composes only explicit successful same-revision comparison runs", async () => {
    const workflow = await readFile(
      new URL("../.github/workflows/python-protected-platforms.yml", import.meta.url),
      "utf8",
    );
    const producerPaths = [
      "../.github/workflows/linux-distribution-validation.yml",
      "../.github/workflows/macos-distribution-validation.yml",
      "../.github/workflows/windows-distribution-validation.yml",
    ];
    const producers = await Promise.all(producerPaths.map((path) => readFile(new URL(path, import.meta.url), "utf8")));

    expect(workflow).toContain("workflow_dispatch:");
    expect(workflow).toContain("linux_run_id:");
    expect(workflow).toContain("macos_run_id:");
    expect(workflow).toContain("windows_run_id:");
    expect(workflow).toContain("actions: read");
    expect(workflow).toContain("contents: read");
    expect(workflow).toContain('.head_sha == $source and .conclusion == "success"');
    expect(workflow).toContain('event == "workflow_dispatch"');
    expect(workflow).toContain("bottie-python-linux-protected-comparison");
    expect(workflow).toContain("bottie-python-macos-protected-comparison");
    expect(workflow).toContain("bottie-python-windows-protected-comparison");
    expect(workflow).toContain("python:protected:bind-platforms");
    expect(workflow).toContain("package/python-protected-platform-evidence.json");
    expect(workflow).not.toMatch(/secrets\.|environment:/);

    expect(producers[0]).toContain("name: bottie-python-linux-protected-comparison");
    expect(producers[0]).toContain("package/python-protected-evidence/linux-accepted.json");
    expect(producers[1]).toContain("name: bottie-python-macos-protected-comparison");
    expect(producers[1]).toContain("package/python-protected-evidence/macos-accepted.json");
    expect(producers[2]).toContain("name: bottie-python-windows-protected-comparison");
    expect(producers[2]).toContain("package/python-protected-evidence/windows-accepted.json");
  });
});

describe("protected Python release eligibility", () => {
  it("binds one ready ordinary candidate to exact same-revision protected platform evidence", () => {
    const platformEvidence = bindProtectedPythonPlatforms(SOURCE_SHA, comparisons());
    const evidence = bindProtectedPythonReleaseEligibility(SOURCE_SHA, ordinaryReleaseCandidate(), platformEvidence);

    expect(evidence).toMatchObject({
      schemaVersion: 1,
      sourceSha: SOURCE_SHA,
      status: "eligible",
      release: { channel: "beta", tag: "v0.9.0", version: "0.9.0" },
      protectedPythonReleaseCandidateSha256: platformEvidence.releaseCandidateSha256,
      runtimeCore: platformEvidence.runtimeCore,
    });
    expect(evidence.platforms.map(({ platform }) => platform)).toEqual(PLATFORMS);
    expect(evidence.platforms[1].nativeTransports[0].sha256).toBe("6".repeat(64));
    expect(JSON.stringify(evidence)).not.toMatch(/Users|C:\\\\|\/private\/|credential|rawOutput/);
  });

  it("rejects an unready, malformed, or version-inconsistent ordinary release candidate", () => {
    const platformEvidence = bindProtectedPythonPlatforms(SOURCE_SHA, comparisons());
    const unready = ordinaryReleaseCandidate();
    unready.ready = false;
    expect(() => bindProtectedPythonReleaseEligibility(SOURCE_SHA, unready, platformEvidence)).toThrow(/not ready/);

    const openGate = ordinaryReleaseCandidate();
    openGate.gates[0] = { failure: "invalid-release-notes", id: "release-notes", passed: false };
    expect(() => bindProtectedPythonReleaseEligibility(SOURCE_SHA, openGate, platformEvidence)).toThrow(
      /release gates/,
    );

    const wrongTag = ordinaryReleaseCandidate();
    wrongTag.release.tag = "v0.8.0";
    expect(() => bindProtectedPythonReleaseEligibility(SOURCE_SHA, wrongTag, platformEvidence)).toThrow(/release/);
  });

  it("revalidates the protected aggregate and exposes only a credential-free command", async () => {
    const tampered = bindProtectedPythonPlatforms(SOURCE_SHA, comparisons());
    tampered.platforms[0].containmentSha256 = "0".repeat(64);
    expect(() => bindProtectedPythonReleaseEligibility(SOURCE_SHA, ordinaryReleaseCandidate(), tampered)).toThrow(
      /platform evidence/,
    );

    const packageManifest = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
    expect(packageManifest.scripts["python:protected:release-eligibility"]).toBe(
      "node scripts/python-protected-release.mjs --bind",
    );
    const dependencyConfig = await readFile(new URL("./dependency-inventory-config.mjs", import.meta.url), "utf8");
    expect(dependencyConfig).toContain('"scripts/python-protected-release.mjs"');
  });
});
