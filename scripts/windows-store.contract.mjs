import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, it } from "node:test";

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
    assert.deepEqual(storeBuildArguments(), ["build", "--no-bundle", "--no-sign", "--ci", "--", "--locked"]);
    assert.deepEqual(makeAppxPackArguments("C:\\layout", "C:\\output\\bottie.msix"), [
      "pack",
      "/o",
      "/d",
      "C:\\layout",
      "/p",
      "C:\\output\\bottie.msix",
    ]);
  });

  it("maps semantic versions to the Store's four numeric components", () => {
    assert.equal(packageVersion("0.9.0"), "1.9.0.0");
    assert.equal(packageVersion("1.2.65535"), "2.2.65535.0");
    assert.throws(() => packageVersion("1.2"), /semantic version/);
    assert.throws(() => packageVersion("1.2.65536"), /Store version/);
    assert.throws(() => packageVersion("65535.0.0"), /Store version/);
  });

  it("requires exact public Partner Center identity fields without treating them as credentials", () => {
    assert.deepEqual(
      resolveStoreIdentity({
        BOTTIE_WINDOWS_STORE_IDENTITY_NAME: STORE_IDENTITY.name,
        BOTTIE_WINDOWS_STORE_PUBLISHER: STORE_IDENTITY.publisher,
        BOTTIE_WINDOWS_STORE_PUBLISHER_DISPLAY_NAME: STORE_IDENTITY.publisherDisplayName,
      }),
      STORE_IDENTITY,
    );
    assert.throws(() => resolveStoreIdentity({}), /Partner Center identity/);
    assert.throws(
      () =>
        resolveStoreIdentity({
          BOTTIE_WINDOWS_STORE_IDENTITY_NAME: "bad identity",
          BOTTIE_WINDOWS_STORE_PUBLISHER: STORE_IDENTITY.publisher,
          BOTTIE_WINDOWS_STORE_PUBLISHER_DISPLAY_NAME: STORE_IDENTITY.publisherDisplayName,
        }),
      /identity name/,
    );
  });

  it("renders a full-trust desktop manifest with bounded escaped Store identity", () => {
    const manifest = renderAppxManifest(STORE_IDENTITY, "0.9.0", "x64");

    assert.ok(
      manifest.includes(
        '<Identity Name="12345HorstHerb.Bottie" Version="1.9.0.0" ' +
          'Publisher="CN=01234567-89ab-cdef-0123-456789abcdef" ProcessorArchitecture="x64" />',
      ),
    );
    assert.ok(manifest.includes('Name="Windows.Desktop"\n      MinVersion="10.0.19041.0"'));
    assert.ok(manifest.includes('<rescap:Capability Name="runFullTrust" />'));
    assert.ok(manifest.includes('Executable="bottie.exe"'));
    assert.ok(manifest.includes('uap10:RuntimeBehavior="packagedClassicApp"'));
    assert.ok(manifest.includes('uap10:TrustLevel="mediumIL"'));
    assert.ok(manifest.includes('Square150x150Logo="Assets\\Square150x150Logo.png"'));
    assert.ok(manifest.includes('Square44x44Logo="Assets\\Square44x44Logo.png"'));
    assert.ok(manifest.includes("<PublisherDisplayName>Horst Herb</PublisherDisplayName>"));
    assert.equal(manifest.endsWith("\n"), true);

    const escaped = renderAppxManifest({ ...STORE_IDENTITY, publisherDisplayName: "Herb & Co" }, "0.9.0", "x64");
    assert.ok(escaped.includes("<PublisherDisplayName>Herb &amp; Co</PublisherDisplayName>"));
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

    await assert.rejects(summarizeExtractedMsix(root, STORE_IDENTITY, "0.9.0"), /must remain unsigned/);
    await rm(join(root, "AppxSignature.p7x"));
    const evidence = await summarizeExtractedMsix(root, STORE_IDENTITY, "0.9.0");

    assert.deepEqual(
      {
        architecture: evidence.architecture,
        identity: evidence.identity,
        signed: evidence.signed,
        version: evidence.version,
      },
      {
        architecture: "x86_64",
        identity: { name: STORE_IDENTITY.name, publisherDisplayName: STORE_IDENTITY.publisherDisplayName },
        signed: false,
        version: "0.9.0",
      },
    );
    assert.equal(Object.hasOwn(evidence.identity, "publisher"), false);
    assert.deepEqual(Object.keys(evidence.requiredDocuments), ["licence", "modelNotice", "thirdPartyNotices"]);
    for (const digest of Object.values(evidence.requiredDocuments)) assert.match(digest, /^[a-f0-9]{64}$/);
  });

  it("keeps Store validation manual, credential-free, and non-publishing", async () => {
    const [workflow, packageJson] = await Promise.all([
      readFile(new URL("../.github/workflows/windows-store-msix-validation.yml", import.meta.url), "utf8"),
      readFile(new URL("../package.json", import.meta.url), "utf8"),
    ]);

    assert.ok(workflow.includes("workflow_dispatch:"));
    assert.ok(workflow.includes("identity_name:"));
    assert.ok(workflow.includes("publisher:"));
    assert.ok(workflow.includes("publisher_display_name:"));
    assert.ok(workflow.includes("package:windows:store"));
    assert.ok(workflow.includes("appcert.exe"));
    assert.ok(workflow.includes("package/windows-store/bottie-0.9.0-x64.msix"));
    assert.doesNotMatch(workflow, /pull_request:|push:|release:|secrets\./);
    assert.doesNotMatch(workflow, /winget|Partner Center|store submission/i);
    assert.equal(
      JSON.parse(packageJson).scripts["package:windows:store:test"],
      "node --test scripts/windows-store.contract.mjs",
    );
  });

  it("reduces a passing certification report to one hash-bound path-free result", () => {
    const report = '<?xml version="1.0"?><REPORT OVERALL_RESULT="PASS" PARTIAL_RUN="FALSE" host="C:\\private" />';

    const evidence = certificationKitEvidence(report);
    assert.equal(evidence.passed, true);
    assert.match(evidence.reportSha256, /^[a-f0-9]{64}$/);
    assert.equal(JSON.stringify(evidence).includes("private"), false);
    assert.throws(() => certificationKitEvidence(report.replace("PASS", "FAIL")), /did not pass/);
    assert.throws(
      () => certificationKitEvidence(report.replace('PARTIAL_RUN="FALSE"', 'PARTIAL_RUN="TRUE"')),
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
