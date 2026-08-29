import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import {
  distributionBuildArguments,
  distributionBundleArguments,
  resolveSigningCredentials,
  signToolSignArguments,
  signToolVerifyArguments,
} from "./windows-distribution.mjs";

const CERTIFICATE_PATH = "C:\\runner-temp\\bottie-signing.pfx";
const EXECUTABLE_PATH = "C:\\target\\release\\bottie.exe";
const INSTALLER_PATH = "C:\\target\\release\\bundle\\msi\\bottie.msi";

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
  });
});
