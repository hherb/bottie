import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, symlink, truncate, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import {
  inspectPackagedPythonBundle,
  inspectPythonRuntime,
  preparePythonBundleInputs,
  sidecarSourceName,
  stageBuiltPythonRuntime,
  validateRuntimeManifest,
} from "./python-runtime-bundle.mjs";

const TARGET = "x86_64-unknown-linux-gnu";
const SHA256_PATTERN = /^[a-f0-9]{64}$/;
const REPOSITORY_ROOT = join(import.meta.dirname, "..");

/** Returns one deterministic digest for fixture bytes. */
function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

/** Builds the smallest valid provenance manifest used by focused tests. */
function manifest(licence = "CPython licence\n") {
  return {
    schemaVersion: 2,
    pythonVersion: "3.14.7",
    wasiSdkVersion: "24",
    wasmtimeVersion: "45.0.3",
    source: {
      url: "https://www.python.org/ftp/python/3.14.7/Python-3.14.7.tar.xz",
      bytes: 24_053_924,
      sha256: "a".repeat(64),
      licenceSha256: sha256(licence),
      sbom: {
        url: "https://www.python.org/ftp/python/3.14.7/Python-3.14.7.tar.xz.spdx.json",
        bytes: 1,
        sha256: "b".repeat(64),
      },
    },
    buildTools: {
      wasiSdkLinuxX64: {
        url:
          "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-24/" +
          "wasi-sdk-24.0-x86_64-linux.tar.gz",
        bytes: 2,
        sha256: "c".repeat(64),
      },
      wasmtimeLinuxX64: {
        url:
          "https://github.com/bytecodealliance/wasmtime/releases/download/v45.0.3/" +
          "wasmtime-v45.0.3-x86_64-linux.tar.xz",
        bytes: 3,
        sha256: "d".repeat(64),
      },
    },
    build: {
      command: ["python3", "Tools/wasm/wasi", "build"],
      sourceDateEpoch: 1_785_888_000,
      target: "wasm32-wasip1",
      strippedPaths: [
        "curses",
        "ctypes/test",
        "ensurepip",
        "distutils",
        "lib2to3",
        "idlelib",
        "test",
        "multiprocessing",
        "tkinter",
        "turtledemo",
        "venv",
        "unittest/test",
      ],
    },
    archive: {
      url:
        "https://github.com/brettcannon/cpython-wasi-build/releases/download/v3.14.7/" +
        "python-3.14.7-wasi_sdk-24.zip",
      bytes: 4,
      sha256: "e".repeat(64),
      role: "compatibility-test-input",
    },
    requiredPaths: ["python.wasm", "lib/python3.14/os.py", "LICENSE"],
  };
}

/** Creates a tiny runtime with the same required shape as the real payload. */
async function createRuntime(root, licence = "CPython licence\n") {
  await mkdir(join(root, "lib", "python3.14"), { recursive: true });
  await writeFile(join(root, "python.wasm"), "wasm");
  await writeFile(join(root, "lib", "python3.14", "os.py"), "name = 'posix'\n");
  await writeFile(join(root, "LICENSE"), licence);
}

