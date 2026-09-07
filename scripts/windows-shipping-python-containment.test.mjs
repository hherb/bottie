import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import {
  credentialFreeWindowsEnvironment,
  parseInstalledWindowsProof,
  requireMatchingWindowsProtectedInspections,
  windowsShippingContainmentRecord,
  windowsShippingVerificationPlan,
} from "./windows-shipping-python-containment.mjs";
import { protectedInspectionSha256 } from "./python-protected-package.mjs";

const SOURCE_SHA = "a".repeat(40);
const SHA256 = "b".repeat(64);

/** Returns one complete path-free protected Windows package inspection. */
function protectedInspection() {
  return {
    bundled: true,
    nativeTransports: [
      {
        bytes: 420_000,
        path: "bottie-python-appcontainer.exe",
        sha256: "c".repeat(64),
      },
    ],
    runnerBytes: 14_001_024,
    runnerSha256: SHA256,
    runtime: {
      schemaVersion: 1,
      fileCount: 540,
      licenceSha256: "d".repeat(64),
      pythonVersion: "3.14.7",
      pythonWasmSha256: "e".repeat(64),
      runtimeTreeSha256: "f".repeat(64),
      totalBytes: 25_000_000,
      wasiSdkVersion: "24",
    },
    target: "x86_64-pc-windows-msvc",
  };
}

