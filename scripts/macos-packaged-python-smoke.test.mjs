import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import { credentialFreeSigningPlan, packagedBundleLayout } from "./macos-packaged-python-smoke.mjs";

const REPOSITORY_ROOT = import.meta.dirname.replace(/\/scripts$/, "");
const EPHEMERAL_IDENTITY = "A".repeat(40);
const EPHEMERAL_KEYCHAIN = "/tmp/bottie-python-signing.keychain-db";

describe("packaged macOS Python XPC smoke", () => {
  it("uses only the exact client and nested service resources from the packaged app", () => {
    expect(packagedBundleLayout("/tmp/bottie.app")).toEqual({
      application: "/tmp/bottie.app/Contents/Helpers/BottiePythonXPCClient.app",
      applicationExecutable:
        "/tmp/bottie.app/Contents/Helpers/BottiePythonXPCClient.app/Contents/MacOS/bottie-python-xpc-client",
      runner:
        "/tmp/bottie.app/Contents/Helpers/BottiePythonXPCClient.app/Contents/XPCServices/com.bottie.python-runner.xpc/Contents/Helpers/bottie-python-runner",
      runtime:
        "/tmp/bottie.app/Contents/Helpers/BottiePythonXPCClient.app/Contents/XPCServices/com.bottie.python-runner.xpc/Contents/Resources/python-runtime",
      service:
        "/tmp/bottie.app/Contents/Helpers/BottiePythonXPCClient.app/Contents/XPCServices/com.bottie.python-runner.xpc",
      serviceExecutable:
        "/tmp/bottie.app/Contents/Helpers/BottiePythonXPCClient.app/Contents/XPCServices/com.bottie.python-runner.xpc/Contents/MacOS/bottie-python-xpc-service",
    });
  });

  it("ephemerally signs exact packaged nested code inside out without Apple credentials or recursive signing", () => {
    const layout = packagedBundleLayout("/tmp/bottie.app");
    const plan = credentialFreeSigningPlan("/repo", layout, EPHEMERAL_IDENTITY, EPHEMERAL_KEYCHAIN);

    expect(plan.map((step) => step.path)).toEqual([layout.runner, layout.service, layout.application]);
    for (const step of plan) {
      expect(step.arguments).toContain("--sign");
      expect(step.arguments[step.arguments.indexOf("--sign") + 1]).toBe(EPHEMERAL_IDENTITY);
      expect(step.arguments).toContain("--keychain");
      expect(step.arguments[step.arguments.indexOf("--keychain") + 1]).toBe(EPHEMERAL_KEYCHAIN);
      expect(step.arguments).not.toContain("--deep");
      expect(step.arguments).not.toContain("--timestamp");
    }
    expect(plan[0].arguments).toContain("/repo/macos-python-xpc/Runner.entitlements");
    expect(plan[1].arguments).toContain("/repo/macos-python-xpc/Service.entitlements");
  });

  it("runs the packaged proof after build and uploads only path-free macOS evidence", async () => {
    const workflow = await readFile(
      new URL("../.github/workflows/python-runtime-provenance.yml", import.meta.url),
      "utf8",
    );
    const packageJson = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
    const build = workflow.indexOf("- name: Build the unsigned macOS development package");
    const signing = workflow.indexOf("- name: Create an ephemeral macOS development-signing identity");
    const proof = workflow.indexOf("- name: Inspect and prove the macOS development bundle");
    const cleanup = workflow.indexOf("- name: Remove the ephemeral macOS development-signing identity");

    expect(packageJson.scripts["python:xpc:prove-packaged"]).toBe(
      "node scripts/macos-packaged-python-smoke.mjs --prove",
    );
    expect(build).toBeGreaterThan(-1);
    expect(signing).toBeGreaterThan(-1);
    expect(build).toBeGreaterThan(signing);
    expect(proof).toBeGreaterThan(build);
    expect(cleanup).toBeGreaterThan(proof);
    expect(workflow.slice(cleanup)).toContain("if: always() && runner.os == 'macOS'");
    expect(workflow.slice(proof)).toContain("python:xpc:prove-packaged");
    expect(workflow.slice(proof)).toContain("macos.json");
    expect(workflow.slice(proof)).toContain("macos-containment.json");
    expect(workflow).toContain("openssl rand -hex 24");
    expect(workflow).toContain("umask 077");
    expect(workflow).toContain("-passout env:BOTTIE_P12_PASSWORD");
    expect(workflow).toContain("pkcs12_options=()");
    expect(workflow).toContain('if [[ "$(openssl pkcs12 -help 2>&1 || true)" == *"-legacy"* ]]; then');
    expect(workflow).toContain("pkcs12_options=(-legacy)");
    expect(workflow).toContain('openssl pkcs12 -export "${pkcs12_options[@]}"');
    expect(workflow).not.toContain("openssl pkcs12 -export -legacy");
    expect(workflow).toContain("sudo security add-trusted-cert -d -r trustRoot -p codeSign");
    expect(workflow.slice(cleanup)).toContain("sudo security remove-trusted-cert -d");
    expect(workflow).not.toContain("-passout pass:");
    expect(workflow).not.toContain("secrets.");
  });
});
