#!/usr/bin/env node

/** Builds and inspects Bottie's unsigned Microsoft Store MSIX package. */

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { copyFile, lstat, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const APPLICATION_DESCRIPTION = "A local-first, provider-flexible desktop chatbot with persistent memory.";
const APPLICATION_ID = "Bottie";
const APPLICATION_NAME = "bottie";
const DEFAULT_EVIDENCE_PATH = "package/windows-store/windows-store-evidence.json";
const DEFAULT_MSIX_PATH = "package/windows-store/bottie-0.9.0-x64.msix";
const MAKEAPPX_PATH_ENVIRONMENT = "BOTTIE_WINDOWS_MAKEAPPX_PATH";
const MAX_STORE_VERSION_COMPONENT = 65_535;
const MINIMUM_WINDOWS_VERSION = "10.0.19041.0";
const MAXIMUM_TESTED_WINDOWS_VERSION = "10.0.26100.0";
const PACKAGE_ARCHITECTURE = "x64";
const PE_ARCHITECTURES = new Map([
  [0x014c, "x86"],
  [0x8664, "x86_64"],
  [0xaa64, "aarch64"],
]);
const PE_MACHINE_OFFSET = 4;
const PE_OFFSET_POSITION = 0x3c;
const PE_SIGNATURE = Buffer.from([0x50, 0x45, 0x00, 0x00]);
const REQUIRED_ASSETS = Object.freeze([
  ["StoreLogo.png", "StoreLogo.png"],
  ["Square44x44Logo.png", "Square44x44Logo.png"],
  ["Square150x150Logo.png", "Square150x150Logo.png"],
]);
const REQUIRED_DOCUMENTS = new Map([
  ["LICENSE", "licence"],
  ["MODEL-NOTICE.txt", "modelNotice"],
  ["THIRD-PARTY-NOTICES.txt", "thirdPartyNotices"],
]);
const STORE_IDENTITY_NAME = "BOTTIE_WINDOWS_STORE_IDENTITY_NAME";
const STORE_PUBLISHER = "BOTTIE_WINDOWS_STORE_PUBLISHER";
const STORE_PUBLISHER_DISPLAY_NAME = "BOTTIE_WINDOWS_STORE_PUBLISHER_DISPLAY_NAME";
const STORE_NAME_PATTERN = /^[A-Za-z0-9.-]{3,50}$/;

/** Returns the locked executable-only build used before repository-owned MSIX packaging. */
export function storeBuildArguments() {
  return ["build", "--no-bundle", "--no-sign", "--ci", "--", "--locked"];
}

/** Maps product semver monotonically into Store's non-zero-major, zero-revision version contract. */
export function packageVersion(version) {
  const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.exec(version);
  if (!match) throw new Error("The Bottie version must be a three-component semantic version.");
  const components = match.slice(1).map(Number);
  const storeComponents = [components[0] + 1, components[1], components[2], 0];
  if (storeComponents.some((component) => component < 0 || component > MAX_STORE_VERSION_COMPONENT)) {
    throw new Error("The Bottie version cannot be represented as a Microsoft Store version.");
  }
  return storeComponents.join(".");
}

/** Resolves the exact public identity assigned by Partner Center and rejects guessed or malformed values. */
export function resolveStoreIdentity(environment) {
  const identity = {
    name: environment[STORE_IDENTITY_NAME]?.trim(),
    publisher: environment[STORE_PUBLISHER]?.trim(),
    publisherDisplayName: environment[STORE_PUBLISHER_DISPLAY_NAME]?.trim(),
  };
  if (!identity.name || !identity.publisher || !identity.publisherDisplayName) {
    throw new Error("The complete public Partner Center identity is required.");
  }
  if (!STORE_NAME_PATTERN.test(identity.name)) throw new Error("The Partner Center identity name is malformed.");
  validateXmlValue(identity.publisher, "publisher", 8_192);
  validateXmlValue(identity.publisherDisplayName, "publisher display name", 256);
  return identity;
}

/** Renders the deterministic full-trust desktop package manifest accepted by MakeAppx. */
export function renderAppxManifest(identity, version, architecture) {
  if (architecture !== PACKAGE_ARCHITECTURE) throw new Error("The current Store package supports only x64 Windows.");
  const name = xmlAttribute(identity.name);
  const publisher = xmlAttribute(identity.publisher);
  const publisherDisplayName = xmlText(identity.publisherDisplayName);
  const storeVersion = packageVersion(version);
  return `<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:uap10="http://schemas.microsoft.com/appx/manifest/uap/windows10/10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap uap10 rescap">
  <Identity Name="${name}" Version="${storeVersion}" Publisher="${publisher}" ProcessorArchitecture="${architecture}" />
  <Properties>
    <DisplayName>${APPLICATION_NAME}</DisplayName>
    <PublisherDisplayName>${publisherDisplayName}</PublisherDisplayName>
    <Description>${APPLICATION_DESCRIPTION}</Description>
    <Logo>Assets\\StoreLogo.png</Logo>
  </Properties>
  <Resources>
    <Resource Language="en-us" />
  </Resources>
  <Dependencies>
    <TargetDeviceFamily
      Name="Windows.Desktop"
      MinVersion="${MINIMUM_WINDOWS_VERSION}"
      MaxVersionTested="${MAXIMUM_TESTED_WINDOWS_VERSION}" />
  </Dependencies>
  <Applications>
    <Application
      Id="${APPLICATION_ID}"
      Executable="bottie.exe"
      uap10:RuntimeBehavior="packagedClassicApp"
      uap10:TrustLevel="mediumIL">
      <uap:VisualElements
        DisplayName="${APPLICATION_NAME}"
        Description="${APPLICATION_DESCRIPTION}"
        BackgroundColor="transparent"
        Square150x150Logo="Assets\\Square150x150Logo.png"
        Square44x44Logo="Assets\\Square44x44Logo.png" />
    </Application>
  </Applications>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
</Package>
`;
}

/** Returns one overwrite-safe MakeAppx pack invocation. */
export function makeAppxPackArguments(layoutDirectory, outputPath) {
  return ["pack", "/o", "/d", layoutDirectory, "/p", outputPath];
}

/** Returns one overwrite-safe MakeAppx unpack invocation for independent inspection. */
function makeAppxUnpackArguments(packagePath, outputDirectory) {
  return ["unpack", "/o", "/p", packagePath, "/d", outputDirectory];
}

/** Inspects one independently unpacked unsigned MSIX using path-free public evidence. */
export async function summarizeExtractedMsix(rootPath, identity, version) {
  const root = resolve(rootPath);
  if (!(await lstat(root)).isDirectory()) throw new Error("The unpacked MSIX root is unavailable.");
  const files = [];
  await visitRegularFiles(root, root, files);
  if (files.some((file) => file.path.toLowerCase() === "appxsignature.p7x")) {
    throw new Error("The Microsoft Store submission package must remain unsigned.");
  }
  const expectedManifest = renderAppxManifest(identity, version, PACKAGE_ARCHITECTURE);
  const manifest = await readFile(join(root, "AppxManifest.xml"), "utf8");
  if (manifest !== expectedManifest) throw new Error("The unpacked MSIX manifest differs from its reviewed contract.");
  const blockMap = await readFile(join(root, "AppxBlockMap.xml"), "utf8");
  if (!blockMap.includes("http://www.w3.org/2001/04/xmlenc#sha256")) {
    throw new Error("The MSIX block map must use SHA2-256.");
  }
  requirePaths(files, [
    "AppxBlockMap.xml",
    "AppxManifest.xml",
    "[Content_Types].xml",
    "bottie.exe",
    ...REQUIRED_ASSETS.map(([name]) => `Assets/${name}`),
    ...REQUIRED_DOCUMENTS.keys(),
  ]);
  const executableArchitecture = await inspectPortableExecutable(join(root, "bottie.exe"));
  if (executableArchitecture !== "x86_64") throw new Error("The Store MSIX must contain the x64 Bottie executable.");
  const digest = createHash("sha256");
  for (const file of files) digest.update(`${file.path}\0${file.sha256}\0`);
  return {
    architecture: executableArchitecture,
    fileCount: files.length,
    identity: { name: identity.name, publisherDisplayName: identity.publisherDisplayName },
    packageDigest: digest.digest("hex"),
    packageVersion: packageVersion(version),
    requiredAssets: Object.fromEntries(REQUIRED_ASSETS.map(([name]) => [name, true])),
    requiredDocuments: Object.fromEntries(
      [...REQUIRED_DOCUMENTS].map(([name, key]) => [key, files.find((file) => file.path === name)?.sha256]),
    ),
    signed: false,
    totalBytes: files.reduce((total, file) => total + file.size, 0),
    version,
  };
}

/** Reduces one complete passing Windows App Certification Kit report to a path-free hash-bound result. */
export function certificationKitEvidence(report) {
  if (typeof report !== "string" || Buffer.byteLength(report) > 16 * 1024 * 1024) {
    throw new Error("The Windows App Certification Kit report is invalid.");
  }
  const root = report.match(/<REPORT\b([^>]*)>/)?.[1];
  if (!root || !/\bPARTIAL_RUN="FALSE"/.test(root)) {
    throw new Error("The Windows App Certification Kit report is incomplete.");
  }
  if (!/\bOVERALL_RESULT="PASS"/.test(root)) {
    throw new Error("Windows App Certification Kit did not pass.");
  }
  return { passed: true, reportSha256: createHash("sha256").update(report).digest("hex") };
}

/** Creates a deterministic package layout from the locked executable and reviewed repository assets. */
async function createPackageLayout(repositoryRoot, layoutDirectory, executablePath, identity, version) {
  const assetsDirectory = join(layoutDirectory, "Assets");
  await mkdir(assetsDirectory, { recursive: true });
  await copyFile(executablePath, join(layoutDirectory, "bottie.exe"));
  for (const name of REQUIRED_DOCUMENTS.keys()) await copyFile(join(repositoryRoot, name), join(layoutDirectory, name));
  for (const [destination, source] of REQUIRED_ASSETS) {
    await copyFile(join(repositoryRoot, "src-tauri", "icons", source), join(assetsDirectory, destination));
  }
  await writeFile(
    join(layoutDirectory, "AppxManifest.xml"),
    renderAppxManifest(identity, version, PACKAGE_ARCHITECTURE),
  );
}

/** Builds one locked release executable with Tauri's existing cross-platform wrapper. */
function buildExecutable(repositoryRoot, targetDirectory) {
  const wrapper = join(repositoryRoot, "scripts", "macos-development-signing.mjs");
  const result = spawnSync(process.execPath, [wrapper, "--tauri", ...storeBuildArguments()], {
    cwd: repositoryRoot,
    env: { ...process.env, CARGO_TARGET_DIR: targetDirectory },
    stdio: "inherit",
  });
  if (result.status !== 0) throw new Error("The locked Windows Store executable build failed.");
}

/** Runs MakeAppx without retaining its host paths or raw output. */
function runMakeAppx(makeAppxPath, arguments_) {
  const result = spawnSync(makeAppxPath, arguments_, { encoding: "utf8" });
  if (result.error || result.status !== 0) throw new Error("Microsoft MakeAppx rejected the Store package.");
}

/** Builds, unpacks, and inspects one real unsigned Store package in disposable directories. */
async function buildStorePackage(repositoryRoot, identity) {
  const config = JSON.parse(await readFile(join(repositoryRoot, "src-tauri", "tauri.conf.json"), "utf8"));
  const version = config.version;
  const makeAppxPath = resolveMakeAppxPath(process.env);
  const outputPath = resolveBoundedOutput(
    repositoryRoot,
    process.env.BOTTIE_WINDOWS_STORE_MSIX_PATH,
    DEFAULT_MSIX_PATH,
  );
  const evidencePath = resolveBoundedOutput(
    repositoryRoot,
    process.env.BOTTIE_WINDOWS_STORE_EVIDENCE_PATH,
    DEFAULT_EVIDENCE_PATH,
  );
  const temporaryRoot = await mkdtemp(join(tmpdir(), "bottie-windows-store-"));
  try {
    const targetDirectory = join(temporaryRoot, "target");
    const layoutDirectory = join(temporaryRoot, "layout");
    const extractedDirectory = join(temporaryRoot, "extracted");
    await mkdir(layoutDirectory);
    await mkdir(extractedDirectory);
    buildExecutable(repositoryRoot, targetDirectory);
    await createPackageLayout(
      repositoryRoot,
      layoutDirectory,
      join(targetDirectory, "release", "bottie.exe"),
      identity,
      version,
    );
    await mkdir(dirname(outputPath), { recursive: true });
    runMakeAppx(makeAppxPath, makeAppxPackArguments(layoutDirectory, outputPath));
    runMakeAppx(makeAppxPath, makeAppxUnpackArguments(outputPath, extractedDirectory));
    const evidence = {
      schemaVersion: 1,
      ...(await summarizeExtractedMsix(extractedDirectory, identity, version)),
      msix: await fileSummary(outputPath),
    };
    await mkdir(dirname(evidencePath), { recursive: true });
    await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
}

/** Adds only a complete passing certification result to the existing bounded package evidence. */
async function recordCertificationKit(repositoryRoot, suppliedReportPath) {
  const reportPath = resolveBoundedOutput(repositoryRoot, suppliedReportPath, "");
  const evidencePath = resolveBoundedOutput(
    repositoryRoot,
    process.env.BOTTIE_WINDOWS_STORE_EVIDENCE_PATH,
    DEFAULT_EVIDENCE_PATH,
  );
  const evidence = JSON.parse(await readFile(evidencePath, "utf8"));
  if (evidence?.schemaVersion !== 1 || evidence?.signed !== false) {
    throw new Error("The Windows Store package evidence is unavailable.");
  }
  evidence.certificationKit = certificationKitEvidence(await readFile(reportPath, "utf8"));
  await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
}

/** Requires one absolute caller-selected Windows SDK MakeAppx executable. */
function resolveMakeAppxPath(environment) {
  const path = environment[MAKEAPPX_PATH_ENVIRONMENT]?.trim();
  if (!path || !isAbsolute(path)) throw new Error("The Windows SDK MakeAppx path is unavailable.");
  return path;
}

/** Constrains generated package and evidence files to the ignored package directory. */
function resolveBoundedOutput(repositoryRoot, supplied, fallback) {
  const output = resolve(repositoryRoot, supplied?.trim() || fallback);
  const packageRoot = resolve(repositoryRoot, "package");
  const child = relative(packageRoot, output);
  if (!child || child === ".." || child.startsWith(`..${sep}`) || isAbsolute(child)) {
    throw new Error("Windows Store outputs must stay inside the repository package directory.");
  }
  return output;
}

/** Recursively inventories regular unpacked package files and rejects links or special entries. */
async function visitRegularFiles(directory, root, files) {
  const entries = await readdir(directory, { withFileTypes: true });
  entries.sort((left, right) => left.name.localeCompare(right.name));
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) await visitRegularFiles(path, root, files);
    else if (entry.isFile()) {
      const bytes = await readFile(path);
      files.push({
        path: relative(root, path).split(sep).join("/"),
        sha256: createHash("sha256").update(bytes).digest("hex"),
        size: bytes.length,
      });
    } else throw new Error("The unpacked MSIX contains an unsupported filesystem entry.");
  }
}

