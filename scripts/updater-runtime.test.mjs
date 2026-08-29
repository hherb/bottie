import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

describe("production updater runtime contract", () => {
  it("embeds only a generated public key and keeps updater artifacts out of ordinary builds", async () => {
    const baseConfig = JSON.parse(await readFile(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
    const updaterConfig = JSON.parse(
      await readFile(new URL("../src-tauri/tauri.updater.conf.json", import.meta.url), "utf8"),
    );
    const encodedPublicKey = (
      await readFile(new URL("../distribution/update/bottie-updater.pub", import.meta.url), "utf8")
    ).trim();
    const decodedPublicKey = Buffer.from(encodedPublicKey, "base64").toString("utf8");

    expect(baseConfig.bundle.createUpdaterArtifacts).toBeUndefined();
    expect(baseConfig.plugins.updater).toEqual({ pubkey: "" });
    expect(updaterConfig.bundle.createUpdaterArtifacts).toBe(true);
    expect(decodedPublicKey).toMatch(/^untrusted comment: minisign public key: [A-F0-9]{16}\nRW/);
    expect(decodedPublicKey).not.toMatch(/PRIVATE KEY/i);
  });

  it("keeps the endpoint and updater API native-only", async () => {
    const [nativeUpdater, capability, packageMetadata] = await Promise.all([
      readFile(new URL("../src-tauri/src/updater.rs", import.meta.url), "utf8"),
      readFile(new URL("../src-tauri/capabilities/default.json", import.meta.url), "utf8"),
      readFile(new URL("../package.json", import.meta.url), "utf8"),
    ]);

    expect(nativeUpdater).toContain("https://github.com/hherb/bottie/releases/latest/download/latest.json");
    expect(nativeUpdater).toContain('include_str!("../../distribution/update/bottie-updater.pub")');
    expect(nativeUpdater).not.toContain("dangerousInsecureTransportProtocol");
    expect(capability).not.toContain("updater:");
    expect(packageMetadata).not.toContain("@tauri-apps/plugin-updater");
  });
});
