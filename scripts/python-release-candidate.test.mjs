import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import { bindPythonReleaseCandidate, buildSourceMarker } from "./python-release-candidate.mjs";

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

/** Returns one exact package inspection for a supported platform. */
function inspection(platform) {
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
  return {
    bundled: true,
    nativeTransports: transports[platform].map((path, index) => ({
      bytes: index + 10,
      path,
      sha256: `${index + 1}`.repeat(64),
    })),
    runnerBytes: 14_000_000,
    runnerSha256: SHA,
    runtime: runtime(),
    target: targets[platform],
  };
}

/** Returns the complete closed containment record for a supported platform. */
function containment(platform) {
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

/** Returns one complete three-platform candidate fixture. */
function candidateInputs() {
  return Object.fromEntries(
    ["linux", "macos", "windows"].map((platform) => {
      const packageInspection = inspection(platform);
      return [
        platform,
        {
          containment: containment(platform),
          inspection: packageInspection,
          installedInspection: platform === "macos" ? undefined : structuredClone(packageInspection),
          marker: buildSourceMarker(platform, CANDIDATE_SHA),
        },
      ];
    }),
  );
}

describe("Python release-candidate evidence", () => {
  it("creates only an exact path-free source marker", () => {
    expect(buildSourceMarker("linux", CANDIDATE_SHA)).toEqual({
      schemaVersion: 1,
      platform: "linux",
      sourceSha: CANDIDATE_SHA,
    });
    expect(() => buildSourceMarker("android", CANDIDATE_SHA)).toThrow(/platform/);
    expect(() => buildSourceMarker("linux", "main")).toThrow(/source revision/);
  });

  it("deterministically binds one complete consistent three-platform proof", () => {
    const inputs = candidateInputs();

    const first = bindPythonReleaseCandidate(CANDIDATE_SHA, inputs);
    const second = bindPythonReleaseCandidate(CANDIDATE_SHA, inputs);

    expect(second).toEqual(first);
    expect(first).toMatchObject({
      schemaVersion: 1,
      sourceSha: CANDIDATE_SHA,
      status: "accepted",
      runtime: runtime(),
    });
    expect(first.platforms.map((item) => item.platform)).toEqual(["linux", "macos", "windows"]);
    expect(first.platforms.every((item) => /^[a-f0-9]{64}$/.test(item.inspectionSha256))).toBe(true);
    expect(first.platforms.find((item) => item.platform === "macos").installedInspectionSha256).toBeNull();
  });

  it("rejects missing or mixed-revision evidence", () => {
    const missing = candidateInputs();
    delete missing.windows;
    expect(() => bindPythonReleaseCandidate(CANDIDATE_SHA, missing)).toThrow(/three platforms/);

    const mixed = candidateInputs();
    mixed.macos.marker = buildSourceMarker("macos", OTHER_SHA);
    expect(() => bindPythonReleaseCandidate(CANDIDATE_SHA, mixed)).toThrow(/source revision/);
  });

  it("rejects changed installed bytes or inconsistent shared runtime identity", () => {
    const installedMismatch = candidateInputs();
    installedMismatch.windows.installedInspection.runnerSha256 = OTHER_SHA.repeat(2).slice(0, 64);
    expect(() => bindPythonReleaseCandidate(CANDIDATE_SHA, installedMismatch)).toThrow(/installed inspection/);

    const runtimeMismatch = candidateInputs();
    runtimeMismatch.linux.inspection.runtime.runtimeTreeSha256 = "f".repeat(64);
    runtimeMismatch.linux.installedInspection.runtime.runtimeTreeSha256 = "f".repeat(64);
    expect(() => bindPythonReleaseCandidate(CANDIDATE_SHA, runtimeMismatch)).toThrow(/runtime identity/);
  });

  it("rejects incomplete or path-bearing evidence instead of retaining it", () => {
    const incomplete = candidateInputs();
    incomplete.macos.containment.clientExitKilledRunner = false;
    expect(() => bindPythonReleaseCandidate(CANDIDATE_SHA, incomplete)).toThrow(/containment/);

    const addedField = candidateInputs();
    addedField.windows.inspection.privatePath = "C:\\private\\runner";
    expect(() => bindPythonReleaseCandidate(CANDIDATE_SHA, addedField)).toThrow(/inspection/);

    expect(JSON.stringify(bindPythonReleaseCandidate(CANDIDATE_SHA, candidateInputs()))).not.toMatch(
      /Users|C:\\\\private|\/private\/|runner\\\\|runner\//,
    );
  });

  it("binds and uploads the credential-free result in the provenance workflow", async () => {
    const workflow = await readFile(
      new URL("../.github/workflows/python-runtime-provenance.yml", import.meta.url),
      "utf8",
    );
    const packageManifest = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));

    expect(packageManifest.scripts["python:evidence:bind"]).toBe("node scripts/python-release-candidate.mjs --bind");
    expect(workflow).toContain("Stamp the checked-out source revision");
    expect(workflow).toContain("python-release-candidate.mjs --stamp");
    expect(workflow).toContain("name: Bind Python release-candidate evidence");
    expect(workflow).toContain("needs: bundle-and-inspect");
    expect(workflow).toContain("merge-multiple: true");
    expect(workflow).toContain('npm run --silent python:evidence:bind -- "$GITHUB_SHA"');
    expect(workflow).toContain("package/python-release-candidate-evidence.json");
    expect(workflow).not.toMatch(/secrets\./);
  });
});