/** Requires each reviewed package-relative path exactly once. */
function requirePaths(files, requiredPaths) {
  for (const path of requiredPaths) {
    if (files.filter((file) => file.path === path).length !== 1) throw new Error(`The MSIX must contain ${path}.`);
  }
}

/** Returns one public file digest without retaining its host path. */
async function fileSummary(path) {
  const bytes = await readFile(path);
  return { sha256: createHash("sha256").update(bytes).digest("hex"), size: bytes.length };
}

/** Reads the public machine field from one Portable Executable header. */
async function inspectPortableExecutable(path) {
  const bytes = await readFile(path);
  if (bytes.length <= PE_OFFSET_POSITION + 4 || bytes.subarray(0, 2).toString("ascii") !== "MZ") {
    throw new Error("The packaged Bottie executable is not a valid PE image.");
  }
  const peOffset = bytes.readUInt32LE(PE_OFFSET_POSITION);
  const hasCompleteHeader = bytes.length > peOffset + PE_MACHINE_OFFSET + 2;
  if (!hasCompleteHeader || !bytes.subarray(peOffset, peOffset + 4).equals(PE_SIGNATURE)) {
    throw new Error("The packaged Bottie executable has an invalid PE header.");
  }
  return PE_ARCHITECTURES.get(bytes.readUInt16LE(peOffset + PE_MACHINE_OFFSET)) ?? "unknown";
}

