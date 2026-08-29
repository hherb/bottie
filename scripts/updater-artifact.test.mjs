import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import {
  bindUpdaterArtifactEvidence,
  parseUpdaterArtifactEvidence,
  publicUpdaterVerificationEnvironment,
  requireUpdaterSigningEnvironment,
  updaterSigningArguments,
  updaterVerificationArguments,
} from "./updater-artifact.mjs";

const SHA_A = "a".repeat(64);
const SHA_B = "b".repeat(64);
const SHA_C = "c".repeat(64);

/** Returns one path-free verified updater record shaped like the Rust verifier output. */
function verifiedUpdaterEvidence() {
  return {
    schemaVersion: 1,
    artifact: { sha256: SHA_A, size: 42 },
    publicKeySha256: SHA_B,
    signature: { format: "minisign", sha256: SHA_C, verifies: true },
  };
}

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

  it("removes every private signing value before invoking the public verifier", () => {
    const environment = publicUpdaterVerificationEnvironment({
      BOTTIE_APPLE_NOTARY_KEY_P8: "apple-private-key",
      BOTTIE_LINUX_SIGNING_KEY_PASSPHRASE: "linux-passphrase",
      BOTTIE_WINDOWS_SIGNING_CERTIFICATE_PASSWORD: "windows-password",
      PATH: "/usr/bin",
      SAFE_BUILD_VALUE: "retained",
      TAURI_SIGNING_PRIVATE_KEY: "encrypted-private-key-content",
      TAURI_SIGNING_PRIVATE_KEY_PASSWORD: "protected-password",
      TAURI_SIGNING_PRIVATE_KEY_PATH: "/secure/bottie.key",
    });

    expect(environment).toEqual({ PATH: "/usr/bin", SAFE_BUILD_VALUE: "retained" });
    expect(JSON.stringify(environment)).not.toMatch(/private-key|passphrase|password|bottie\.key/);
  });

  it("invokes the locked native verifier with only public and artifact paths", () => {
    const arguments_ = updaterVerificationArguments("/repo", "/runner/final-artifact", "/runner/final-artifact.sig");

    expect(arguments_).toEqual([
      "run",
      "--quiet",
      "--locked",
      "--manifest-path",
      "/repo/src-tauri/Cargo.toml",
      "--bin",
      "bottie-updater-evidence",
      "--",
      "--verify",
      "/runner/final-artifact",
      "/runner/final-artifact.sig",
      "/repo/distribution/update/bottie-updater.pub",
    ]);
    expect(arguments_.join(" ")).not.toMatch(/password|private-key/);
  });

  it("accepts only exact path-free cryptographic verifier output", () => {
    const evidence = verifiedUpdaterEvidence();

    expect(parseUpdaterArtifactEvidence(`${JSON.stringify(evidence)}\n`)).toEqual(evidence);
    expect(parseUpdaterArtifactEvidence(JSON.stringify({ ...evidence, hostPath: "/runner/private" }))).toBeNull();
    expect(
      parseUpdaterArtifactEvidence(
        JSON.stringify({ ...evidence, signature: { ...evidence.signature, verifies: false } }),
      ),
    ).toBeNull();
    expect(parseUpdaterArtifactEvidence(JSON.stringify({ ...evidence, publicKeySha256: "short" }))).toBeNull();
  });

  it("binds one verified artifact to an exact supported target and final distribution hash", () => {
    const evidence = verifiedUpdaterEvidence();

    expect(bindUpdaterArtifactEvidence(evidence, "linux-x86_64", SHA_A)).toEqual({
      ...evidence,
      target: "linux-x86_64",
    });
    expect(() => bindUpdaterArtifactEvidence(evidence, "linux-x86_64", SHA_B)).toThrow(/final artifact/);
    expect(() => bindUpdaterArtifactEvidence(evidence, "android-aarch64", SHA_A)).toThrow(/target/);
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
