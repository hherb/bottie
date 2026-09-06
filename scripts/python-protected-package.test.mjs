import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import { bindPythonReleaseCandidate, buildSourceMarker } from "./python-release-candidate.mjs";
import { bindProtectedPythonPackage, protectedInspectionSha256 } from "./python-protected-package.mjs";

const CANDIDATE_SHA = "a".repeat(40);
const OTHER_SHA = "b".repeat(40);
const SHA = "c".repeat(64);

/** Returns the shared reviewed runtime identity recorded by every package inspection. */
function runtime() {
  return {
    schemaVersion: 1,
    fileCount: 539,
    licenceSha256: SHA,
    pythonVersion: "3.14.7",
    pythonWasmSha256: "d".repeat(64),
    runtimeTreeSha256: "e".repeat(64),
    totalBytes: 24_000_000,
    wasiSdkVersion: "24",
  };
}

/** Returns one complete development-package inspection for a supported platform. */
function developmentInspection(platform) {
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
  const packagedRuntime = runtime();
  if (platform === "windows") {
    packagedRuntime.fileCount = 540;
    packagedRuntime.runtimeTreeSha256 = "f".repeat(64);
    packagedRuntime.totalBytes = 34_000_000;
  }
  return {
    bundled: true,
    nativeTransports: transports[platform].map((path, index) => ({
      bytes: index + 10,
      path,
      sha256: `${index + 1}`.repeat(64),
    })),
    runnerBytes: 14_000_000,
    runnerSha256: SHA,
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

/** Returns one accepted development release-candidate manifest. */
function releaseCandidate() {
  const inputs = Object.fromEntries(
    ["linux", "macos", "windows"].map((platform) => {
      const inspection = developmentInspection(platform);
      return [
        platform,
        {
          containment: developmentContainment(platform),
          inspection,
          installedInspection: platform === "macos" ? undefined : structuredClone(inspection),
          marker: buildSourceMarker(platform, CANDIDATE_SHA),
        },
      ];
    }),
  );
  return bindPythonReleaseCandidate(CANDIDATE_SHA, inputs);
}

/** Returns a protected-package inspection whose signed native bytes may differ from development bytes. */
function protectedInspection(platform) {
  const inspection = developmentInspection(platform);
  inspection.runnerBytes += 1_024;
  inspection.runnerSha256 = "6".repeat(64);
  const transportDigests = ["7", "8", "9", "a"];
  inspection.nativeTransports = inspection.nativeTransports.map((transport, index) => ({
    ...transport,
    bytes: transport.bytes + 2_048,
    sha256: transportDigests[index].repeat(64),
  }));
  return inspection;
}

/** Returns separate shipping-containment evidence bound to one protected inspection. */
function shippingContainment(platform, inspection) {
  const common = {
    inspectionSha256: protectedInspectionSha256(inspection),
    platform,
    schemaVersion: 1,
    sourceSha: CANDIDATE_SHA,
    status: "ok",
    target: inspection.target,
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

describe("protected Python package comparison", () => {
  it("accepts changed signed native bytes only when the exact runtime identity remains unchanged", () => {
    const candidate = releaseCandidate();
    const inspection = protectedInspection("macos");

    const evidence = bindProtectedPythonPackage(
      CANDIDATE_SHA,
      "macos",
      candidate,
      inspection,
      shippingContainment("macos", inspection),
    );

    expect(evidence).toMatchObject({
      schemaVersion: 1,
      sourceSha: CANDIDATE_SHA,
      status: "accepted",
      platform: "macos",
      target: "aarch64-apple-darwin",
      runner: { bytes: 14_001_024, sha256: "6".repeat(64) },
      runtime: candidate.platforms.find(({ platform }) => platform === "macos").runtime,
    });
    expect(evidence.nativeTransports[0].sha256).toBe("7".repeat(64));
    expect(JSON.stringify(evidence)).not.toMatch(/Users|C:\\\\|\/private\/|identity|credential|rawOutput/);
  });

  it("rejects any protected runtime change or the wrong development platform identity", () => {
    const candidate = releaseCandidate();
    const changed = protectedInspection("linux");
    changed.runtime.runtimeTreeSha256 = "0".repeat(64);
    expect(() =>
      bindProtectedPythonPackage(CANDIDATE_SHA, "linux", candidate, changed, shippingContainment("linux", changed)),
    ).toThrow(/runtime identity/);

    const wrongTarget = protectedInspection("linux");
    wrongTarget.target = "x86_64-pc-windows-msvc";
    expect(() =>
      bindProtectedPythonPackage(
        CANDIDATE_SHA,
        "linux",
        candidate,
        wrongTarget,
        shippingContainment("linux", wrongTarget),
      ),
    ).toThrow(/inspection/);
  });

  it("requires separate closed shipping containment tied to the exact protected inspection", () => {
    const candidate = releaseCandidate();
    const inspection = protectedInspection("windows");
    const incomplete = shippingContainment("windows", inspection);
    incomplete.jobCloseKilledRunner = false;
    expect(() => bindProtectedPythonPackage(CANDIDATE_SHA, "windows", candidate, inspection, incomplete)).toThrow(
      /shipping containment/,
    );

    const stale = shippingContainment("windows", inspection);
    stale.inspectionSha256 = "0".repeat(64);
    expect(() => bindProtectedPythonPackage(CANDIDATE_SHA, "windows", candidate, inspection, stale)).toThrow(
      /protected inspection/,
    );

    const pathBearing = shippingContainment("windows", inspection);
    pathBearing.runnerPath = "C:\\private\\runner.exe";
    expect(() => bindProtectedPythonPackage(CANDIDATE_SHA, "windows", candidate, inspection, pathBearing)).toThrow(
      /not closed/,
    );
  });

  it("rejects mixed source revisions and tampered accepted candidate evidence", () => {
    const candidate = releaseCandidate();
    const inspection = protectedInspection("linux");
    const containment = shippingContainment("linux", inspection);
    containment.sourceSha = OTHER_SHA;
    expect(() => bindProtectedPythonPackage(CANDIDATE_SHA, "linux", candidate, inspection, containment)).toThrow(
      /source revision/,
    );

    const tampered = releaseCandidate();
    tampered.platforms[0].runtime.pythonWasmSha256 = "0".repeat(64);
    expect(() =>
      bindProtectedPythonPackage(
        CANDIDATE_SHA,
        "linux",
        tampered,
        inspection,
        shippingContainment("linux", inspection),
      ),
    ).toThrow(/release-candidate evidence/);
  });

  it("exposes only a credential-free local comparison command", async () => {
    const packageManifest = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
    const dependencyConfig = await readFile(new URL("./dependency-inventory-config.mjs", import.meta.url), "utf8");

    expect(packageManifest.scripts["python:protected:compare"]).toBe(
      "node scripts/python-protected-package.mjs --compare",
    );
    expect(dependencyConfig).toContain('"scripts/python-protected-package.mjs"');
  });
});