describe("CPython/WASI development bundle", () => {
  it("keeps Python bundling opt-in and the hosted proof credential-free", async () => {
    const config = JSON.parse(
      await readFile(join(REPOSITORY_ROOT, "src-tauri", "tauri.python-development.conf.json"), "utf8"),
    );
    const workflow = await readFile(
      join(REPOSITORY_ROOT, ".github", "workflows", "python-runtime-provenance.yml"),
      "utf8",
    );

    expect(config.bundle.externalBin).toEqual(["../package/python-development/bottie-python-runner"]);
    expect(config.bundle.macOS.minimumSystemVersion).toBe("10.15");
    expect(config.bundle.resources).toMatchObject({
      "../package/python-development/python-runtime": "python-runtime",
    });
    expect(workflow).toContain("pull_request:");
    expect(workflow).toContain("scripts/python-runtime-bundle.mjs");
    expect(workflow).toContain("--stage-built");
    expect(workflow).toContain("--inspect-package");
    expect(workflow).toContain('MACOSX_DEPLOYMENT_TARGET: "10.15"');
    expect(workflow).toContain('CXXFLAGS: "-mmacosx-version-min=10.15"');
    expect(workflow).not.toContain("secrets.");
    expect(workflow).not.toContain("Microsoft Store");
  });

  it("requires official source provenance and exact build-tool inputs", () => {
    expect(validateRuntimeManifest(manifest())).toMatchObject({
      pythonVersion: "3.14.7",
      wasiSdkVersion: "24",
      wasmtimeVersion: "45.0.3",
    });

    expect(() =>
      validateRuntimeManifest({
        ...manifest(),
        source: { ...manifest().source, url: "https://example.com/Python.tar.xz" },
      }),
    ).toThrow(/official Python source/);
  });

  it("records only path-free runtime content evidence and rejects symlinks", async () => {
    const root = await mkdtemp(join(tmpdir(), "bottie-python-runtime-test-"));
    await createRuntime(root);

    const evidence = await inspectPythonRuntime(root, manifest());
    expect(evidence).toMatchObject({ fileCount: 3, pythonVersion: "3.14.7", totalBytes: 35 });
    expect(evidence.runtimeTreeSha256).toMatch(SHA256_PATTERN);
    expect(evidence.pythonWasmSha256).toBe(sha256("wasm"));
    expect(JSON.stringify(evidence)).not.toContain(root);

    await symlink("os.py", join(root, "lib", "python3.14", "alias.py"));
    await expect(inspectPythonRuntime(root, manifest())).rejects.toThrow(/symbolic link/);
  });

  it("rejects an oversized runtime before reading its payload", async () => {
    const root = await mkdtemp(join(tmpdir(), "bottie-python-runtime-large-test-"));
    await createRuntime(root);
    await truncate(join(root, "python.wasm"), 128 * 1024 * 1024 + 1);

    await expect(inspectPythonRuntime(root, manifest())).rejects.toThrow(/byte count exceeds/);
  });

  it("stages the reviewed CPython layout from one completed official build", async () => {
    const root = await mkdtemp(join(tmpdir(), "bottie-python-build-test-"));
    const source = join(root, "Python-3.14.7");
    const output = join(root, "runtime");
    await mkdir(join(source, "Lib", "test"), { recursive: true });
    await mkdir(join(source, "cross-build", "wasm32-wasip1", "build", "lib.fixture"), { recursive: true });
    await writeFile(join(source, "Lib", "os.py"), "name = 'posix'\n");
    await writeFile(join(source, "Lib", "test", "test_os.py"), "removed\n");
    await writeFile(join(source, "LICENSE"), "CPython licence\n");
    await writeFile(join(source, "cross-build", "wasm32-wasip1", "python.wasm"), "wasm");
    await writeFile(
      join(source, "cross-build", "wasm32-wasip1", "build", "lib.fixture", "_sysconfigdata_test.py"),
      "build_time_vars = {}\n",
    );

    const evidence = await stageBuiltPythonRuntime(source, output, manifest());
    expect(evidence.fileCount).toBe(4);
    await expect(readFile(join(output, "lib", "python3.14", "test", "test_os.py"))).rejects.toThrow();
    expect(await readFile(join(output, "lib", "python3.14", "_sysconfigdata_test.py"), "utf8")).toContain(
      "build_time_vars",
    );
  });

  it("prepares target-suffixed sidecar inputs and verifies the packaged payload", async () => {
    const root = await mkdtemp(join(tmpdir(), "bottie-python-bundle-test-"));
    const runtime = join(root, "runtime-source");
    const runner = join(root, "bottie-python-runner");
    const inputs = join(root, "inputs");
    await createRuntime(runtime);
    await writeFile(runner, "runner");

    const evidence = await preparePythonBundleInputs({
      manifest: manifest(),
      outputRoot: inputs,
      runnerPath: runner,
      runtimeRoot: runtime,
      target: TARGET,
    });
    expect(sidecarSourceName(TARGET)).toBe(`bottie-python-runner-${TARGET}`);
    expect(evidence).toMatchObject({ runnerSha256: sha256("runner"), target: TARGET });

    const packageRoot = join(root, "package");
    await mkdir(join(packageRoot, "usr", "lib"), { recursive: true });
    await mkdir(join(packageRoot, "usr", "bin"), { recursive: true });
    await preparePythonBundleInputs({
      manifest: manifest(),
      outputRoot: join(packageRoot, "usr", "lib", "bottie"),
      runnerPath: runner,
      runtimeRoot: runtime,
      target: TARGET,
    });
    await writeFile(join(packageRoot, "usr", "bin", "bottie-python-runner"), "runner");

    const inspection = await inspectPackagedPythonBundle(packageRoot, "linux", manifest());
    expect(inspection).toMatchObject({ bundled: true, target: TARGET, runtime: { fileCount: 3 } });
    expect(JSON.stringify(inspection)).not.toContain(packageRoot);

    const evidencePath = join(packageRoot, "usr", "lib", "bottie", "python-runtime-evidence.json");
    const tamperedEvidence = JSON.parse(await readFile(evidencePath, "utf8"));
    tamperedEvidence.target = "x86_64-pc-windows-msvc";
    await writeFile(evidencePath, `${JSON.stringify(tamperedEvidence)}\n`);
    await expect(inspectPackagedPythonBundle(packageRoot, "linux", manifest())).rejects.toThrow(/manifest or platform/);
  });
});