/** Validates one bounded XML field before escaping. */
function validateXmlValue(value, description, maximumLength) {
  if (!value || value.length > maximumLength || /[\u0000-\u001f\u007f]/.test(value)) {
    throw new Error(`The Partner Center ${description} is malformed.`);
  }
}

/** Escapes one XML attribute after bounded validation. */
function xmlAttribute(value) {
  validateXmlValue(value, "manifest attribute", 8_192);
  return value.replaceAll("&", "&amp;").replaceAll('"', "&quot;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

/** Escapes one XML text node after bounded validation. */
function xmlText(value) {
  validateXmlValue(value, "manifest text", 256);
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

/** Accepts only the explicit credential-free Store package mode. */
async function main() {
  if (process.platform !== "win32") throw new Error("Windows Store packaging requires a Windows host.");
  const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const arguments_ = process.argv.slice(2);
  if (arguments_.length === 1 && arguments_[0] === "--build") {
    await buildStorePackage(repositoryRoot, resolveStoreIdentity(process.env));
    return;
  }
  if (arguments_.length === 2 && arguments_[0] === "--record-certification") {
    await recordCertificationKit(repositoryRoot, arguments_[1]);
    return;
  }
  throw new Error("Use the exact --build or --record-certification mode for Windows Store packaging.");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`[bottie] ${error instanceof Error ? error.message : "Windows Store packaging failed."}`);
    process.exitCode = 1;
  });
}