/** Returns the exact completed native proof emitted by the installed Windows verifier. */
function nativeProof() {
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

describe("Windows shipping Python containment producer", () => {
  it("creates one closed installed-package record bound to the exact protected inspection", () => {
    const inspection = protectedInspection();

    expect(windowsShippingContainmentRecord(SOURCE_SHA, inspection, nativeProof())).toEqual({
      appContainerDeniedHostFixture: true,
      appContainerLowIntegrity: true,
      appContainerNoCapabilities: true,
      cancellation: true,
      installedProtectedPackage: true,
      inspectionSha256: protectedInspectionSha256(inspection),
      jobCloseKilledRunner: true,
      platform: "windows",
      privatePipeExecution: true,
      privilegesStripped: true,
      resourceLimits: true,
      schemaVersion: 1,
      sourceSha: SOURCE_SHA,
      status: "ok",
      target: "x86_64-pc-windows-msvc",
    });

    const incomplete = nativeProof();
    incomplete.appContainerNoCapabilities = false;
    expect(() => windowsShippingContainmentRecord(SOURCE_SHA, inspection, incomplete)).toThrow(/native proof/);
    expect(() => windowsShippingContainmentRecord("../main", inspection, nativeProof())).toThrow(/source revision/);
  });

  it("parses only complete path-free installed native proof output", () => {
    expect(parseInstalledWindowsProof(JSON.stringify(nativeProof()))).toEqual(nativeProof());
    expect(() => parseInstalledWindowsProof("not json C:\\private")).toThrow(/invalid JSON/);

    const pathBearing = nativeProof();
    pathBearing.profilePath = "C:\\Users\\runner\\AppData\\Local\\Packages\\proof";
    expect(() => parseInstalledWindowsProof(JSON.stringify(pathBearing))).toThrow(/incomplete/);
  });

  it("removes signing credentials and process-injection overrides from every child process", () => {
    expect(
      credentialFreeWindowsEnvironment({
        APPDATA: "C:\\Users\\runner\\AppData\\Roaming",
        BOTTIE_PYTHON_INSTALLED_ROOT: "C:\\stale\\bundle",
        BOTTIE_PYTHON_WASI_RUNTIME: "C:\\stale\\runtime",
        BOTTIE_UPDATER_SIGNING_PRIVATE_KEY: "secret",
        BOTTIE_WINDOWS_SIGNING_CERTIFICATE_PASSWORD: "secret",
        BOTTIE_WINDOWS_SIGNTOOL_PATH: "C:\\sdk\\signtool.exe",
        COR_ENABLE_PROFILING: "1",
        DOTNET_STARTUP_HOOKS: "C:\\private\\hook.dll",
        NODE_OPTIONS: "--require C:\\private\\inject.cjs",
        PATH: "C:\\Windows\\System32",
        TAURI_SIGNING_PRIVATE_KEY: "secret",
      }),
    ).toEqual({
      APPDATA: "C:\\Users\\runner\\AppData\\Roaming",
      PATH: "C:\\Windows\\System32",
    });
  });

  it("requires fixed-layout installed bytes to equal the extracted signed package", () => {
    const expected = protectedInspection();
    expect(requireMatchingWindowsProtectedInspections(expected, structuredClone(expected))).toEqual(expected);

    const changed = structuredClone(expected);
    changed.nativeTransports[0].sha256 = "0".repeat(64);
    expect(() => requireMatchingWindowsProtectedInspections(expected, changed)).toThrow(/installed Windows package/);

    const expanded = structuredClone(expected);
    expanded.installedPath = "C:\\Program Files\\bottie";
    expect(() => requireMatchingWindowsProtectedInspections(expected, expanded)).toThrow(/not closed/);
  });

  it("independently verifies the signed MSI and every extracted executable without signing", () => {
    const plan = windowsShippingVerificationPlan(
      "C:\\sdk\\signtool.exe",
      "C:\\package\\bottie.msi",
      "C:\\extracted\\PFiles\\bottie",
    );

    expect(plan.map(({ label }) => label)).toEqual([
      "signed Windows protected package",
      "protected Bottie application",
      "protected Python AppContainer controller",
      "protected Python runner",
    ]);
    expect(windowsShippingVerificationPlan("C:\\sdk\\signtool.exe", "C:\\package\\bottie.msi")).toEqual([plan[0]]);
    expect(plan.map(({ arguments: arguments_ }) => arguments_.at(-1))).toEqual([
      "C:\\package\\bottie.msi",
      "C:\\extracted\\PFiles\\bottie\\bottie.exe",
      "C:\\extracted\\PFiles\\bottie\\bottie-python-appcontainer.exe",
      "C:\\extracted\\PFiles\\bottie\\bottie-python-runner.exe",
    ]);
    for (const step of plan) {
      expect(step.command).toBe("C:\\sdk\\signtool.exe");
      expect(step.arguments).toEqual(["verify", "/pa", "/all", "/v", step.arguments.at(-1)]);
      expect(step.arguments).not.toContain("sign");
    }
  });

  it("registers the credential-free proof without giving the producer signing authority", async () => {
    const packageManifest = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
    const dependencyConfig = await readFile(new URL("./dependency-inventory-config.mjs", import.meta.url), "utf8");
    const producer = await readFile(new URL("./windows-shipping-python-containment.mjs", import.meta.url), "utf8");
    const containmentWorkflow = await readFile(
      new URL("../.github/workflows/windows-python-appcontainer.yml", import.meta.url),
      "utf8",
    );
    const distributionWorkflow = await readFile(
      new URL("../.github/workflows/windows-distribution-validation.yml", import.meta.url),
      "utf8",
    );

    expect(packageManifest.scripts["python:protected:windows:prove-shipping"]).toBe(
      "node scripts/windows-shipping-python-containment.mjs --prove",
    );
    expect(dependencyConfig).toContain('"scripts/windows-shipping-python-containment.mjs"');
    expect(containmentWorkflow).toContain("scripts/windows-shipping-python-containment.test.mjs");
    expect(containmentWorkflow).toContain("scripts/windows-distribution.test.mjs");
    expect(containmentWorkflow).toContain('".github/workflows/windows-distribution-validation.yml"');
    expect(distributionWorkflow).toContain("python:protected:windows:prove-shipping");
    expect(distributionWorkflow).toContain("python:protected:compare");
    expect(
      producer.indexOf("runVerificationPlan(windowsShippingVerificationPlan(signTool, msi), environment)"),
    ).toBeLessThan(producer.indexOf("msiAdministrativeInstallArguments(msi, extracted)"));
    expect(
      producer.indexOf("windowsShippingVerificationPlan(signTool, msi, packageApplication).slice(1)"),
    ).toBeLessThan(producer.indexOf("validateProtectedPythonInspection("));
    expect(producer.lastIndexOf("validateProtectedPythonInspection(")).toBeLessThan(
      producer.lastIndexOf("requireMatchingWindowsProtectedInspections("),
    );
    expect(producer.lastIndexOf("requireMatchingWindowsProtectedInspections(")).toBeLessThan(
      producer.indexOf('"--prove-installed"'),
    );
    expect(producer).not.toMatch(/signToolSignArguments|\bsign\b|certificate|password|cargo|gh workflow/i);
  });
});
