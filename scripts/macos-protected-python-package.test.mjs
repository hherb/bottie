import { lstat, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import {
  credentialFreeMacosEnvironment,
  macosProtectedBuildArguments,
  stageMacosProtectedInputs,
} from "./macos-protected-python-package.mjs";

const TARGET = "aarch64-apple-darwin";

/** Creates the exact three development inputs permitted to cross into protected staging. */
async function createDevelopmentInputs(root) {
  await mkdir(join(root, "python-runtime", "lib"), { recursive: true });
  await writeFile(join(root, `bottie-python-runner-${TARGET}`), "runner");
  await writeFile(join(root, "python-runtime", "python.wasm"), "runtime");
  await writeFile(join(root, "python-runtime", "lib", "os.py"), "library");
  await writeFile(join(root, "python-runtime-evidence.json"), '{"schemaVersion":1}\n');
}

describe("macOS protected Python package producer", () => {
  it("builds only the opt-in unsigned protected app with locked inputs", () => {
    expect(macosProtectedBuildArguments()).toEqual([
      "build",
      "--bundles",
      "app",
      "--no-sign",
      "--ci",
      "--config",
      "src-tauri/tauri.updater.conf.json",
      "--config",
      "src-tauri/tauri.python-protected.macos.conf.json",
      "--",
      "--locked",
    ]);
  });

  it("removes signing and notarization credentials from every producer subprocess", () => {
    expect(
      credentialFreeMacosEnvironment({
        BOTTIE_APPLE_DISTRIBUTION_IDENTITY: "private",
        BOTTIE_APPLE_NOTARY_KEY_ID: "private",
        BOTTIE_EPHEMERAL_SIGNING_IDENTITY: "private",
        BOTTIE_P12_PASSWORD: "private",
        BOTTIE_UPDATER_SIGNING_PRIVATE_KEY: "private",
        PATH: "/usr/bin",
        TAURI_SIGNING_PRIVATE_KEY: "private",
      }),
    ).toEqual({ PATH: "/usr/bin" });
  });

  it("copies only the exact source runner and runtime into a fresh protected staging root", async () => {
    const temporary = await mkdtemp(join(tmpdir(), "bottie-macos-protected-stage-test-"));
    const source = join(temporary, "development");
    const output = join(temporary, "protected");
    try {
      await createDevelopmentInputs(source);
      await mkdir(output, { recursive: true });
      await writeFile(join(output, "stale-signed-client"), "must disappear");

      const staged = await stageMacosProtectedInputs(source, output, TARGET);

      expect(staged).toEqual({
        evidence: "python-runtime-evidence.json",
        runner: `bottie-python-runner-${TARGET}`,
        runtime: "python-runtime",
      });
      await expect(lstat(join(output, "stale-signed-client"))).rejects.toThrow();
      expect(await readFile(join(output, staged.runner), "utf8")).toBe("runner");
      expect(await readFile(join(output, staged.runtime, "lib", "os.py"), "utf8")).toBe("library");
    } finally {
      await rm(temporary, { recursive: true, force: true });
    }
  });

  it("rejects a symlinked runner before protected staging", async () => {
    const temporary = await mkdtemp(join(tmpdir(), "bottie-macos-protected-link-test-"));
    const source = join(temporary, "development");
    const output = join(temporary, "protected");
    try {
      await createDevelopmentInputs(source);
      await rm(join(source, `bottie-python-runner-${TARGET}`));
      await symlink("python-runtime/python.wasm", join(source, `bottie-python-runner-${TARGET}`));

      await expect(stageMacosProtectedInputs(source, output, TARGET)).rejects.toThrow(/regular file/);
    } finally {
      await rm(temporary, { recursive: true, force: true });
    }
  });

  it("rejects unsupported targets and links nested inside the runtime", async () => {
    const temporary = await mkdtemp(join(tmpdir(), "bottie-macos-protected-runtime-link-test-"));
    const source = join(temporary, "development");
    const output = join(temporary, "protected");
    try {
      await createDevelopmentInputs(source);
      await expect(stageMacosProtectedInputs(source, output, "../escape")).rejects.toThrow(/target/);
      await symlink("../python.wasm", join(source, "python-runtime", "lib", "linked.py"));
      await expect(stageMacosProtectedInputs(source, output, TARGET)).rejects.toThrow(/ordinary files/);
    } finally {
      await rm(temporary, { recursive: true, force: true });
    }
  });

  it("registers only local producer commands and the dedicated overlay", async () => {
    const packageManifest = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
    const overlay = JSON.parse(
      await readFile(new URL("../src-tauri/tauri.python-protected.macos.conf.json", import.meta.url), "utf8"),
    );

    expect(packageManifest.scripts["python:protected:macos:produce"]).toBe(
      "node scripts/macos-protected-python-package.mjs --produce",
    );
    expect(overlay.bundle.macOS.files).toEqual({
      "Helpers/BottiePythonXPCClient.app": "../package/python-protected/BottiePythonXPCClient.app",
    });
    expect(overlay.bundle.resources["../package/python-protected/python-runtime-evidence.json"]).toBe(
      "python-runtime-evidence.json",
    );
    expect(JSON.stringify(overlay)).not.toMatch(/development|identity|credential|sign|notar/);
  });

  it("produces and uploads the inspection only after the accepted candidate exists", async () => {
    const workflow = await readFile(
      new URL("../.github/workflows/python-runtime-provenance.yml", import.meta.url),
      "utf8",
    );
    const job = workflow.indexOf("  produce-macos-protected-inspection:");
    const protectedJob = workflow.slice(job);

    expect(job).toBeGreaterThan(workflow.indexOf("  bind-evidence:"));
    expect(protectedJob).toContain("needs: [build-runtime, bind-evidence]");
    expect(protectedJob).toContain("runs-on: macos-15");
    expect(protectedJob).toContain("name: bottie-python-runtime-provenance");
    expect(protectedJob).toContain("name: bottie-python-release-candidate-evidence");
    expect(protectedJob).toContain("python:protected:macos:produce");
    expect(protectedJob).toContain("package/python-protected-evidence/macos.json");
    expect(protectedJob).not.toMatch(/secrets\.|environment:|codesign|notary|security add-trusted-cert/);
  });
});
