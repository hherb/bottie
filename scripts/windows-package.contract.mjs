import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, it } from "node:test";

import {
  classifyAuthenticodeStatus,
  combineWindowsPackageEvidence,
  embeddedIconPowerShellScript,
  inspectExtractedWindowsBundle,
  msiAdministrativeInstallArguments,
  offlineProviderSettings,
  parseEmbeddedIconDimensions,
  windowsBuildArguments,
  windowsSmokeBuildArguments,
  versionedPackageEvidence,
} from "./windows-package.mjs";

const temporaryDirectories = [];

afterEach(async () => {
  await Promise.all(temporaryDirectories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })));
});

/** Creates a minimal administratively extracted MSI fixture. */
async function createExtractedBundleFixture() {
  const directory = await mkdtemp(join(tmpdir(), "bottie-windows-package-test-"));
  temporaryDirectories.push(directory);
  await mkdir(join(directory, "PFiles", "bottie"), { recursive: true });
  await writeFile(join(directory, "PFiles", "bottie", "bottie.exe"), "native executable");
  await writeFile(join(directory, "PFiles", "bottie", "onnxruntime.dll"), "native runtime");
  await writeFile(join(directory, "PFiles", "bottie", "LICENSE"), "project licence");
  await writeFile(join(directory, "PFiles", "bottie", "MODEL-NOTICE.txt"), "model notice");
  await writeFile(join(directory, "PFiles", "bottie", "THIRD-PARTY-NOTICES.txt"), "third-party notices");
  return directory;
}

describe("Windows package evidence", () => {
  it("combines the real Bottie package inspection with only the isolated smoke outcome", () => {
    const bundle = { payload: { applicationDirectory: "PFiles/bottie" } };
    const smoke = { terminated: true };

    assert.deepEqual(combineWindowsPackageEvidence(bundle, smoke), { bundle, smoke });
    assert.throws(
      () =>
        combineWindowsPackageEvidence({ payload: { applicationDirectory: "PFiles/bottie-packaging-smoke" } }, smoke),
      /real Bottie package/,
    );
  });

  it("binds retained evidence to the checked-out release version and schema", () => {
    assert.deepEqual(versionedPackageEvidence("0.9.0", { bundle: {}, smoke: {} }), {
      schemaVersion: 1,
      version: "0.9.0",
      bundle: {},
      smoke: {},
    });
  });

  it("keeps the build locked, MSI-only, unsigned, and non-interactive", () => {
    assert.deepEqual(windowsBuildArguments(), ["build", "--bundles", "msi", "--no-sign", "--ci", "--", "--locked"]);
  });

  it("builds smoke code under a distinct application identity without changing dependency resolution", () => {
    assert.deepEqual(windowsSmokeBuildArguments(), [
      "build",
      "--bundles",
      "msi",
      "--no-sign",
      "--ci",
      "--config",
      JSON.stringify({ identifier: "com.bottie.packaging-smoke", productName: "bottie-packaging-smoke" }),
      "--",
      "--locked",
    ]);
  });

  it("administratively extracts an MSI without installing it", () => {
    assert.deepEqual(msiAdministrativeInstallArguments("C:\\package\\bottie.msi", "C:\\temp\\extract"), [
      "/a",
      "C:\\package\\bottie.msi",
      "/qn",
      "/norestart",
      "TARGETDIR=C:\\temp\\extract",
    ]);
  });

  it("inventories required payload files and native runtimes using relative paths", async () => {
    const bundle = await createExtractedBundleFixture();

    const inspection = await inspectExtractedWindowsBundle(bundle);

    assert.equal(inspection.applicationDirectory, "PFiles/bottie");
    assert.equal(inspection.executable, "bottie.exe");
    assert.deepEqual(inspection.nativeRuntimeAssets, ["onnxruntime.dll"]);
    assert.deepEqual(Object.keys(inspection.requiredDocuments), ["licence", "modelNotice", "thirdPartyNotices"]);
    assert.deepEqual(
      inspection.files.map((file) => file.path),
      ["bottie.exe", "LICENSE", "MODEL-NOTICE.txt", "onnxruntime.dll", "THIRD-PARTY-NOTICES.txt"],
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
    await mkdir(join(bundle, "duplicate"));
    await writeFile(join(bundle, "duplicate", "bottie.exe"), "duplicate executable");

    await assert.rejects(inspectExtractedWindowsBundle(bundle), /exactly one Bottie executable/);
  });

  it("classifies unsigned, valid, and untrusted Authenticode states without retaining signer identities", () => {
    assert.equal(classifyAuthenticodeStatus("NotSigned"), "unsigned");
    assert.equal(classifyAuthenticodeStatus("Valid"), "identified");
    assert.equal(classifyAuthenticodeStatus("UnknownError"), "untrusted");
  });

  it("extracts only public embedded-icon dimensions from the installed executable", () => {
    const script = embeddedIconPowerShellScript();

    assert.match(script, /ExtractAssociatedIcon/);
    assert.match(script, /BOTTIE_WINDOWS_INSPECT_PATH/);
    assert.match(script, /Width/);
    assert.match(script, /Height/);
    assert.doesNotMatch(script, /Write-Output.*INSPECT_PATH/);
    assert.deepEqual(parseEmbeddedIconDimensions("32x32"), { height: 32, width: 32 });
    assert.throws(() => parseEmbeddedIconDimensions("0x32"), /invalid embedded-icon evidence/);
    assert.throws(() => parseEmbeddedIconDimensions("path 32x32"), /invalid embedded-icon evidence/);
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
