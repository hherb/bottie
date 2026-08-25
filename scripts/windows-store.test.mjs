import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import {
  certificationKitEvidence,
  makeAppxPackArguments,
  packageVersion,
  renderAppxManifest,
  resolveStoreIdentity,
  storeBuildArguments,
  summarizeExtractedMsix,
} from "./windows-store.mjs";

const STORE_IDENTITY = Object.freeze({
  name: "12345HorstHerb.Bottie",
  publisher: "CN=01234567-89ab-cdef-0123-456789abcdef",
  publisherDisplayName: "Horst Herb",
});
const temporaryDirectories = [];

afterEach(async () => {
  await Promise.all(temporaryDirectories.splice(0).map((path) => rm(path, { recursive: true, force: true })));
});

describe("Windows Store MSIX", () => {
  it("builds one locked unsigned executable before repository-owned packaging", () => {
    expect(storeBuildArguments()).toEqual(["build", "--no-bundle", "--no-sign", "--ci", "--", "--locked"]);
    expect(makeAppxPackArguments("C:\\layout", "C:\\output\\bottie.msix")).toEqual([
      "pack",
      "/o",
      "/d",
      "C:\\layout",
      "/p",
      "C:\\output\\bottie.msix",
    ]);
  });

  it("maps semantic versions to the Store's four numeric components", () => {
    expect(packageVersion("0.9.0")).toBe("1.9.0.0");
    expect(packageVersion("1.2.65535")).toBe("2.2.65535.0");
    expect(() => packageVersion("1.2")).toThrow(/semantic version/);
    expect(() => packageVersion("1.2.65536")).toThrow(/Store version/);
    expect(() => packageVersion("65535.0.0")).toThrow(/Store version/);
  });

  it("requires exact public Partner Center identity fields without treating them as credentials", () => {
    expect(
      resolveStoreIdentity({
        BOTTIE_WINDOWS_STORE_IDENTITY_NAME: STORE_IDENTITY.name,
        BOTTIE_WINDOWS_STORE_PUBLISHER: STORE_IDENTITY.publisher,
        BOTTIE_WINDOWS_STORE_PUBLISHER_DISPLAY_NAME: STORE_IDENTITY.publisherDisplayName,
      }),
    ).toEqual(STORE_IDENTITY);
    expect(() => resolveStoreIdentity({})).toThrow(/Partner Center identity/);
    expect(() =>
      resolveStoreIdentity({
        BOTTIE_WINDOWS_STORE_IDENTITY_NAME: "bad identity",
        BOTTIE_WINDOWS_STORE_PUBLISHER: STORE_IDENTITY.publisher,
        BOTTIE_WINDOWS_STORE_PUBLISHER_DISPLAY_NAME: STORE_IDENTITY.publisherDisplayName,
      }),
    ).toThrow(/identity name/);
  });

  it("renders a full-trust desktop manifest with bounded escaped Store identity", () => {
    const manifest = renderAppxManifest(STORE_IDENTITY, "0.9.0", "x64");

    expect(manifest).toContain(
      '<Identity Name="12345HorstHerb.Bottie" Version="1.9.0.0" ' +
        'Publisher="CN=01234567-89ab-cdef-0123-456789abcdef" ProcessorArchitecture="x64" />',
    );
    expect(manifest).toContain('Name="Windows.Desktop"\n      MinVersion="10.0.19041.0"');
    expect(manifest).toContain('<rescap:Capability Name="runFullTrust" />');
    expect(manifest).toContain('Executable="bottie.exe"');
    expect(manifest).toContain('uap10:RuntimeBehavior="packagedClassicApp"');
    expect(manifest).toContain('uap10:TrustLevel="mediumIL"');
    expect(manifest).toContain('Square150x150Logo="Assets\\Square150x150Logo.png"');
    expect(manifest).toContain('Square44x44Logo="Assets\\Square44x44Logo.png"');
    expect(manifest).toContain("<PublisherDisplayName>Horst Herb</PublisherDisplayName>");
    expect(manifest.endsWith("\n")).toBe(true);

    const escaped = renderAppxManifest({ ...STORE_IDENTITY, publisherDisplayName: "Herb & Co" }, "0.9.0", "x64");
    expect(escaped).toContain("<PublisherDisplayName>Herb &amp; Co</PublisherDisplayName>");
  });

  it("inspects only the required unsigned x64 payload and public package metadata", async () => {
    const root = await mkdtemp(join(tmpdir(), "bottie-msix-fixture-"));
    temporaryDirectories.push(root);
    await mkdir(join(root, "Assets"));
    await writeFile(join(root, "AppxManifest.xml"), renderAppxManifest(STORE_IDENTITY, "0.9.0", "x64"));
    await writeFile(
      join(root, "AppxBlockMap.xml"),
      '<BlockMap HashMethod="http://www.w3.org/2001/04/xmlenc#sha256" />',
    );
    await writeFile(join(root, "bottie.exe"), peFixture(0x8664));
    await writeFile(join(root, "[Content_Types].xml"), "content types");
    for (const name of ["StoreLogo.png", "Square44x44Logo.png", "Square150x150Logo.png"]) {
      await writeFile(join(root, "Assets", name), name);
    }
    await writeFile(join(root, "LICENSE"), "licence");
    await writeFile(join(root, "MODEL-NOTICE.txt"), "model notice");
    await writeFile(join(root, "THIRD-PARTY-NOTICES.txt"), "third party notices");
    await writeFile(join(root, "resources.pri"), "resources");
    await writeFile(join(root, "AppxSignature.p7x"), "unexpected signature");

    await expect(summarizeExtractedMsix(root, STORE_IDENTITY, "0.9.0")).rejects.toThrow(/must remain unsigned/);
    await rm(join(root, "AppxSignature.p7x"));
    const evidence = await summarizeExtractedMsix(root, STORE_IDENTITY, "0.9.0");

    expect(evidence).toMatchObject({
      architecture: "x86_64",
      identity: { name: STORE_IDENTITY.name, publisherDisplayName: STORE_IDENTITY.publisherDisplayName },
      signed: false,
      version: "0.9.0",
    });
    expect(evidence.identity).not.toHaveProperty("publisher");
    expect(evidence.requiredDocuments).toEqual({
      licence: expect.stringMatching(/^[a-f0-9]{64}$/),
      modelNotice: expect.stringMatching(/^[a-f0-9]{64}$/),
      thirdPartyNotices: expect.stringMatching(/^[a-f0-9]{64}$/),
    });
  });

  it("keeps Store validation manual, credential-free, and non-publishing", async () => {
    const workflow = await readFile(
      new URL("../.github/workflows/windows-store-msix-validation.yml", import.meta.url),
      "utf8",
    );

    expect(workflow).toContain("workflow_dispatch:");
    expect(workflow).toContain("identity_name:");
    expect(workflow).toContain("publisher:");
    expect(workflow).toContain("publisher_display_name:");
    expect(workflow).toContain("package:windows:store");
    expect(workflow).toContain("appcert.exe");
    expect(workflow).toContain("package/windows-store/bottie-0.9.0-x64.msix");
    expect(workflow).not.toMatch(/pull_request:|push:|release:|secrets\./);
    expect(workflow).not.toMatch(/winget|Partner Center|store submission/i);
  });

  it("reduces a passing certification report to one hash-bound path-free result", () => {
    const report = '<?xml version="1.0"?><REPORT OVERALL_RESULT="PASS" PARTIAL_RUN="FALSE" host="C:\\private" />';

    expect(certificationKitEvidence(report)).toEqual({
      passed: true,
      reportSha256: expect.stringMatching(/^[a-f0-9]{64}$/),
    });
    expect(JSON.stringify(certificationKitEvidence(report))).not.toContain("private");
    expect(() => certificationKitEvidence(report.replace("PASS", "FAIL"))).toThrow(/did not pass/);
    expect(() => certificationKitEvidence(report.replace('PARTIAL_RUN="FALSE"', 'PARTIAL_RUN="TRUE"'))).toThrow(
      /complete/,
    );
  });
});

/** Creates one minimal PE header with the supplied public machine type. */
function peFixture(machine) {
  const bytes = Buffer.alloc(128);
  bytes.write("MZ", 0, "ascii");
  bytes.writeUInt32LE(64, 0x3c);
  bytes.write("PE\0\0", 64, "binary");
  bytes.writeUInt16LE(machine, 68);
  return bytes;
}
