import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import {
  macosShippingContainmentRecord,
  macosShippingProofEnvironment,
  macosShippingVerificationPlan,
  requireMatchingMacosProtectedInspection,
} from "./macos-shipping-python-containment.mjs";
import { protectedInspectionSha256 } from "./python-protected-package.mjs";

const SOURCE_SHA = "a".repeat(40);
const SHA256 = "b".repeat(64);

/** Returns one complete path-free protected macOS package inspection. */
function protectedInspection() {
  return {
    bundled: true,
    nativeTransports: [
      "Contents/Helpers/BottiePythonXPCClient.app/Contents/Info.plist",
      "Contents/Helpers/BottiePythonXPCClient.app/Contents/MacOS/bottie-python-xpc-client",
      "Contents/Helpers/BottiePythonXPCClient.app/Contents/XPCServices/" +
        "com.bottie.python-runner.xpc/Contents/Info.plist",
      "Contents/Helpers/BottiePythonXPCClient.app/Contents/XPCServices/" +
        "com.bottie.python-runner.xpc/Contents/MacOS/bottie-python-xpc-service",
    ].map((path, index) => ({ bytes: index + 10, path, sha256: `${index + 1}`.repeat(64) })),
    runnerBytes: 14_000_000,
    runnerSha256: SHA256,
    runtime: {
      schemaVersion: 1,
      fileCount: 539,
      licenceSha256: SHA256,
      pythonVersion: "3.14.7",
      pythonWasmSha256: "c".repeat(64),
      runtimeTreeSha256: "d".repeat(64),
      totalBytes: 24_000_000,
      wasiSdkVersion: "24",
    },
    target: "aarch64-apple-darwin",
  };
}

describe("macOS shipping Python containment producer", () => {
  it("emits only the closed inspection-bound macOS shipping record", () => {
    const inspection = protectedInspection();

    expect(macosShippingContainmentRecord(SOURCE_SHA, inspection)).toEqual({
      appSandboxDeniedHostFixture: true,
      cancellation: true,
      clientExitKilledRunner: true,
      inspectedProtectedPackage: true,
      inspectionSha256: protectedInspectionSha256(inspection),
      platform: "macos",
      privatePipeExecution: true,
      schemaVersion: 1,
      sourceSha: SOURCE_SHA,
      status: "ok",
      target: "aarch64-apple-darwin",
    });
  });

  it("requires the reinspection to equal the exact closed supplied inspection", () => {
    const expected = protectedInspection();
    const reordered = Object.fromEntries(Object.entries(structuredClone(expected)).reverse());

    expect(requireMatchingMacosProtectedInspection(reordered, expected)).toEqual(expected);

    const changedRuntime = structuredClone(expected);
    changedRuntime.runtime.runtimeTreeSha256 = "0".repeat(64);
    expect(() => requireMatchingMacosProtectedInspection(expected, changedRuntime)).toThrow(/does not match/);

    const expanded = structuredClone(expected);
    expanded.applicationPath = "/private/protected/bottie.app";
    expect(() => requireMatchingMacosProtectedInspection(expanded, expected)).toThrow(/not closed/);
  });

  it("rejects malformed source revisions before creating a shipping claim", () => {
    expect(() => macosShippingContainmentRecord("../main", protectedInspection())).toThrow(/source revision/);
  });

  it("verifies exact nested and outer signed code without signing or recursive verification", () => {
    const plan = macosShippingVerificationPlan("/tmp/bottie.app");

    expect(plan.map(({ label }) => label)).toEqual([
      "packaged Python runner",
      "packaged Python XPC service",
      "packaged Python XPC client",
      "protected Bottie application",
    ]);
    expect(
      macosShippingProofEnvironment({
        BOTTIE_APPLE_DISTRIBUTION_IDENTITY: "private",
        CODESIGN_ALLOCATE: "/private/tool",
        DYLD_INSERT_LIBRARIES: "/private/library.dylib",
        PATH: "/usr/bin",
        TAURI_SIGNING_PRIVATE_KEY: "private",
      }),
    ).toEqual({ PATH: "/usr/bin" });
    expect(plan.at(-1).arguments.at(-1)).toBe("/tmp/bottie.app");
    for (const step of plan) {
      expect(step.command).toBe("/usr/bin/codesign");
      expect(step.arguments).toContain("--verify");
      expect(step.arguments).toContain("--strict");
      expect(step.arguments).not.toContain("--deep");
      expect(step.arguments).not.toContain("--sign");
      expect(step.arguments).not.toContain("--timestamp");
    }
  });

  it("registers a local credential-free proof command without workflow dispatch", async () => {
    const packageManifest = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
    const dependencyConfig = await readFile(new URL("./dependency-inventory-config.mjs", import.meta.url), "utf8");
    const producer = await readFile(new URL("./macos-shipping-python-containment.mjs", import.meta.url), "utf8");
    const xpcProof = await readFile(new URL("./macos-python-xpc.mjs", import.meta.url), "utf8");
    const provenanceWorkflow = await readFile(
      new URL("../.github/workflows/python-runtime-provenance.yml", import.meta.url),
      "utf8",
    );

    expect(packageManifest.scripts["python:protected:macos:prove-shipping"]).toBe(
      "node scripts/macos-shipping-python-containment.mjs --prove",
    );
    expect(dependencyConfig).toContain('"scripts/macos-shipping-python-containment.mjs"');
    expect(provenanceWorkflow).not.toContain("python:protected:macos:prove-shipping");
    expect(producer).toContain("macosShippingProofEnvironment(process.env)");
    expect(producer).toContain("exerciseProof(packagedBundleLayout(application), fixtureDirectory, environment)");
    expect(xpcProof).toContain("env: environment");
    expect(producer).not.toMatch(/--sign|--deep|notary|xcrun|cargo|tauri|actions\/|gh workflow/);
  });
});
