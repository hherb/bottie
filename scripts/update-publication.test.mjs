import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import { buildUpdatePublication, expectedPublicationAssets, verifyGitHubRelease } from "./update-publication.mjs";

const VERSION = "0.9.0";
const SOURCE_SHA = "a".repeat(40);
const RAW_SIGNATURE = `untrusted comment: signature from minisign secret key
RWQf6LRCGA9i59SLOFxz6NxvASXDJeRtuZykwQepbDEGt87ig1BNpWaVWuNrm73YiIiJbq71Wi+dP9eKL8OC351vwIasSSbXxwA=
trusted comment: timestamp:1555779966\tfile:bottie
QtKMXWyYcwdpZAlPF7tE2ENJkRd1ujvKjlj1m9RtHTBnZPa5WKU5uWRs5GoP5M/VqE81QFuMKI5k/SfNQUaOAA==`;
const SIGNATURE = Buffer.from(RAW_SIGNATURE).toString("base64");
const PUBLIC_KEY = Buffer.from(
  `untrusted comment: minisign public key: E7620F1842B4E81F
RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3
`,
).toString("base64");

/** Returns one updater evidence record bound to the supplied final bytes. */
function updaterEvidence(target, bytes, signature) {
  return {
    artifact: { sha256: sha256(bytes), size: bytes.length },
    publicKeySha256: sha256(PUBLIC_KEY),
    schemaVersion: 1,
    signature: { format: "minisign", sha256: sha256(signature), verifies: true },
    target,
  };
}

/** Writes the exact three-platform publication fixture into one disposable root. */
async function writeFixture(repositoryRoot) {
  const artifactDirectory = join(repositoryRoot, "package", "updater-artifacts");
  await writeFile(join(repositoryRoot, "package.json"), JSON.stringify({ version: VERSION }));
  await writeFile(join(repositoryRoot, "RELEASE-NOTES.md"), "Bottie 0.9.0 beta tester release.");
  await writeFile(join(repositoryRoot, "updater.pub"), PUBLIC_KEY);
  await mkdir(artifactDirectory, { recursive: true });
  const evidence = {};
  for (const asset of expectedPublicationAssets(VERSION).filter((item) => item.kind === "artifact")) {
    const bytes = Buffer.from(`final-${asset.target}-bytes`);
    const signature = Buffer.from(SIGNATURE);
    await writeFile(join(artifactDirectory, asset.name), bytes);
    await writeFile(join(artifactDirectory, `${asset.name}.sig`), signature);
    evidence[asset.target] = updaterEvidence(asset.target, bytes, signature);
  }
  return evidence;
}

describe("protected updater publication", () => {
  it("builds a full latest-release manifest from exact three-platform final bytes", async () => {
    const repositoryRoot = await mkdtemp(join(tmpdir(), "bottie-publication-"));
    try {
      const evidence = await writeFixture(repositoryRoot);
      const publication = await buildUpdatePublication({
        artifactDirectory: join(repositoryRoot, "package", "updater-artifacts"),
        distributionEvidence: evidence,
        notes: "Bottie 0.9.0 beta tester release.",
        publicKey: PUBLIC_KEY,
        publishedAt: "2026-09-02T01:02:03Z",
        version: VERSION,
      });

      expect(Object.keys(publication.manifest.platforms)).toEqual(["darwin-aarch64", "linux-x86_64", "windows-x86_64"]);
      expect(publication.evidence).toMatchObject({
        status: "draft",
        targets: ["darwin-aarch64", "linux-x86_64", "windows-x86_64"],
        version: VERSION,
      });
      expect(JSON.stringify(publication.evidence)).not.toMatch(/signature|github\.com|\.msi|\.deb|\.tar\.gz/);
    } finally {
      await rm(repositoryRoot, { recursive: true, force: true });
    }
  });

  it("rejects any final byte or signature that differs from protected evidence", async () => {
    const repositoryRoot = await mkdtemp(join(tmpdir(), "bottie-publication-"));
    try {
      const evidence = await writeFixture(repositoryRoot);
      const asset = expectedPublicationAssets(VERSION).find((item) => item.target === "windows-x86_64");
      await writeFile(join(repositoryRoot, "package", "updater-artifacts", asset.name), "altered");

      await expect(
        buildUpdatePublication({
          artifactDirectory: join(repositoryRoot, "package", "updater-artifacts"),
          distributionEvidence: evidence,
          notes: "Bottie 0.9.0 beta tester release.",
          publicKey: PUBLIC_KEY,
          publishedAt: "2026-09-02T01:02:03Z",
          version: VERSION,
        }),
      ).rejects.toThrow(/protected evidence/);
    } finally {
      await rm(repositoryRoot, { recursive: true, force: true });
    }
  });

  it("accepts only a published full release on the exact current-main commit with exact assets", () => {
    const assets = expectedPublicationAssets(VERSION).map((asset) => ({
      digest: `sha256:${"b".repeat(64)}`,
      name: asset.name,
      size: 10,
      state: "uploaded",
    }));
    const expected = Object.fromEntries(assets.map((asset) => [asset.name, asset]));
    const release = {
      assets,
      draft: false,
      prerelease: false,
      tag_name: `v${VERSION}`,
      target_commitish: SOURCE_SHA,
    };

    expect(
      verifyGitHubRelease({
        expectedAssets: expected,
        latestRelease: release,
        release,
        sourceSha: SOURCE_SHA,
        version: VERSION,
      }),
    ).toBe(true);
    expect(() =>
      verifyGitHubRelease({
        expectedAssets: expected,
        latestRelease: { ...release, tag_name: "v0.8.0" },
        release,
        sourceSha: SOURCE_SHA,
        version: VERSION,
      }),
    ).toThrow(/latest full release/);
    expect(() =>
      verifyGitHubRelease({
        expectedAssets: expected,
        latestRelease: release,
        release: { ...release, prerelease: true },
        sourceSha: SOURCE_SHA,
        version: VERSION,
      }),
    ).toThrow(/full release/);
    expect(() =>
      verifyGitHubRelease({
        expectedAssets: expected,
        latestRelease: { ...release, tag_name: "v0.8.0" },
        release: { ...release, tag_name: "v0.8.0" },
        sourceSha: SOURCE_SHA,
        version: VERSION,
      }),
    ).toThrow(/full release/);
  });

  it("keeps publication manual, current-main-bound, environment-gated, and separate from Store work", async () => {
    const workflow = await readFile(new URL("../.github/workflows/updater-publication.yml", import.meta.url), "utf8");

    expect(workflow).toContain("workflow_dispatch:");
    expect(workflow).toContain("environment: updater-publication");
    expect(workflow).toContain('"publish Bottie 0.9.0 beta as the latest full GitHub release"');
    expect(workflow).toContain('current_main="$(gh api');
    expect(workflow).toContain('current_main" != "$GITHUB_SHA');
    expect(workflow).toContain("npm run release:candidate");
    expect(workflow).toContain("node scripts/update-publication.mjs --verify-draft");
    expect(workflow).toContain("node scripts/update-publication.mjs --verify-published");
    expect(workflow).toContain("-F prerelease=false");
    expect(workflow).toContain("-f make_latest=true");
    expect(workflow).not.toMatch(/pull_request:|push:|windows-store|Partner Center|Microsoft Store/);
  });
});

/** Returns one lowercase SHA-256 digest. */
function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}
