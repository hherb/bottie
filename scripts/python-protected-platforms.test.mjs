import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import {
  bindProtectedPythonDistributionEnvelope,
  bindProtectedPythonPlatforms,
} from "./python-protected-platforms.mjs";
import {
  comparisons,
  envelopes,
  ordinaryReleaseCandidate,
  OTHER_OUTER_SHA,
  OTHER_SHA,
  outerDistributionEvidence,
  PLATFORMS,
  SHA,
  SOURCE_SHA,
} from "./python-protected-platforms-fixtures.mjs";
import { bindProtectedPythonReleaseEligibility } from "./python-protected-release.mjs";

describe("protected Python platform aggregation", () => {
  it("binds one comparison to the normalized same-run outer distribution", () => {
    const comparison = comparisons().macos;
    const rawOuter = outerDistributionEvidence("macos");
    rawOuter.privatePath = "/Users/private/bottie.app";
    const envelope = bindProtectedPythonDistributionEnvelope(SOURCE_SHA, "macos", comparison, rawOuter);

    expect(envelope).toMatchObject({
      schemaVersion: 1,
      sourceSha: SOURCE_SHA,
      status: "accepted",
      platform: "macos",
      comparison,
      outerDistribution: { identifier: "com.bottie.app", version: "0.9.0" },
    });
    expect(envelope.comparisonSha256).toMatch(/^[a-f0-9]{64}$/);
    expect(envelope.outerDistributionSha256).toMatch(/^[a-f0-9]{64}$/);
    expect(envelope.bindingSha256).toMatch(/^[a-f0-9]{64}$/);
    expect(JSON.stringify(envelope)).not.toContain("/Users/private");
  });

  it("rejects substituted, digest-drifted, or open envelopes", () => {
    const records = envelopes();
    records.macos.platform = "linux";
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, records)).toThrow(/envelope/);

    const digestDrift = envelopes();
    digestDrift.linux.outerDistribution.bundleDigest = OTHER_OUTER_SHA;
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, digestDrift)).toThrow(/outer-distribution binding/);

    const open = envelopes();
    open.windows.privatePath = "C:\\private\\bottie.msi";
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, open)).toThrow(/not closed/);

    const nestedOpen = envelopes();
    nestedOpen.windows.outerDistribution.smoke.privatePath = "C:\\private\\support";
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, nestedOpen)).toThrow(/not closed/);

    const incompleteOuter = outerDistributionEvidence("windows");
    incompleteOuter.bundle.installer.size = 0;
    expect(() =>
      bindProtectedPythonDistributionEnvelope(SOURCE_SHA, "windows", comparisons().windows, incompleteOuter),
    ).toThrow(/outer distribution.*complete/);
  });

  it("deterministically binds one complete same-revision protected platform set", () => {
    const evidence = bindProtectedPythonPlatforms(SOURCE_SHA, envelopes());

    expect(bindProtectedPythonPlatforms(SOURCE_SHA, envelopes())).toEqual(evidence);
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
    const missing = envelopes();
    delete missing.windows;
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, missing)).toThrow(/three platforms/);

    const mixedRevision = envelopes();
    mixedRevision.macos.comparison.sourceSha = OTHER_SHA;
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, mixedRevision)).toThrow(/source revision/);

    const mixedCandidate = envelopes();
    const substitutedComparison = structuredClone(mixedCandidate.windows.comparison);
    substitutedComparison.releaseCandidateSha256 = "0".repeat(64);
    mixedCandidate.windows = bindProtectedPythonDistributionEnvelope(
      SOURCE_SHA,
      "windows",
      substitutedComparison,
      outerDistributionEvidence("windows"),
    );
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, mixedCandidate)).toThrow(/release candidate/);
  });

  it("recomputes canonical inspection and containment bindings", () => {
    const changedInspection = envelopes();
    changedInspection.macos.comparison.runner.bytes += 1;
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, changedInspection)).toThrow(/inspection/);

    const changedContainment = envelopes();
    changedContainment.linux.comparison.containment.cancellation = false;
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, changedContainment)).toThrow(/containment/);

    const pathBearing = envelopes();
    pathBearing.windows.comparison.containment.privatePath = "C:\\private\\runner.exe";
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, pathBearing)).toThrow(/not closed/);
  });

  it("requires one shared runtime core while retaining platform-specific layouts", () => {
    const changedCore = envelopes();
    const otherComparisons = comparisons(539, "0".repeat(64));
    otherComparisons.windows.releaseCandidateSha256 = changedCore.windows.comparison.releaseCandidateSha256;
    const otherCore = bindProtectedPythonDistributionEnvelope(
      SOURCE_SHA,
      "windows",
      otherComparisons.windows,
      outerDistributionEvidence("windows"),
    );
    changedCore.windows = otherCore;
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, changedCore)).toThrow(/runtime core/);

    const changedLayout = envelopes();
    const otherLayoutComparisons = comparisons(540);
    otherLayoutComparisons.windows.releaseCandidateSha256 = changedLayout.windows.comparison.releaseCandidateSha256;
    const otherLayout = bindProtectedPythonDistributionEnvelope(
      SOURCE_SHA,
      "windows",
      otherLayoutComparisons.windows,
      outerDistributionEvidence("windows"),
    );
    changedLayout.windows = otherLayout;
    expect(() => bindProtectedPythonPlatforms(SOURCE_SHA, changedLayout)).toThrow(/Windows.*layout/);
  });

  it("exposes only a credential-free local aggregate command", async () => {
    const packageManifest = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
    const dependencyConfig = await readFile(new URL("./dependency-inventory-config.mjs", import.meta.url), "utf8");

    expect(packageManifest.scripts["python:protected:bind-platforms"]).toBe(
      "node scripts/python-protected-platforms.mjs --bind",
    );
    expect(packageManifest.scripts["python:protected:bind-envelope"]).toBe(
      "node scripts/python-protected-platforms.mjs --envelope",
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
    expect(workflow).toContain("bottie-python-linux-protected-envelope");
    expect(workflow).toContain("bottie-python-macos-protected-envelope");
    expect(workflow).toContain("bottie-python-windows-protected-envelope");
    expect(workflow).toContain("python:protected:bind-platforms");
    expect(workflow).toContain("package/python-protected-platform-evidence.json");
    expect(workflow).not.toMatch(/secrets\.|environment:/);

    expect(producers[0]).toContain("name: bottie-python-linux-protected-envelope");
    expect(producers[0]).toContain("package/python-protected-evidence/linux-envelope.json");
    expect(producers[1]).toContain("name: bottie-python-macos-protected-envelope");
    expect(producers[1]).toContain("package/python-protected-evidence/macos-envelope.json");
    expect(producers[2]).toContain("name: bottie-python-windows-protected-envelope");
    expect(producers[2]).toContain("package/python-protected-evidence/windows-envelope.json");
  });
});

