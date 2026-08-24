#!/usr/bin/env node

/** Generates and verifies Bottie's approved desktop application icon assets. */

import { spawnSync } from "node:child_process";
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const REPOSITORY_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const TAURI_CLI = "node_modules/@tauri-apps/cli/tauri.js";
const ICON_OUTPUT_DIRECTORY = "src-tauri/icons";
const FAVICON_OUTPUT = "static/favicon.png";
const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
const PNG_IHDR_MINIMUM_BYTES = 29;

/** Approved project-owned source image used by the locked Tauri icon generator. */
export const APPLICATION_ICON_SOURCE = "assets/bottie-logo-kit/bottie-icon-512.png";

/** Approved small-size source copied exactly to the compiled WebView favicon path. */
export const FAVICON_SOURCE = "assets/bottie-logo-kit/favicon-64.png";

/** Required generated desktop PNG names and their exact square pixel dimensions. */
export const DESKTOP_ICON_PNG_SIZES = Object.freeze({
  "32x32.png": 32,
  "64x64.png": 64,
  "128x128.png": 128,
  "128x128@2x.png": 256,
  "icon.png": 512,
  "Square30x30Logo.png": 30,
  "Square44x44Logo.png": 44,
  "StoreLogo.png": 50,
  "Square71x71Logo.png": 71,
  "Square89x89Logo.png": 89,
  "Square107x107Logo.png": 107,
  "Square142x142Logo.png": 142,
  "Square150x150Logo.png": 150,
  "Square284x284Logo.png": 284,
  "Square310x310Logo.png": 310,
});

const DESKTOP_ICON_FILES = [...Object.keys(DESKTOP_ICON_PNG_SIZES), "icon.icns", "icon.ico"];

/** Returns the exact local Tauri CLI arguments used for deterministic icon generation. */
export function tauriIconArguments(outputDirectory) {
  return ["icon", APPLICATION_ICON_SOURCE, "--output", outputDirectory];
}

/** Reads the fixed PNG signature and IHDR fields needed by the asset contract. */
export function inspectPng(bytes) {
  if (bytes.length < PNG_IHDR_MINIMUM_BYTES || !bytes.subarray(0, PNG_SIGNATURE.length).equals(PNG_SIGNATURE)) {
    throw new Error("Application icon is not a valid PNG.");
  }
  if (bytes.subarray(12, 16).toString("ascii") !== "IHDR") {
    throw new Error("Application icon PNG does not begin with IHDR.");
  }
  const colorType = bytes[25];
  return {
    bitDepth: bytes[24],
    colorType,
    hasAlpha: colorType === 4 || colorType === 6,
    height: bytes.readUInt32BE(20),
    width: bytes.readUInt32BE(16),
  };
}

/** Verifies the ICNS container signature and declared byte length. */
function verifyIcns(bytes) {
  if (bytes.length < 8 || bytes.subarray(0, 4).toString("ascii") !== "icns" || bytes.readUInt32BE(4) !== bytes.length) {
    throw new Error("Generated macOS icon is not a complete ICNS container.");
  }
}

/** Sorts complete ICNS entries by type so locked generation has stable bytes across invocations. */
export function canonicalizeIcns(bytes) {
  verifyIcns(bytes);
  const entries = [];
  let offset = 8;
  while (offset < bytes.length) {
    if (offset + 8 > bytes.length) throw new Error("Generated ICNS contains a truncated entry header.");
    const entryLength = bytes.readUInt32BE(offset + 4);
    if (entryLength < 8 || offset + entryLength > bytes.length) {
      throw new Error("Generated ICNS contains an invalid entry length.");
    }
    entries.push(bytes.subarray(offset, offset + entryLength));
    offset += entryLength;
  }
  entries.sort((left, right) => {
    const typeOrder = left.subarray(0, 4).compare(right.subarray(0, 4));
    return typeOrder || left.compare(right);
  });
  const header = Buffer.alloc(8);
  header.write("icns", 0, 4, "ascii");
  header.writeUInt32BE(bytes.length, 4);
  return Buffer.concat([header, ...entries]);
}

/** Verifies the ICO directory header contains at least one icon image. */
function verifyIco(bytes) {
  if (bytes.length < 6 || bytes.readUInt16LE(0) !== 0 || bytes.readUInt16LE(2) !== 1 || bytes.readUInt16LE(4) < 1) {
    throw new Error("Generated Windows icon is not a valid ICO container.");
  }
}

