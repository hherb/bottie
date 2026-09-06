import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import {
  credentialFreeLinuxEnvironment,
  linuxShippingContainmentRecord,
  linuxShippingVerificationPlan,
  parseInstalledLinuxProof,
  requireMatchingLinuxProtectedInspections,
} from "./linux-shipping-python-containment.mjs";
import { protectedInspectionSha256 } from "./python-protected-package.mjs";

const SOURCE_SHA = "a".repeat(40);
const SHA256 = "b".repeat(64);

/** Returns one complete path-free protected Linux package inspection. */
function protectedInspection() {
  return {
    bundled: true,
    nativeTransports: [],
    runnerBytes: 14_001_024,
    runnerSha256: SHA256,
    runtime: {
      schemaVersion: 1,
      fileCount: 539,
      licenceSha256: "c".repeat(64),
      pythonVersion: "3.14.7",
      pythonWasmSha256: "d".repeat(64),
      runtimeTreeSha256: "e".repeat(64),
      totalBytes: 24_000_000,
      wasiSdkVersion: "24",
    },
    target: "x86_64-unknown-linux-gnu",
  };
}

/** Returns the exact completed native proof emitted by the installed Linux verifier. */
function nativeProof() {
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

describe("Linux shipping Python containment producer", () => {
  it("creates one closed installed-package record bound to the exact protected inspection", () => {
    const inspection = protectedInspection();

    expect(linuxShippingContainmentRecord(SOURCE_SHA, inspection, nativeProof())).toEqual({
      cancellation: true,
      environmentIsolated: true,
      execDenied: true,
      installedProtectedPackage: true,
      inspectionSha256: protectedInspectionSha256(inspection),
      landlockDeniedHostFixture: true,
      networkDenied: true,
      parentCloseKilledRunner: true,
      parentDeathSignal: true,
      platform: "linux",
      processCreationDenied: true,
      resourceLimits: true,
      runtimeReadable: true,
      schemaVersion: 1,
      sourceSha: SOURCE_SHA,
      status: "ok",
      target: "x86_64-unknown-linux-gnu",
      workspaceReadable: true,
    });

    const incomplete = nativeProof();
    incomplete.networkDenied = false;
    expect(() => linuxShippingContainmentRecord(SOURCE_SHA, inspection, incomplete)).toThrow(/native proof/);
    expect(() => linuxShippingContainmentRecord("../main", inspection, nativeProof())).toThrow(/source revision/);
  });

  it("parses only complete path-free native proof output", () => {
    expect(parseInstalledLinuxProof(JSON.stringify(nativeProof()))).toEqual(nativeProof());
    expect(() => parseInstalledLinuxProof("not json /home/runner")).toThrow(/invalid JSON/);

    const pathBearing = nativeProof();
    pathBearing.runnerPath = "/usr/bin/bottie-python-runner";
    expect(() => parseInstalledLinuxProof(JSON.stringify(pathBearing))).toThrow(/incomplete/);
  });

  it("removes signing and loader overrides from every child process", () => {
    expect(
      credentialFreeLinuxEnvironment({
        BOTTIE_UPDATER_SIGNING_PRIVATE_KEY: "secret",
        BOTTIE_LINUX_SIGNING_PRIVATE_KEY_BASE64: "secret",
        GNUPGHOME: "/private/gnupg",
        HOME: "/home/runner",
        LD_PRELOAD: "/private/injected.so",
        PATH: "/usr/bin",
        TAURI_SIGNING_PRIVATE_KEY: "secret",
      }),
    ).toEqual({ HOME: "/home/runner", PATH: "/usr/bin" });
  });

  it("requires fixed-layout installed bytes to equal the extracted signed package", () => {
    const expected = protectedInspection();
    expect(requireMatchingLinuxProtectedInspections(expected, structuredClone(expected))).toEqual(expected);

    const changed = structuredClone(expected);
    changed.runnerSha256 = "f".repeat(64);
    expect(() => requireMatchingLinuxProtectedInspections(expected, changed)).toThrow(/installed Linux package/);

    const expanded = structuredClone(expected);
    expanded.installedPath = "/usr/lib/bottie/python-runtime";
    expect(() => requireMatchingLinuxProtectedInspections(expected, expanded)).toThrow(/not closed/);
  });

  it("verifies the signed DEB with only checked-in public trust material", () => {
    const plan = linuxShippingVerificationPlan("/repo", "/tmp/proof", "/tmp/bottie.deb");

    expect(plan).toEqual([
      {
        arguments: [
          "--batch",
          "--yes",
          "--dearmor",
          "--output",
          "/tmp/proof/keyrings/5C1D104ACE472474CE21070B065CFE6D5D9FD8A4/bottie.gpg",
          "/repo/distribution/linux/bottie-linux-signing-public.asc",
        ],
        command: "/usr/bin/gpg",
        label: "published Linux public key",
      },
      {
        arguments: [
          "--policies-dir",
          "/tmp/proof/policies",
          "--keyrings-dir",
          "/tmp/proof/keyrings",
          "/tmp/bottie.deb",
        ],
        command: "debsig-verify",
        label: "signed Linux protected package",
      },
    ]);
    expect(JSON.stringify(plan)).not.toMatch(/secret|private-key|credential/i);
  });

  it("exposes a credential-free producer without protected-workflow composition", async () => {
    const packageManifest = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
    const dependencyConfig = await readFile(new URL("./dependency-inventory-config.mjs", import.meta.url), "utf8");
    const provenanceWorkflow = await readFile(
      new URL("../.github/workflows/python-runtime-provenance.yml", import.meta.url),
      "utf8",
    );
    const protectedWorkflow = await readFile(
      new URL("../.github/workflows/linux-distribution-validation.yml", import.meta.url),
      "utf8",
    );
    const containmentWorkflow = await readFile(
      new URL("../.github/workflows/linux-python-containment.yml", import.meta.url),
      "utf8",
    );
    const producer = await readFile(new URL("./linux-shipping-python-containment.mjs", import.meta.url), "utf8");

    expect(packageManifest.scripts["python:protected:linux:prove-shipping"]).toBe(
      "node scripts/linux-shipping-python-containment.mjs --prove",
    );
    expect(dependencyConfig).toContain('"scripts/linux-shipping-python-containment.mjs"');
    expect(provenanceWorkflow).not.toContain("python:protected:linux:prove-shipping");
    expect(protectedWorkflow).not.toContain("python:protected:linux:prove-shipping");
    expect(containmentWorkflow).toContain("scripts/linux-shipping-python-containment.mjs");
    expect(containmentWorkflow).toContain("scripts/linux-shipping-python-containment.test.mjs");
    expect(producer.indexOf("linuxShippingVerificationPlan(repository, temporary, deb)")).toBeLessThan(
      producer.indexOf('runHostCommand("dpkg-deb"'),
    );
    expect(producer.indexOf('runHostCommand("dpkg-deb"')).toBeLessThan(
      producer.indexOf("validateProtectedPythonInspection("),
    );
    expect(producer.indexOf("validateProtectedPythonInspection(")).toBeLessThan(
      producer.indexOf('inspectPackagedPythonBundle("/"'),
    );
    expect(producer.indexOf('inspectPackagedPythonBundle("/"')).toBeLessThan(producer.indexOf('"--prove-installed"'));
  });
});
