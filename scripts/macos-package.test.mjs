import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import {
  classifyCodeSignature,
  developmentExecutableSigningArguments,
  developmentSigningArguments,
  inspectBundleFiles,
  macosBuildArguments,
  macosSmokeBuildArguments,
  offlineProviderSettings,
  sqliteImmutableUri,
} from "./macos-package.mjs";

const temporaryDirectories = [];

afterEach(async () => {
  await Promise.all(temporaryDirectories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })));
});

/** Creates a minimal application bundle fixture for path-safe inspection tests. */
async function createBundleFixture() {
  const directory = await mkdtemp(join(tmpdir(), "bottie-package-test-"));
  temporaryDirectories.push(directory);
  const bundle = join(directory, "bottie.app");
  await mkdir(join(bundle, "Contents", "MacOS"), { recursive: true });
  await mkdir(join(bundle, "Contents", "Resources", "_up_", "immutable"), { recursive: true });
  await mkdir(join(bundle, "Contents", "Frameworks"), { recursive: true });
  await writeFile(join(bundle, "Contents", "Info.plist"), "plist");
  await writeFile(join(bundle, "Contents", "MacOS", "bottie"), "native executable");
  await writeFile(join(bundle, "Contents", "Resources", "icon.icns"), "icon");
  await writeFile(join(bundle, "Contents", "Resources", "_up_", "immutable", "app.js"), "frontend");
  await writeFile(join(bundle, "Contents", "Frameworks", "libonnxruntime.dylib"), "runtime");
  return bundle;
}

describe("macOS package evidence", () => {
  it("keeps the build locked, app-only, unsigned, and non-interactive", () => {
    expect(macosBuildArguments()).toEqual(["build", "--bundles", "app", "--no-sign", "--ci", "--", "--locked"]);
  });

  it("builds smoke code under a distinct application identity without changing dependency resolution", () => {
    expect(macosSmokeBuildArguments()).toEqual([
      "build",
      "--bundles",
      "app",
      "--no-sign",
      "--ci",
      "--config",
      JSON.stringify({ identifier: "com.bottie.packaging-smoke", productName: "bottie-packaging-smoke" }),
      "--",
      "--locked",
    ]);
  });

  it("inventories required bundle files and native runtime assets with relative paths", async () => {
    const bundle = await createBundleFixture();

    const inspection = await inspectBundleFiles(bundle);

    expect(inspection.requiredEntries).toEqual({ executable: true, icon: true, infoPlist: true });
    expect(inspection.frontendAssetCount).toBe(1);
    expect(inspection.nativeRuntimeAssets).toEqual(["Contents/Frameworks/libonnxruntime.dylib"]);
    expect(inspection.files.map((file) => file.path)).not.toContain(expect.stringContaining(bundle));
    expect(inspection.files.every((file) => file.sha256.length === 64)).toBe(true);
  });

  it("classifies unsigned, ad-hoc, and identified signatures without retaining identities", () => {
    expect(classifyCodeSignature("code object is not signed at all")).toBe("unsigned");
    expect(classifyCodeSignature("Identifier=com.bottie.app\nSignature=adhoc")).toBe("ad-hoc");
    expect(classifyCodeSignature("Authority=Apple Development: Private Name\nTeamIdentifier=ABC123")).toBe(
      "identified",
    );
  });

  it("development-signs the bundle without timestamping or a recursive signing escape hatch", () => {
    const arguments_ = developmentSigningArguments("A".repeat(40), "/tmp/bottie.app");

    expect(arguments_).toEqual([
      "--force",
      "--sign",
      "A".repeat(40),
      "--options",
      "runtime",
      "--pagesize",
      "4096",
      "--timestamp=none",
      "/tmp/bottie.app",
    ]);
    expect(arguments_).not.toContain("--deep");
  });

  it("reuses the proven development-runner identity for the exact packaged executable", () => {
    expect(developmentExecutableSigningArguments("A".repeat(40), "/tmp/bottie")).toEqual([
      "--force",
      "--sign",
      "A".repeat(40),
      "--identifier",
      "com.bottie.app.dev",
      "--options",
      "runtime",
      "--pagesize",
      "4096",
      "--timestamp=none",
      "/tmp/bottie",
    ]);
  });

  it("creates a completed local-provider setup that can only reach the isolated endpoint", () => {
    expect(offlineProviderSettings(43127)).toEqual({
      omlxBaseUrl: "http://127.0.0.1:43127/",
      ollamaBaseUrl: "http://127.0.0.1:43127/",
      setupCompleted: true,
      lastProviderId: "omlx",
      lastModelId: "packaging-offline-smoke",
    });
  });

  it("opens smoke databases through encoded immutable SQLite URIs", () => {
    expect(sqliteImmutableUri("/Users/example/Library/Application Support/com.bottie.smoke/bottie.sqlite3")).toBe(
      "file:///Users/example/Library/Application%20Support/com.bottie.smoke/bottie.sqlite3?immutable=1",
    );
  });
});
