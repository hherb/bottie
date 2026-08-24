#!/usr/bin/env node

/** Generates Bottie's deterministic distributable third-party notice bundle. */

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const REPOSITORY_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const OUTPUT_PATH = "THIRD-PARTY-NOTICES.txt";
const ONNX_RUNTIME_LICENCE_PATH = "third-party/onnxruntime-1.28.0/LICENSE";
const ONNX_RUNTIME_NOTICES_PATH = "third-party/onnxruntime-1.28.0/ThirdPartyNotices.txt";
const DIVIDER = "=".repeat(80);
const LICENCE_FILE_PATTERN = /^(?:licen[cs]e|copying|copyright|notice)(?:[-._].*)?$/i;
const SPDX_VERSION = "v3.28.0";
const SPDX_LICENCES = [
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "BSL-1.0",
  "CDLA-Permissive-2.0",
  "ISC",
  "MIT-0",
  "MIT",
  "MPL-2.0",
  "Python-2.0",
  "Unicode-3.0",
  "Zlib",
];
const SPDX_EXCEPTIONS = ["LLVM-exception"];

/** Builds one sorted notice document and shares identical licence bodies between package identities. */
export function buildThirdPartyNotices({ packages, onnxRuntimeLicence, onnxRuntimeNotices }) {
  const texts = new Map();
  for (const package_ of [...packages].sort(comparePackages)) {
    if (package_.texts.length === 0) {
      throw new Error(`${package_.ecosystem}:${package_.name}@${package_.version} is missing licence text.`);
    }
    for (const rawText of package_.texts) {
      const text = normalizeText(rawText);
      const key = createHash("sha256").update(text).digest("hex");
      const entry = texts.get(key) ?? { packages: new Set(), text };
      entry.packages.add(`${package_.ecosystem}:${package_.name}@${package_.version}`);
      texts.set(key, entry);
    }
  }
  const packageSections = [...texts.values()]
    .map((entry) => ({ ...entry, packages: [...entry.packages].sort() }))
    .sort((left, right) => left.packages[0].localeCompare(right.packages[0]))
    .map((entry) => `Used by: ${entry.packages.join(", ")}\n\n${entry.text}`)
    .join(`\n${DIVIDER}\n\n`);
  return normalizeText(
    `Bottie third-party notices\n\n` +
      `This file accompanies Bottie's application packages. Package identities are exact locked versions. ` +
      `Identical licence texts are emitted once and list every package that uses them. When an upstream package ` +
      `archive omits its workspace licence file, the declared expression is paired with the canonical SPDX 3.28.0 ` +
      `text and the package's authoritative source URL.\n\n` +
      `${DIVIDER}\n\n${packageSections}\n${DIVIDER}\n\n` +
      `Microsoft ONNX Runtime 1.28.0 licence\n\n${onnxRuntimeLicence}\n${DIVIDER}\n\n` +
      `Microsoft ONNX Runtime 1.28.0 upstream third-party notices\n\n${onnxRuntimeNotices}`,
  );
}

/** Collects exact top-level distributable licence files for every notice-required locked package. */
export function collectPackageLicenceTexts(inventory, repositoryRoot = REPOSITORY_ROOT, cargoHome = null) {
  const cargoRoot = cargoHome ?? join(homedir(), ".cargo");
  const registryRoot = join(cargoRoot, "registry", "src");
  const registrySources = readdirSync(registryRoot)
    .map((name) => join(registryRoot, name))
    .filter((path) => statSync(path).isDirectory());
  const collected = new Map();
  const missing = [];
  const entries = [
    ...inventory.rust.map((entry) => ({ ...entry, ecosystem: "cargo" })),
    ...inventory.npm.map((entry) => ({ ...entry, ecosystem: "npm" })),
  ].filter((entry) => entry.classification === "notice-required");
  for (const entry of entries) {
    const identity = `${entry.ecosystem}:${entry.name}@${entry.version}`;
    const roots =
      entry.ecosystem === "cargo"
        ? registrySources.map((root) => join(root, `${entry.name}-${entry.version}`)).filter(existsSync)
        : npmLicenceRoots(entry, repositoryRoot);
    const exactTexts = roots.flatMap(readLicenceTexts);
    const texts = exactTexts.length > 0 ? exactTexts : declaredLicenceTexts(entry, repositoryRoot);
    if (texts.length === 0) {
      missing.push(identity);
      continue;
    }
    const current = collected.get(identity) ?? new Set();
    for (const text of texts) current.add(normalizeText(text));
    collected.set(identity, current);
  }
  if (missing.length > 0) throw new Error(`Missing licence text for: ${[...new Set(missing)].sort().join(", ")}`);
  const texts = new Map();
  const packages = {};
  for (const [identity, sources] of [...collected.entries()].sort(([left], [right]) => left.localeCompare(right))) {
    packages[identity] = [...sources]
      .map((source) => {
        const hash = createHash("sha256").update(source).digest("hex");
        texts.set(hash, source);
        return hash;
      })
      .sort();
  }
  return {
    schemaVersion: 1,
    spdxFallbackVersion: "3.28.0",
    packages,
    texts: Object.fromEntries([...texts.entries()].sort(([left], [right]) => left.localeCompare(right))),
  };
}

