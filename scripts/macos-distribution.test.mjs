import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import {
  classifyGatekeeperAssessment,
  distributionSigningArguments,
  macosUpdaterTarget,
  notarySubmitArguments,
  parseNotaryResult,
  resolveNotaryAuthentication,
  selectDeveloperIdApplicationIdentity,
  staplerArguments,
  updaterArchiveArguments,
} from "./macos-distribution.mjs";
import { inspectBundleFiles } from "./macos-package.mjs";
import { macosUpdaterBuildArguments } from "./macos-package.mjs";

const IDENTITIES = `
  1) AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA "Apple Development: Example One (TEAMONE)"
  2) BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB "Developer ID Application: Example One (TEAMONE)"
     2 valid identities found
`;

describe("macOS distribution signing and notarization", () => {
  it("creates updater artifacts only in the protected distribution build", () => {
    expect(macosUpdaterBuildArguments()).toContain("src-tauri/tauri.updater.conf.json");
    expect(updaterArchiveArguments("/bundle/bottie.app", "/bundle/bottie.app.tar.gz")).toEqual([
      "-czf",
      "/bundle/bottie.app.tar.gz",
      "-C",
      "/bundle",
      "bottie.app",
    ]);
  });

  it("maps only one exact native architecture into its Tauri updater target", () => {
    expect(macosUpdaterTarget(["arm64"])).toBe("darwin-aarch64");
    expect(macosUpdaterTarget(["x86_64"])).toBe("darwin-x86_64");
    expect(() => macosUpdaterTarget(["arm64", "x86_64"])).toThrow(/single supported architecture/);
    expect(() => macosUpdaterTarget(["i386"])).toThrow(/single supported architecture/);
  });

  it("retains cryptographically verified updater evidence for the final notarized archive", async () => {
    const script = await readFile(new URL("./macos-distribution.mjs", import.meta.url), "utf8");

    expect(script).toContain("bindUpdaterArtifactEvidence(updater, macosUpdaterTarget(architectures))");
    expect(script.indexOf("const notarization = notarizeAndVerify(")).toBeLessThan(
      script.indexOf("const updater = await createUpdaterArchive("),
    );
    expect(script.indexOf("const updater = await createUpdaterArchive(")).toBeLessThan(
      script.indexOf("const evidence = await createDistributionEvidence("),
    );
    expect(script.indexOf("const evidence = await createDistributionEvidence(")).toBeLessThan(
      script.indexOf("await exportUpdaterArtifact("),
    );
  });

  it("keeps production updater signing in the same protected manual environment", async () => {
    const workflow = await readFile(
      new URL("../.github/workflows/macos-distribution-validation.yml", import.meta.url),
      "utf8",
    );

    expect(workflow).toContain("environment: macos-distribution");
    expect(workflow).toContain("workflow_call:");
    expect(workflow).toContain("BOTTIE_UPDATER_SIGNING_PRIVATE_KEY");
    expect(workflow).toContain("BOTTIE_UPDATER_SIGNING_PRIVATE_KEY_PASSWORD");
    const signingStep = workflow.indexOf("- name: Sign, notarize, staple, and verify");
    expect(workflow.slice(0, signingStep)).not.toContain("BOTTIE_UPDATER_SIGNING_PRIVATE_KEY");
    expect(workflow).not.toMatch(/push:|pull_request:|release:/);
    expect(workflow).toContain("name: bottie-updater-macos");
    expect(workflow).toContain("retention-days: 1");
  });

  it("selects only a Developer ID Application identity and never returns its label", () => {
    expect(selectDeveloperIdApplicationIdentity(IDENTITIES)).toBe("BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB");
    expect(() => selectDeveloperIdApplicationIdentity("0 valid identities found")).toThrow(/Developer ID Application/);
  });

  it("requires an explicit match when multiple distribution identities are available", () => {
    const identities = `${IDENTITIES}
      3) CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC "Developer ID Application: Example Two (TEAMTWO)"`;

    expect(() => selectDeveloperIdApplicationIdentity(identities)).toThrow(/BOTTIE_APPLE_DISTRIBUTION_IDENTITY/);
    expect(selectDeveloperIdApplicationIdentity(identities, "c".repeat(40))).toBe("C".repeat(40));
  });

  it("uses hardened runtime, a secure timestamp, and the checked-in entitlements without deep signing", () => {
    const arguments_ = distributionSigningArguments(
      "B".repeat(40),
      "/repo/src-tauri/Entitlements.plist",
      "/repo/bottie.app",
    );

    expect(arguments_).toEqual([
      "--force",
      "--sign",
      "B".repeat(40),
      "--options",
      "runtime",
      "--timestamp",
      "--entitlements",
      "/repo/src-tauri/Entitlements.plist",
      "/repo/bottie.app",
    ]);
    expect(arguments_).not.toContain("--deep");
    expect(arguments_).not.toContain("--timestamp=none");
  });

  it("keeps the hardened-runtime entitlement policy minimal and credential-free", async () => {
    const entitlements = await readFile(new URL("../src-tauri/Entitlements.plist", import.meta.url), "utf8");

    expect(entitlements).toContain("<dict>");
    expect(entitlements).not.toMatch(/application-identifier|team-identifier|keychain-access-groups/);
    expect(entitlements).not.toMatch(/allow-unsigned-executable-memory|disable-library-validation|get-task-allow/);
  });

  it("binds Tauri's macOS bundle to the reviewed entitlement file", async () => {
    const config = JSON.parse(await readFile(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));

    expect(config.bundle.macOS.entitlements).toBe("Entitlements.plist");
    expect(config.bundle.macOS.hardenedRuntime).toBe(true);
    expect(config.identifier).toBe("com.bottie.app");
  });

  it("accepts either one host keychain profile or one complete protected API-key set", () => {
    expect(resolveNotaryAuthentication({ BOTTIE_APPLE_NOTARY_PROFILE: "bottie-notary" })).toEqual([
      "--keychain-profile",
      "bottie-notary",
    ]);
    expect(
      resolveNotaryAuthentication({
        BOTTIE_APPLE_NOTARY_KEY_PATH: "/private/key.p8",
        BOTTIE_APPLE_NOTARY_KEY_ID: "KEY123",
        BOTTIE_APPLE_NOTARY_ISSUER_ID: "issuer-uuid",
      }),
    ).toEqual(["--key", "/private/key.p8", "--key-id", "KEY123", "--issuer", "issuer-uuid"]);
    expect(() => resolveNotaryAuthentication({})).toThrow(/notary credentials/);
    expect(() =>
      resolveNotaryAuthentication({
        BOTTIE_APPLE_NOTARY_PROFILE: "profile",
        BOTTIE_APPLE_NOTARY_KEY_PATH: "/private/key.p8",
        BOTTIE_APPLE_NOTARY_KEY_ID: "KEY123",
        BOTTIE_APPLE_NOTARY_ISSUER_ID: "issuer-uuid",
      }),
    ).toThrow(/exactly one/);
    expect(() =>
      resolveNotaryAuthentication(
        {
          BOTTIE_APPLE_NOTARY_KEY_PATH: "/repo/private/key.p8",
          BOTTIE_APPLE_NOTARY_KEY_ID: "KEY123",
          BOTTIE_APPLE_NOTARY_ISSUER_ID: "issuer-uuid",
        },
        "/repo",
      ),
    ).toThrow(/outside the repository/);
  });

  it("submits one ZIP with bounded waiting and structured output", () => {
    expect(notarySubmitArguments("/tmp/bottie.zip", ["--keychain-profile", "profile"])).toEqual([
      "notarytool",
      "submit",
      "/tmp/bottie.zip",
      "--keychain-profile",
      "profile",
      "--wait",
      "--timeout",
      "30m",
      "--no-progress",
      "--output-format",
      "json",
    ]);
  });

  it("accepts only an Apple-accepted notarization result and returns identity-free evidence", () => {
    expect(
      parseNotaryResult('{"id":"submission-id","message":"Successfully uploaded file","status":"Accepted"}'),
    ).toEqual({ accepted: true, status: "accepted" });
    expect(() => parseNotaryResult('{"id":"submission-id","status":"Invalid"}')).toThrow(/not accepted/);
    expect(() => parseNotaryResult("not-json")).toThrow(/structured notarization result/);
  });

  it("staples and validates the exact application bundle", () => {
    expect(staplerArguments("staple", "/repo/bottie.app")).toEqual(["stapler", "staple", "-v", "/repo/bottie.app"]);
    expect(staplerArguments("validate", "/repo/bottie.app")).toEqual(["stapler", "validate", "-v", "/repo/bottie.app"]);
  });

  it("records Gatekeeper acceptance without retaining authority or host-path output", () => {
    expect(
      classifyGatekeeperAssessment(
        0,
        "accepted\nsource=Notarized Developer ID\norigin=Developer ID Application: Private",
      ),
    ).toEqual({ accepted: true, source: "notarized-developer-id" });
    expect(classifyGatekeeperAssessment(3, "rejected\nsource=Unnotarized Developer ID")).toEqual({
      accepted: false,
      source: "rejected",
    });
  });

  it("keeps protected CI validation manual, environment-gated, and evidence-only", async () => {
    const workflow = await readFile(
      new URL("../.github/workflows/macos-distribution-validation.yml", import.meta.url),
      "utf8",
    );

    expect(workflow).toContain("workflow_dispatch:");
    expect(workflow).toContain("environment: macos-distribution");
    expect(workflow).toContain("BOTTIE_APPLE_DEVELOPER_ID_P12_BASE64");
    expect(workflow).toContain("BOTTIE_APPLE_NOTARY_KEY_P8");
    expect(workflow).toContain("package/macos-distribution-evidence.json");
    expect(workflow).not.toMatch(/pull_request:|push:|release:/);
    expect(workflow).not.toMatch(/package\/macos\/.*\.(?:app|zip|dmg)/);
  });

  it("rejects host-absolute symlink targets before they can enter retained evidence", async () => {
    const temporaryDirectory = await mkdtemp(join(tmpdir(), "bottie-distribution-test-"));
    const bundle = join(temporaryDirectory, "bottie.app");
    const contents = join(bundle, "Contents");
    const macos = join(contents, "MacOS");
    const resources = join(contents, "Resources");
    try {
      await mkdir(macos, { recursive: true });
      await mkdir(resources, { recursive: true });
      await writeFile(join(contents, "Info.plist"), "plist");
      await writeFile(join(macos, "bottie"), "native executable");
      await writeFile(join(resources, "icon.icns"), "icon");
      await symlink("/Users/private/not-for-evidence", join(resources, "host-link"));

      await expect(inspectBundleFiles(bundle)).rejects.toThrow(/unsafe symbolic link/);
    } finally {
      await rm(temporaryDirectory, { recursive: true, force: true });
    }
  });
});
