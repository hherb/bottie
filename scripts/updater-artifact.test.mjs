import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import { requireUpdaterSigningEnvironment, updaterSigningArguments } from "./updater-artifact.mjs";

describe("protected updater artifact signing", () => {
  it("requires one private-key source, a password, and no repository-contained path", () => {
    expect(
      requireUpdaterSigningEnvironment(
        {
          TAURI_SIGNING_PRIVATE_KEY: "encrypted-private-key-content",
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: "protected-password",
        },
        "/repo",
      ),
    ).toEqual({ source: "protected-content" });
    expect(
      requireUpdaterSigningEnvironment(
        {
          TAURI_SIGNING_PRIVATE_KEY_PATH: "/secure/bottie.key",
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: "protected-password",
        },
        "/repo",
      ),
    ).toEqual({ source: "protected-path" });
    expect(() => requireUpdaterSigningEnvironment({}, "/repo")).toThrow(/unavailable or ambiguous/);
    expect(() =>
      requireUpdaterSigningEnvironment(
        {
          TAURI_SIGNING_PRIVATE_KEY: "encrypted-private-key-content",
          TAURI_SIGNING_PRIVATE_KEY_PATH: "/secure/bottie.key",
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: "protected-password",
        },
        "/repo",
      ),
    ).toThrow(/ambiguous/);
    expect(() =>
      requireUpdaterSigningEnvironment(
        {
          TAURI_SIGNING_PRIVATE_KEY_PATH: "/repo/private.key",
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: "protected-password",
        },
        "/repo",
      ),
    ).toThrow(/outside the repository/);
  });

  it("passes no key or password through command arguments", () => {
    const arguments_ = updaterSigningArguments("/runner/final-artifact");

    expect(arguments_).toEqual(["--tauri", "signer", "sign", "/runner/final-artifact"]);
    expect(arguments_.join(" ")).not.toMatch(/password|private-key/);
  });

  it("signs only after the final platform trust operation in every protected path", async () => {
    const [macos, windows, linux] = await Promise.all([
      readFile(new URL("./macos-distribution.mjs", import.meta.url), "utf8"),
      readFile(new URL("./windows-distribution.mjs", import.meta.url), "utf8"),
      readFile(new URL("./linux-distribution.mjs", import.meta.url), "utf8"),
    ]);

    expect(macos.lastIndexOf("const notarization = notarizeAndVerify(")).toBeLessThan(
      macos.lastIndexOf("await createUpdaterArchive("),
    );
    expect(windows.indexOf("signAndVerify(signToolPath, credentials, msiPath)")).toBeLessThan(
      windows.lastIndexOf("signUpdaterArtifact("),
    );
    expect(linux.indexOf("signAndVerifyLinuxDistribution(configuration, debPath)")).toBeLessThan(
      linux.lastIndexOf("signUpdaterArtifact("),
    );
  });
});
