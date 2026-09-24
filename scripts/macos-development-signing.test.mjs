import { describe, expect, it } from "vitest";

import {
  cargoRunnerValue,
  pythonDevelopmentSigningPlan,
  pythonDevelopmentArguments,
  pythonDevelopmentEnvironment,
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

  it("selects the explicit platform Python development resources only for dev", () => {
    expect(pythonDevelopmentArguments("linux", ["dev"])).toEqual([
      "dev",
      "--config",
      "src-tauri/tauri.python-development.linux.conf.json",
    ]);
    expect(pythonDevelopmentArguments("win32", ["dev", "--no-watch"])).toEqual([
      "dev",
      "--no-watch",
      "--config",
      "src-tauri/tauri.python-development.windows.conf.json",
    ]);

    const macos = pythonDevelopmentArguments("darwin", ["dev"]);
    expect(macos.slice(0, 2)).toEqual(["dev", "--config"]);
    expect(JSON.parse(macos[2])).toEqual({
      bundle: {
        resources: {
          "../package/python-development/BottiePythonXPCClient.app": "BottiePythonXPCClient.app",
          "../package/python-development/python-runtime-evidence.json": "python-runtime-evidence.json",
        },
      },
    });
    expect(() => pythonDevelopmentArguments("darwin", ["build"])).toThrow(/development/);
    expect(() => pythonDevelopmentArguments("freebsd", ["dev"])).toThrow(/platform/);
    expect(pythonDevelopmentArguments("linux", ["dev", "--", "application-argument"])).toEqual([
      "dev",
      "--config",
      "src-tauri/tauri.python-development.linux.conf.json",
      "--",
      "application-argument",
    ]);
  });

  it("scopes debug Python activation to the explicit command environment", () => {
    expect(pythonDevelopmentEnvironment({ EXISTING: "value" })).toEqual({
      EXISTING: "value",
      BOTTIE_PYTHON_DEVELOPMENT: "1",
    });
    expect(pythonDevelopmentEnvironment({ BOTTIE_PYTHON_DEVELOPMENT: "1" })).toEqual({
      BOTTIE_PYTHON_DEVELOPMENT: "1",
    });
    expect(() => pythonDevelopmentEnvironment({ BOTTIE_PYTHON_DEVELOPMENT: "unexpected" })).toThrow(/already set/);
  });

  it("signs staged Python code inside-out with the selected development identity", () => {
    const identity = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    const plan = pythonDevelopmentSigningPlan("/repo", identity);

    expect(plan.map((entry) => entry.kind)).toEqual(["runner", "service", "client"]);
    expect(plan[0].path).toBe(
      "/repo/package/python-development/BottiePythonXPCClient.app/Contents/XPCServices/" +
        "com.bottie.python-runner.xpc/Contents/Helpers/bottie-python-runner",
    );
    expect(plan[0].arguments).toContain("/repo/macos-python-xpc/Runner.entitlements");
    expect(plan[1].arguments).toContain("/repo/macos-python-xpc/Service.entitlements");
    expect(plan[2].path).toBe("/repo/package/python-development/BottiePythonXPCClient.app");
    for (const entry of plan) {
      expect(entry.arguments).toContain(identity);
      expect(entry.arguments.at(-1)).toBe(entry.path);
    }
  });
});
