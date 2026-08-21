import { describe, expect, it } from "vitest";

import {
  cargoRunnerValue,
  resolveTauriCliPath,
  selectAppleDevelopmentIdentity,
  shouldConfigureDevelopmentSigning,
} from "./macos-development-signing.mjs";

const ONE_IDENTITY = `
  1) AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA "Apple Development: Example One (TEAMONE)"
  2) BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB "Developer ID Application: Example One (TEAMONE)"
     2 valid identities found
`;

describe("macOS development signing", () => {
  it("selects the only Apple Development identity without retaining its label", () => {
    expect(selectAppleDevelopmentIdentity(ONE_IDENTITY)).toBe("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
  });

  it("requires an explicit identity when more than one development identity is usable", () => {
    const identities = `${ONE_IDENTITY}
      3) CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC "Apple Development: Example Two (TEAMTWO)"`;

    expect(() => selectAppleDevelopmentIdentity(identities)).toThrow(/BOTTIE_APPLE_SIGNING_IDENTITY/);
    expect(selectAppleDevelopmentIdentity(identities, "cccccccccccccccccccccccccccccccccccccccc")).toBe(
      "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
    );
  });

  it("rejects missing and non-development identities", () => {
    expect(() => selectAppleDevelopmentIdentity("0 valid identities found")).toThrow(/Apple Development/);
    expect(() => selectAppleDevelopmentIdentity(ONE_IDENTITY, "missing identity")).toThrow(/does not match/);
  });

  it("configures the Cargo runner only for Tauri development on macOS", () => {
    expect(shouldConfigureDevelopmentSigning("darwin", ["dev"])).toBe(true);
    expect(shouldConfigureDevelopmentSigning("darwin", ["build"])).toBe(false);
    expect(shouldConfigureDevelopmentSigning("linux", ["dev"])).toBe(false);
  });

  it("builds Cargo's literal runner arguments and rejects paths it cannot represent", () => {
    expect(cargoRunnerValue("/usr/local/bin/node", "/repo/scripts/runner.mjs")).toBe(
      "/usr/local/bin/node /repo/scripts/runner.mjs --cargo-runner",
    );
    expect(() => cargoRunnerValue("/node path/node", "/repo/scripts/runner.mjs")).toThrow(/whitespace/);
    expect(() => cargoRunnerValue("/usr/local/bin/node", "/repo path/runner.mjs")).toThrow(/whitespace/);
  });

  it("runs Tauri's executable beside its package entry point", () => {
    expect(resolveTauriCliPath("/repo/node_modules/@tauri-apps/cli/main.js")).toBe(
      "/repo/node_modules/@tauri-apps/cli/tauri.js",
    );
  });
});