/** Includes the matching parent package for platform-binary npm packages that intentionally omit duplicate licences. */
function npmLicenceRoots(entry, repositoryRoot) {
  const roots = [join(repositoryRoot, entry.path)];
  const parent = entry.name.startsWith("@esbuild/")
    ? "esbuild"
    : entry.name.startsWith("@rollup/rollup-")
      ? "rollup"
      : entry.name.startsWith("@tauri-apps/cli-")
        ? "@tauri-apps/cli"
        : entry.name.startsWith("@napi-rs/lzma-")
          ? "@napi-rs/lzma"
          : null;
  if (parent) roots.push(join(repositoryRoot, "node_modules", ...parent.split("/")));
  return roots.filter(existsSync);
}

/** Supplies pinned canonical SPDX text when a published workspace package omitted its shared licence file. */
function declaredLicenceTexts(entry, repositoryRoot) {
  const identifiers = [...SPDX_LICENCES, ...SPDX_EXCEPTIONS].filter((identifier) => entry.licence.includes(identifier));
  if (identifiers.length === 0) return [];
  const provenance = `Declared licence: ${entry.licence}\nAuthoritative package source: ${entry.source}\n`;
  return identifiers.map((identifier) => {
    const kind = SPDX_EXCEPTIONS.includes(identifier) ? "exceptions" : "licenses";
    const path = join(repositoryRoot, "third-party", "spdx-3.28.0", kind, `${identifier}.txt`);
    return `${provenance}\n${readFileSync(path, "utf8")}`;
  });
}

/** Reads exact bounded top-level licence, copying, copyright, and notice files from one package source. */
function readLicenceTexts(root) {
  return readdirSync(root, { withFileTypes: true })
    .filter((entry) => entry.isFile() && LICENCE_FILE_PATTERN.test(entry.name))
    .sort((left, right) => left.name.localeCompare(right.name))
    .map((entry) => readFileSync(join(root, entry.name), "utf8"));
}

/** Uses ecosystem, package name, and exact version as the stable notice order. */
function comparePackages(left, right) {
  return (
    left.ecosystem.localeCompare(right.ecosystem) ||
    left.name.localeCompare(right.name) ||
    left.version.localeCompare(right.version)
  );
}

/** Normalizes generated text to LF with exactly one trailing newline. */
function normalizeText(source) {
  return `${source
    .replaceAll("\r\n", "\n")
    .replace(/[ \t]+$/gm, "")
    .trimEnd()}\n`;
}

/** Refreshes canonical declared-licence fallbacks from one immutable SPDX release tag. */
async function refreshSpdxSources() {
  for (const identifier of [...SPDX_LICENCES, ...SPDX_EXCEPTIONS]) {
    const kind = SPDX_EXCEPTIONS.includes(identifier) ? "exceptions" : "licenses";
    const url = `https://raw.githubusercontent.com/spdx/license-list-data/${SPDX_VERSION}/text/${identifier}.txt`;
    const response = await fetch(url, { redirect: "error" });
    if (!response.ok) throw new Error(`Could not refresh SPDX ${identifier}.`);
    const path = join(REPOSITORY_ROOT, "third-party", "spdx-3.28.0", kind, `${identifier}.txt`);
    const directory = dirname(path);
    if (!existsSync(directory)) mkdirSync(directory, { recursive: true });
    writeFileSync(path, normalizeText(await response.text()));
  }
}

/** Generates or byte-checks the repository notice after package-source collection is available. */
async function main() {
  if (process.argv.includes("--refresh-spdx")) await refreshSpdxSources();
  const inventory = JSON.parse(readFileSync(join(REPOSITORY_ROOT, "dependency-inventory.json"), "utf8"));
  const packageTextsPath = join(REPOSITORY_ROOT, "third-party/package-licence-texts.json");
  if (process.argv.includes("--collect")) {
    const collected = collectPackageLicenceTexts(inventory);
    writeFileSync(packageTextsPath, `${JSON.stringify(collected, null, 2)}\n`);
  }
  const packageTexts = JSON.parse(readFileSync(packageTextsPath, "utf8"));
  const packages = [
    ...inventory.rust.map((entry) => ({ ...entry, ecosystem: "cargo" })),
    ...inventory.npm.map((entry) => ({ ...entry, ecosystem: "npm" })),
  ]
    .filter((entry) => entry.classification === "notice-required")
    .map((entry) => ({
      ...entry,
      texts: (packageTexts.packages[`${entry.ecosystem}:${entry.name}@${entry.version}`] ?? []).map(
        (hash) => packageTexts.texts[hash],
      ),
    }));
  const generated = buildThirdPartyNotices({
    packages,
    onnxRuntimeLicence: readFileSync(join(REPOSITORY_ROOT, ONNX_RUNTIME_LICENCE_PATH), "utf8"),
    onnxRuntimeNotices: readFileSync(join(REPOSITORY_ROOT, ONNX_RUNTIME_NOTICES_PATH), "utf8"),
  });
  const path = join(REPOSITORY_ROOT, OUTPUT_PATH);
  if (process.argv.includes("--check")) {
    if (!existsSync(path) || readFileSync(path, "utf8") !== generated) {
      throw new Error(`${OUTPUT_PATH} is stale; run node scripts/third-party-notices.mjs --write.`);
    }
    console.log(`[bottie] ${OUTPUT_PATH} matches the reviewed package licence texts.`);
    return;
  }
  writeFileSync(path, generated);
  console.log(`[bottie] wrote ${OUTPUT_PATH}.`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    await main();
  } catch (error) {
    console.error(`[bottie] ${error instanceof Error ? error.message : "Third-party notice generation failed."}`);
    process.exitCode = 1;
  }
}