/** Verifies required checked-in sizes, alpha, platform containers, and the exact approved favicon. */
export async function verifyCheckedInApplicationIcons(repositoryRoot = REPOSITORY_ROOT) {
  const iconDirectory = join(repositoryRoot, ICON_OUTPUT_DIRECTORY);
  for (const [name, expectedSize] of Object.entries(DESKTOP_ICON_PNG_SIZES)) {
    const metadata = inspectPng(await readFile(join(iconDirectory, name)));
    if (
      metadata.width !== expectedSize ||
      metadata.height !== expectedSize ||
      metadata.bitDepth !== 8 ||
      !metadata.hasAlpha
    ) {
      throw new Error(`${name} must be a ${expectedSize}x${expectedSize} 8-bit PNG with alpha.`);
    }
  }

  verifyIcns(await readFile(join(iconDirectory, "icon.icns")));
  verifyIco(await readFile(join(iconDirectory, "icon.ico")));

  const favicon = await readFile(join(repositoryRoot, FAVICON_OUTPUT));
  const faviconSource = await readFile(join(repositoryRoot, FAVICON_SOURCE));
  if (!favicon.equals(faviconSource)) throw new Error("The WebView favicon differs from its approved source.");
  const faviconMetadata = inspectPng(favicon);
  if (faviconMetadata.width !== 64 || faviconMetadata.height !== 64 || !faviconMetadata.hasAlpha) {
    throw new Error("The WebView favicon must be a 64x64 PNG with alpha.");
  }

  return {
    favicon: { height: faviconMetadata.height, width: faviconMetadata.width },
    icns: true,
    ico: true,
    pngCount: Object.keys(DESKTOP_ICON_PNG_SIZES).length,
  };
}

/** Runs the repository-locked Tauri CLI without shell interpolation or network access. */
function generateInto(outputDirectory) {
  const result = spawnSync(process.execPath, [TAURI_CLI, ...tauriIconArguments(outputDirectory)], {
    cwd: REPOSITORY_ROOT,
    encoding: "utf8",
  });
  if (result.error || result.status !== 0) {
    throw new Error("The locked Tauri CLI failed to generate Bottie application icons.");
  }
}

/** Copies only desktop outputs plus the reviewed favicon into their checked-in locations. */
async function installGeneratedIcons(generatedDirectory) {
  const iconDirectory = join(REPOSITORY_ROOT, ICON_OUTPUT_DIRECTORY);
  await mkdir(iconDirectory, { recursive: true });
  await Promise.all(
    DESKTOP_ICON_FILES.map((name) => copyFile(join(generatedDirectory, name), join(iconDirectory, name))),
  );
  await copyFile(join(REPOSITORY_ROOT, FAVICON_SOURCE), join(REPOSITORY_ROOT, FAVICON_OUTPUT));
}

/** Proves checked-in desktop outputs exactly match a fresh locked generation. */
async function compareGeneratedIcons(generatedDirectory) {
  for (const name of DESKTOP_ICON_FILES) {
    const generated = await readFile(join(generatedDirectory, name));
    const checkedIn = await readFile(join(REPOSITORY_ROOT, ICON_OUTPUT_DIRECTORY, name));
    if (!generated.equals(checkedIn)) throw new Error(`${name} differs from locked icon generation.`);
  }
}

/** Generates to a process-owned temporary directory and always removes mobile/unselected outputs. */
async function withGeneratedIcons(action) {
  const temporaryDirectory = await mkdtemp(join(tmpdir(), "bottie-application-icons-"));
  try {
    generateInto(temporaryDirectory);
    const icnsPath = join(temporaryDirectory, "icon.icns");
    await writeFile(icnsPath, canonicalizeIcns(await readFile(icnsPath)));
    await action(temporaryDirectory);
  } finally {
    await rm(temporaryDirectory, { recursive: true, force: true });
  }
}

/** Implements the explicit generate/check command-line contract. */
async function main(argument) {
  if (argument === "--generate") {
    await withGeneratedIcons(installGeneratedIcons);
    await verifyCheckedInApplicationIcons();
    return;
  }
  if (argument === "--check") {
    await verifyCheckedInApplicationIcons();
    await withGeneratedIcons(compareGeneratedIcons);
    return;
  }
  throw new Error("Usage: node scripts/application-icons.mjs --generate|--check");
}

if (resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) {
  main(process.argv[2]).catch((error) => {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  });
}