describe("protected Python release eligibility", () => {
  it("binds one ready ordinary candidate to exact same-revision protected platform evidence", () => {
    const platformEnvelopes = envelopes();
    const platformEvidence = bindProtectedPythonPlatforms(SOURCE_SHA, platformEnvelopes);
    const evidence = bindProtectedPythonReleaseEligibility(
      SOURCE_SHA,
      ordinaryReleaseCandidate(platformEnvelopes),
      platformEvidence,
    );

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
    expect(evidence.platforms[1].outerDistributionSha256).toBe(platformEnvelopes.macos.outerDistributionSha256);
    expect(evidence.platforms[1].bindingSha256).toBe(platformEnvelopes.macos.bindingSha256);
    expect(JSON.stringify(evidence)).not.toMatch(/Users|C:\\\\|\/private\/|credential|rawOutput/);
  });

  it("rejects an unready, malformed, or version-inconsistent ordinary release candidate", () => {
    const platformEnvelopes = envelopes();
    const platformEvidence = bindProtectedPythonPlatforms(SOURCE_SHA, platformEnvelopes);
    const unready = ordinaryReleaseCandidate(platformEnvelopes);
    unready.ready = false;
    expect(() => bindProtectedPythonReleaseEligibility(SOURCE_SHA, unready, platformEvidence)).toThrow(/not ready/);

    const openGate = ordinaryReleaseCandidate(platformEnvelopes);
    openGate.gates[0] = { failure: "invalid-release-notes", id: "release-notes", passed: false };
    expect(() => bindProtectedPythonReleaseEligibility(SOURCE_SHA, openGate, platformEvidence)).toThrow(
      /release gates/,
    );

    const wrongTag = ordinaryReleaseCandidate(platformEnvelopes);
    wrongTag.release.tag = "v0.8.0";
    expect(() => bindProtectedPythonReleaseEligibility(SOURCE_SHA, wrongTag, platformEvidence)).toThrow(/release/);
  });

  it("rejects substitution between protected envelopes and ordinary outer distributions", () => {
    const platformEnvelopes = envelopes();
    const platformEvidence = bindProtectedPythonPlatforms(SOURCE_SHA, platformEnvelopes);
    const substituted = ordinaryReleaseCandidate(platformEnvelopes);
    substituted.artifacts.windows.installer.sha256 = OTHER_OUTER_SHA;

    expect(() => bindProtectedPythonReleaseEligibility(SOURCE_SHA, substituted, platformEvidence)).toThrow(
      /outer distribution/,
    );
  });

  it("revalidates the protected aggregate and exposes only a credential-free command", async () => {
    const platformEnvelopes = envelopes();
    const tampered = bindProtectedPythonPlatforms(SOURCE_SHA, platformEnvelopes);
    tampered.platforms[0].containmentSha256 = "0".repeat(64);
    expect(() =>
      bindProtectedPythonReleaseEligibility(SOURCE_SHA, ordinaryReleaseCandidate(platformEnvelopes), tampered),
    ).toThrow(/platform evidence/);

    const packageManifest = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
    expect(packageManifest.scripts["python:protected:release-eligibility"]).toBe(
      "node scripts/python-protected-release.mjs --bind",
    );
    const dependencyConfig = await readFile(new URL("./dependency-inventory-config.mjs", import.meta.url), "utf8");
    expect(dependencyConfig).toContain('"scripts/python-protected-release.mjs"');
  });
});
