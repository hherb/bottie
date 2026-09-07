import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import {
  distributionBuildArguments,
  distributionBundleArguments,
  protectedPythonDistributionBuildArguments,
  protectedPythonDistributionBundleArguments,
  protectedPythonDistributionSigningPlan,
  resolveSigningCredentials,
  signedPythonRunnerEvidence,
  signToolSignArguments,
  signToolVerifyArguments,
} from "./windows-distribution.mjs";

const CERTIFICATE_PATH = "C:\\runner-temp\\bottie-signing.pfx";
const EXECUTABLE_PATH = "C:\\target\\release\\bottie.exe";
const INSTALLER_PATH = "C:\\target\\release\\bundle\\msi\\bottie.msi";
const PYTHON_CONFIG = "src-tauri/tauri.python-development.windows.conf.json";

describe("Windows distribution signing", () => {
  it("builds locked product bytes once, then bundles the signed executable without automatic signing", () => {
    expect(distributionBuildArguments()).toEqual(["build", "--no-bundle", "--no-sign", "--ci", "--", "--locked"]);
    expect(distributionBundleArguments()).toEqual([
      "bundle",
      "--bundles",
      "msi",
      "--no-sign",
      "--ci",
      "--config",
      "src-tauri/tauri.updater.conf.json",
    ]);
  });

  it("adds the Python resources only to the explicit protected distribution mode", () => {
    expect(protectedPythonDistributionBuildArguments()).toEqual([
      "build",
      "--no-bundle",
      "--no-sign",
      "--ci",
      "--config",
      PYTHON_CONFIG,
      "--",
      "--locked",
    ]);
    expect(protectedPythonDistributionBundleArguments()).toEqual([
      "bundle",
      "--bundles",
      "msi",
      "--no-sign",
      "--ci",
      "--config",
      "src-tauri/tauri.updater.conf.json",
      "--config",
      PYTHON_CONFIG,
    ]);
  });

  it("signs the staged AppContainer controller and runner before protected packaging", () => {
    expect(protectedPythonDistributionSigningPlan("C:\\repo")).toEqual([
      {
        label: "protected Python AppContainer controller",
        path: "C:\\repo\\package\\python-development\\bottie-python-appcontainer-x86_64-pc-windows-msvc.exe",
      },
      {
        label: "protected Python runner",
        path: "C:\\repo\\package\\python-development\\bottie-python-runner-x86_64-pc-windows-msvc.exe",
      },
    ]);
  });

  it("signs and rebinds protected Python inputs before the product build", async () => {
    const script = await readFile(new URL("./windows-distribution.mjs", import.meta.url), "utf8");
    const protectedSigning = script.indexOf(
      "if (protectedPython) await signProtectedPythonCode(repositoryRoot, signToolPath, credentials)",
    );
    const productBuild = script.indexOf("buildWindowsBundle(repositoryRoot, buildArguments, targetDirectory)");
    const signingFunction = script.slice(
      script.indexOf("async function signProtectedPythonCode("),
      script.indexOf("/** Requires one regular file"),
    );

    expect(protectedSigning).toBeGreaterThan(-1);
    expect(protectedSigning).toBeLessThan(productBuild);
    expect(signingFunction).toContain("signAndVerify(signToolPath, credentials, step.path)");
    expect(signingFunction).toContain("signedPythonRunnerEvidence(evidence, runner)");
  });

  it("refreshes only the signed runner identity in the closed package marker", () => {
    const evidence = {
      manifestSha256: "a".repeat(64),
      runnerBytes: 7,
      runnerSha256: "b".repeat(64),
      runtime: { schemaVersion: 1 },
      schemaVersion: 1,
      target: "x86_64-pc-windows-msvc",
    };
    const signed = Buffer.from("signed-runner");

    expect(signedPythonRunnerEvidence(evidence, signed)).toEqual({
      ...evidence,
      runnerBytes: signed.length,
      runnerSha256: "1de7563f6f1d5847f3893c34d89eac91f125577e00e3df66ddaac013bb045941",
    });
    expect(() => signedPythonRunnerEvidence({ ...evidence, path: "C:\\private" }, signed)).toThrow(/closed/);
    expect(() => signedPythonRunnerEvidence(evidence, Buffer.alloc(0))).toThrow(/runner/);
  });

  it("uses SHA-256 Authenticode and RFC 3161 timestamps for each exact artifact", () => {
    const arguments_ = signToolSignArguments(CERTIFICATE_PATH, "protected-password", EXECUTABLE_PATH);

    expect(arguments_).toEqual([
      "sign",
      "/fd",
      "SHA256",
      "/tr",
      "http://timestamp.digicert.com",
      "/td",
      "SHA256",
      "/f",
      CERTIFICATE_PATH,
      "/p",
      "protected-password",
      EXECUTABLE_PATH,
    ]);
    expect(signToolSignArguments(CERTIFICATE_PATH, "protected-password", INSTALLER_PATH).at(-1)).toBe(INSTALLER_PATH);
  });

  it("verifies the executable and installer independently under Windows distribution policy", () => {
    expect(signToolVerifyArguments(EXECUTABLE_PATH)).toEqual(["verify", "/pa", "/all", "/v", EXECUTABLE_PATH]);
    expect(signToolVerifyArguments(INSTALLER_PATH)).toEqual(["verify", "/pa", "/all", "/v", INSTALLER_PATH]);
  });

  it("binds verified updater evidence to the exact Authenticode-signed MSI", async () => {
    const script = await readFile(new URL("./windows-distribution.mjs", import.meta.url), "utf8");

    expect(script).toContain('bindUpdaterArtifactEvidence(updater, "windows-x86_64", bundle.installer.sha256)');
    expect(script.indexOf("signAndVerify(signToolPath, credentials, msiPath)")).toBeLessThan(
      script.indexOf('bindUpdaterArtifactEvidence(updater, "windows-x86_64", bundle.installer.sha256)'),
    );
    expect(
      script.indexOf('bindUpdaterArtifactEvidence(updater, "windows-x86_64", bundle.installer.sha256)'),
    ).toBeLessThan(script.indexOf("await exportUpdaterArtifact("));
  });

  it("requires one complete protected credential pair outside the repository", () => {
    expect(
      resolveSigningCredentials(
        {
          BOTTIE_WINDOWS_SIGNING_CERTIFICATE_PATH: CERTIFICATE_PATH,
          BOTTIE_WINDOWS_SIGNING_CERTIFICATE_PASSWORD: "protected-password",
        },
        "C:\\repo",
      ),
    ).toEqual({ certificatePath: CERTIFICATE_PATH, password: "protected-password" });
    expect(() => resolveSigningCredentials({}, "C:\\repo")).toThrow(/credentials are unavailable/);
    expect(() =>
      resolveSigningCredentials(
        {
          BOTTIE_WINDOWS_SIGNING_CERTIFICATE_PATH: "C:\\repo\\private.pfx",
          BOTTIE_WINDOWS_SIGNING_CERTIFICATE_PASSWORD: "protected-password",
        },
        "C:\\repo",
      ),
    ).toThrow(/outside the repository/);
  });

  it("keeps protected CI manual, environment-gated, evidence-only, and self-cleaning", async () => {
    const workflow = await readFile(
      new URL("../.github/workflows/windows-distribution-validation.yml", import.meta.url),
      "utf8",
    );

    expect(workflow).toContain("workflow_dispatch:");
    expect(workflow).toContain("workflow_call:");
    expect(workflow).toContain("environment: windows-distribution");
    expect(workflow).toContain("BOTTIE_WINDOWS_SIGNING_PFX_BASE64");
    expect(workflow).toContain("BOTTIE_WINDOWS_SIGNING_CERTIFICATE_PASSWORD");
    expect(workflow).toContain("BOTTIE_UPDATER_SIGNING_PRIVATE_KEY");
    expect(workflow).toContain("BOTTIE_UPDATER_SIGNING_PRIVATE_KEY_PASSWORD");
    const signingStep = workflow.indexOf("- name: Sign, verify, inspect, and smoke-test Windows distribution");
    expect(workflow.slice(0, signingStep)).not.toContain("BOTTIE_UPDATER_SIGNING_PRIVATE_KEY");
    expect(workflow).toContain("package/windows-package-evidence.json");
    expect(workflow).toContain("if: always()");
    expect(workflow).not.toMatch(
      /environment: windows-distribution\n\s+env:\n\s+BOTTIE_WINDOWS_SIGNING_(?:PFX|CERTIFICATE)/,
    );
    expect(workflow).not.toMatch(/pull_request:|push:|release:/);
    expect(workflow).not.toMatch(/package\/windows\/.*\.msi/);
    expect(workflow).toContain("name: bottie-updater-windows");
    expect(workflow).toContain("retention-days: 1");
  });

  it("adds only an optional same-revision protected Python composition", async () => {
    const workflow = await readFile(
      new URL("../.github/workflows/windows-distribution-validation.yml", import.meta.url),
      "utf8",
    );
    const packageManifest = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
    const recreate = workflow.indexOf("- name: Recreate and inspect the protected Python MSI");
    const credentials = workflow.indexOf("- name: Prepare protected Windows signing material");
    const distribution = workflow.indexOf("- name: Sign and verify the protected Python distribution");
    const containment = workflow.indexOf("- name: Install and prove the protected Python MSI");

    expect(packageManifest.scripts["package:windows:distribution:python"]).toBe(
      "node scripts/windows-distribution.mjs --run-python",
    );
    expect(workflow).toContain("python_provenance_run_id:");
    expect(workflow).toContain("actions: read");
    expect(workflow).toContain(
      'gh api "repos/$env:GITHUB_REPOSITORY/actions/runs/$env:BOTTIE_PYTHON_PROVENANCE_RUN_ID"',
    );
    expect(workflow).toContain('$runRecord.name -ne "Python runtime provenance"');
    expect(workflow).toContain("$runRecord.head_sha -ne $env:GITHUB_SHA");
    expect(workflow).toContain('$runRecord.conclusion -ne "success"');
    expect(workflow).toContain("bottie-python-runtime-provenance");
    expect(workflow).toContain("bottie-python-release-candidate-evidence");
    expect(recreate).toBeGreaterThan(-1);
    expect(recreate).toBeLessThan(credentials);
    expect(workflow.slice(recreate, credentials)).not.toContain("secrets.");
    expect(distribution).toBeGreaterThan(credentials);
    expect(containment).toBeGreaterThan(distribution);
    expect(workflow.slice(recreate, credentials)).toContain("python:protected:inspect");
    expect(workflow.slice(distribution, containment)).toContain("package:windows:distribution:python");
    expect(workflow.slice(containment)).toContain("python:protected:windows:prove-shipping");
    expect(workflow.slice(containment)).toContain("python:protected:compare");
    expect(workflow).toContain("bottie-python-windows-protected-distribution-evidence");
    expect(workflow).toContain("if: inputs.python_provenance_run_id == ''");
    expect(workflow).not.toMatch(/pull_request:|push:|release:/);
  });
});
