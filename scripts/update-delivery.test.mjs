import { createHash } from "node:crypto";

import { describe, expect, it } from "vitest";

import {
  buildStaticUpdateManifest,
  buildUpdateDeliveryEvidence,
  summarizeUpdateDeliveryEvidence,
} from "./update-delivery.mjs";

const SHA_A = "a".repeat(64);
const SHA_B = "b".repeat(64);
const RAW_SIGNATURE_A = `untrusted comment: signature from minisign secret key
RWQf6LRCGA9i59SLOFxz6NxvASXDJeRtuZykwQepbDEGt87ig1BNpWaVWuNrm73YiIiJbq71Wi+dP9eKL8OC351vwIasSSbXxwA=
trusted comment: timestamp:1555779966\tfile:bottie
QtKMXWyYcwdpZAlPF7tE2ENJkRd1ujvKjlj1m9RtHTBnZPa5WKU5uWRs5GoP5M/VqE81QFuMKI5k/SfNQUaOAA==`;
const SIGNATURE_A = Buffer.from(RAW_SIGNATURE_A).toString("base64");
const SIGNATURE_B = Buffer.from(RAW_SIGNATURE_A.replace("59SL", "58SL")).toString("base64");
const PUBLIC_KEY = Buffer.from(
  `untrusted comment: minisign public key: E7620F1842B4E81F
RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3
`,
).toString("base64");

const ARTIFACTS = [
  {
    artifactSha256: SHA_A,
    signature: SIGNATURE_A,
    target: "darwin-aarch64",
    url: "https://github.com/hherb/bottie/releases/download/v0.9.0/bottie.app.tar.gz",
  },
  {
    artifactSha256: SHA_B,
    signature: SIGNATURE_B,
    target: "linux-x86_64",
    url: "https://github.com/hherb/bottie/releases/download/v0.9.0/bottie.AppImage",
  },
];

describe("signed update delivery", () => {
  it("builds one deterministic static Tauri manifest from exact signed artifacts", () => {
    const manifest = buildStaticUpdateManifest({
      artifacts: [...ARTIFACTS].reverse(),
      notes: "Bottie 0.9.0 beta.",
      publishedAt: "2026-08-28T01:02:03Z",
      version: "0.9.0",
    });

    expect(manifest).toEqual({
      notes: "Bottie 0.9.0 beta.",
      platforms: {
        "darwin-aarch64": {
          signature: SIGNATURE_A,
          url: ARTIFACTS[0].url,
        },
        "linux-x86_64": {
          signature: SIGNATURE_B,
          url: ARTIFACTS[1].url,
        },
      },
      pub_date: "2026-08-28T01:02:03.000Z",
      version: "0.9.0",
    });
  });

  it("rejects unsupported targets, duplicate targets, mutable URLs, and absent signature content", () => {
    const input = {
      artifacts: ARTIFACTS,
      notes: "Bottie 0.9.0 beta.",
      publishedAt: "2026-08-28T01:02:03Z",
      version: "0.9.0",
    };

    expect(() =>
      buildStaticUpdateManifest({
        ...input,
        artifacts: [{ ...ARTIFACTS[0], target: "android-aarch64" }],
      }),
    ).toThrow(/target/);
    expect(() => buildStaticUpdateManifest({ ...input, artifacts: [ARTIFACTS[0], ARTIFACTS[0]] })).toThrow(/duplicate/);
    expect(() =>
      buildStaticUpdateManifest({
        ...input,
        artifacts: [{ ...ARTIFACTS[0], url: "https://github.com/hherb/bottie/releases/latest/download/app.tar.gz" }],
      }),
    ).toThrow(/immutable release tag/);
    expect(() => buildStaticUpdateManifest({ ...input, artifacts: [{ ...ARTIFACTS[0], signature: "" }] })).toThrow(
      /signature/,
    );
    expect(() =>
      buildStaticUpdateManifest({
        ...input,
        artifacts: [{ ...ARTIFACTS[0], signature: Buffer.from("R".repeat(64)).toString("base64") }],
      }),
    ).toThrow(/minisign/);
  });

  it("emits path-free publication evidence bound to manifest, public key, and artifact hashes", () => {
    const manifest = buildStaticUpdateManifest({
      artifacts: ARTIFACTS,
      notes: "Bottie 0.9.0 beta.",
      publishedAt: "2026-08-28T01:02:03Z",
      version: "0.9.0",
    });
    const evidence = buildUpdateDeliveryEvidence({
      artifacts: ARTIFACTS,
      manifest,
      publicKey: PUBLIC_KEY,
      status: "published",
    });

    expect(evidence).toMatchObject({
      schemaVersion: 1,
      status: "published",
      targets: ["darwin-aarch64", "linux-x86_64"],
      version: "0.9.0",
    });
    expect(evidence.manifest.sha256).toMatch(/^[a-f0-9]{64}$/);
    expect(evidence.publicKeySha256).toMatch(/^[a-f0-9]{64}$/);
    expect(evidence.artifacts).toEqual([
      { sha256: SHA_A, target: "darwin-aarch64" },
      { sha256: SHA_B, target: "linux-x86_64" },
    ]);
    expect(JSON.stringify(evidence)).not.toMatch(/signature|github\.com|\.tar\.gz|AppImage|untrusted comment/);
  });

  it("binds evidence to exact canonical public-key file bytes, including its final newline", () => {
    const manifest = buildStaticUpdateManifest({
      artifacts: ARTIFACTS,
      notes: "Bottie 0.9.0 beta.",
      publishedAt: "2026-08-28T01:02:03Z",
      version: "0.9.0",
    });
    const publicKeyFile = `${PUBLIC_KEY}\n`;
    const evidence = buildUpdateDeliveryEvidence({
      artifacts: ARTIFACTS,
      manifest,
      publicKey: publicKeyFile,
      status: "draft",
    });

    expect(evidence.publicKeySha256).toBe(createHash("sha256").update(publicKeyFile).digest("hex"));
  });

  it("accepts only exact published evidence and fails closed on altered bindings", () => {
    const manifest = buildStaticUpdateManifest({
      artifacts: ARTIFACTS,
      notes: "Bottie 0.9.0 beta.",
      publishedAt: "2026-08-28T01:02:03Z",
      version: "0.9.0",
    });
    const evidence = buildUpdateDeliveryEvidence({
      artifacts: ARTIFACTS,
      manifest,
      publicKey: PUBLIC_KEY,
      status: "published",
    });

    expect(summarizeUpdateDeliveryEvidence(evidence)).toEqual(evidence);
    expect(summarizeUpdateDeliveryEvidence({ ...evidence, status: "draft" })).toBeNull();
    expect(
      summarizeUpdateDeliveryEvidence({
        ...evidence,
        manifest: { ...evidence.manifest, sha256: "not-a-hash" },
      }),
    ).toBeNull();
    expect(
      summarizeUpdateDeliveryEvidence({
        ...evidence,
        artifacts: [...evidence.artifacts, { sha256: SHA_A, target: "windows-x86_64" }],
      }),
    ).toBeNull();
    expect(() =>
      buildUpdateDeliveryEvidence({
        artifacts: ARTIFACTS,
        manifest,
        publicKey: "/tmp/updater.pub",
        status: "published",
      }),
    ).toThrow(/public key/);
  });
});
