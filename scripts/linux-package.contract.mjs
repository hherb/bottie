import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, it } from "node:test";

import {
  inspectExtractedLinuxBundle,
  linuxBuildArguments,
  linuxSmokeBuildArguments,
  offlineProviderSettings,
  smokeXdgDirectories,
} from "./linux-package.mjs";

const temporaryDirectories = [];

afterEach(async () => {
  await Promise.all(temporaryDirectories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })));
});

/** Creates a minimal extracted DEB payload with one Bottie executable and one native runtime. */
async function createExtractedBundleFixture() {
  const directory = await mkdtemp(join(tmpdir(), "bottie-linux-package-test-"));
  temporaryDirectories.push(directory);
  await mkdir(join(directory, "usr", "bin"), { recursive: true });
  await mkdir(join(directory, "usr", "lib", "bottie"), { recursive: true });
  await mkdir(join(directory, "usr", "share", "applications"), { recursive: true });
  await writeFile(join(directory, "usr", "bin", "bottie"), minimalElf("x86_64"));
  await writeFile(join(directory, "usr", "lib", "bottie", "libonnxruntime.so.1"), "native runtime");
  await writeFile(join(directory, "usr", "share", "applications", "bottie.desktop"), "[Desktop Entry]\n");
  return directory;
}

/** Returns a minimal ELF header with the requested machine classification. */
function minimalElf(architecture) {
  const bytes = Buffer.alloc(64);
  bytes.set([0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01, 0x01]);
  bytes.writeUInt16LE(architecture === "aarch64" ? 0xb7 : 0x3e, 18);
  return bytes;
}

describe("Linux package evidence", () => {
  it("keeps the build locked, DEB-only, unsigned, and non-interactive", () => {
    assert.deepEqual(linuxBuildArguments(), ["build", "--bundles", "deb", "--no-sign", "--ci", "--", "--locked"]);
  });

  it("builds smoke code under a distinct application identity without changing dependency resolution", () => {
    assert.deepEqual(linuxSmokeBuildArguments(), [
      "build",
      "--bundles",
      "deb",
      "--no-sign",
      "--ci",
      "--config",
      JSON.stringify({ identifier: "com.bottie.packaging-smoke", productName: "bottie-packaging-smoke" }),
      "--",
      "--locked",
    ]);
  });

  it("inventories required payload files and native runtimes using relative paths", async () => {
    const bundle = await createExtractedBundleFixture();

    const inspection = await inspectExtractedLinuxBundle(bundle);

    assert.equal(inspection.executable, "usr/bin/bottie");
    assert.equal(inspection.architecture, "x86_64");
    assert.deepEqual(inspection.nativeRuntimeAssets, ["usr/lib/bottie/libonnxruntime.so.1"]);
    assert.deepEqual(
      inspection.files.map((file) => file.path),
      ["usr/bin/bottie", "usr/lib/bottie/libonnxruntime.so.1", "usr/share/applications/bottie.desktop"],
    );
    assert.equal(
      inspection.files.some((file) => file.path.includes(bundle)),
      false,
    );
    assert.equal(
      inspection.files.every((file) => file.sha256.length === 64),
      true,
    );
  });

  it("rejects extracted payloads without exactly one Bottie executable", async () => {
    const bundle = await createExtractedBundleFixture();
    await mkdir(join(bundle, "opt", "bottie"), { recursive: true });
    await writeFile(join(bundle, "opt", "bottie", "bottie"), minimalElf("aarch64"));

    await assert.rejects(inspectExtractedLinuxBundle(bundle), /exactly one Bottie executable/);
  });

  it("creates separate process-owned XDG roots and a distinct application identity", () => {
    assert.deepEqual(smokeXdgDirectories("/tmp/bottie-smoke"), {
      cache: "/tmp/bottie-smoke/cache",
      config: "/tmp/bottie-smoke/config",
      data: "/tmp/bottie-smoke/data",
      runtime: "/tmp/bottie-smoke/runtime",
      support: "/tmp/bottie-smoke/data/com.bottie.packaging-smoke",
      settings: "/tmp/bottie-smoke/config/com.bottie.packaging-smoke/providers.json",
    });
  });

  it("creates completed local-provider settings that can reach only the isolated endpoint", () => {
    assert.deepEqual(offlineProviderSettings(43127), {
      omlxBaseUrl: "http://127.0.0.1:43127/",
      ollamaBaseUrl: "http://127.0.0.1:43127/",
      setupCompleted: true,
      lastProviderId: "omlx",
      lastModelId: "packaging-offline-smoke",
    });
  });
});
