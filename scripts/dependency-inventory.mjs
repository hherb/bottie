#!/usr/bin/env node

/** Builds Bottie's deterministic, offline dependency and licence inventory. */

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const REPOSITORY_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const INVENTORY_FILE = "dependency-inventory.json";
const RUST_MANIFEST = "src-tauri/Cargo.toml";
const RUST_TARGETS = [
  "aarch64-apple-darwin",
  "x86_64-apple-darwin",
  "x86_64-pc-windows-msvc",
  "x86_64-unknown-linux-gnu",
];
const REVIEWED_DATE = "2026-08-24";
const APPLICATION_ASSETS = [
  "src-tauri/icons/32x32.png",
  "src-tauri/icons/64x64.png",
  "src-tauri/icons/128x128.png",
  "src-tauri/icons/128x128@2x.png",
  "src-tauri/icons/icon.icns",
  "src-tauri/icons/icon.ico",
  "static/favicon.png",
];
const APPLICATION_ASSET_SOURCES = [
  "assets/bottie-logo-kit/README.md",
  "assets/bottie-logo-kit/bottie-icon-512.png",
  "assets/bottie-logo-kit/bottie-mark-color.svg",
  "assets/bottie-logo-kit/favicon-64.png",
];
const SPDX_LICENCE_SOURCES = [
  "third-party/spdx-3.28.0/exceptions/LLVM-exception.txt",
  "third-party/spdx-3.28.0/licenses/Apache-2.0.txt",
  "third-party/spdx-3.28.0/licenses/BSD-2-Clause.txt",
  "third-party/spdx-3.28.0/licenses/BSD-3-Clause.txt",
  "third-party/spdx-3.28.0/licenses/BSL-1.0.txt",
  "third-party/spdx-3.28.0/licenses/CDLA-Permissive-2.0.txt",
  "third-party/spdx-3.28.0/licenses/ISC.txt",
  "third-party/spdx-3.28.0/licenses/MIT-0.txt",
  "third-party/spdx-3.28.0/licenses/MIT.txt",
  "third-party/spdx-3.28.0/licenses/MPL-2.0.txt",
  "third-party/spdx-3.28.0/licenses/Python-2.0.txt",
  "third-party/spdx-3.28.0/licenses/Unicode-3.0.txt",
  "third-party/spdx-3.28.0/licenses/Zlib.txt",
];
const HASHED_INPUTS = [
  "package.json",
  "package-lock.json",
  RUST_MANIFEST,
  "src-tauri/Cargo.lock",
  "src-tauri/tauri.conf.json",
  "src-tauri/src/semantic_indexer.rs",
  "scripts/dependency-inventory.mjs",
  "scripts/application-icons.mjs",
  "scripts/release-candidate.mjs",
  "scripts/release-candidate-runtime.mjs",
  "scripts/release-assets.mjs",
  "scripts/third-party-notices.mjs",
  "scripts/macos-package.mjs",
  "scripts/windows-package.mjs",
  "scripts/windows-signature.mjs",
  "scripts/windows-distribution.mjs",
  "scripts/windows-store.mjs",
  "scripts/linux-package.mjs",
  "LICENSE",
  "MODEL-NOTICE.txt",
  "THIRD-PARTY-NOTICES.txt",
  "runtime-assets.json",
  "third-party/onnxruntime-1.28.0/LICENSE",
  "third-party/onnxruntime-1.28.0/ThirdPartyNotices.txt",
  "third-party/package-licence-texts.json",
  ...SPDX_LICENCE_SOURCES,
  ...APPLICATION_ASSET_SOURCES,
  ...APPLICATION_ASSETS,
];
const REVIEW_REQUIRED_LICENCES = ["MPL-2.0", "Python-2.0"];
const REVIEWED_NOTICE_PACKAGES = new Set([
  "cargo:cssparser@0.36.0",
  "cargo:cssparser-macros@0.6.1",
  "cargo:dtoa-short@0.3.5",
  "cargo:option-ext@0.2.0",
  "cargo:selectors@0.36.1",
  "npm:argparse@3.0.0",
]);
const NO_NOTICE_ALTERNATIVES = ["0BSD", "CC0-1.0", "Unlicense"];
const RECOGNISED_NOTICE_LICENCES = [
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "BSL-1.0",
  "CDLA-Permissive-2.0",
  "ISC",
  "LLVM-exception",
  "MIT-0",
  "MIT",
  "Unicode-3.0",
  "Zlib",
];

const SECURITY_RELEVANT_FEATURES = [
  {
    package: "reqwest",
    manifestSelection: "default-features=false; json,rustls,stream",
    consequence: "Provider, Web, and Localmail HTTP use Rustls rather than Reqwest's native-TLS default.",
  },
  {
    package: "rustls",
    manifestSelection: "default-features=false; ring,std",
    consequence: "Bottie fixes the Rustls crypto provider to ring.",
  },
  {
    package: "rusqlite",
    manifestSelection: "backup,bundled",
    consequence: "SQLite is compiled into Bottie and the online-backup API is enabled.",
  },
  {
    package: "fastembed",
    manifestSelection: "default-features=false; hf-hub-rustls-tls,ort-download-binaries-rustls-tls",
    consequence: "Model retrieval and the checksum-pinned ONNX Runtime build input use Rustls.",
  },
  {
    package: "image",
    manifestSelection: "default-features=false; jpeg,png",
    consequence: "Only the two reviewed image decoders are selected.",
  },
  {
    package: "zip",
    manifestSelection: "default-features=false; deflate-flate2",
    consequence: "Portable exports and DOCX parsing select only DEFLATE support.",
  },
  {
    package: "lopdf",
    manifestSelection: "default-features=false",
    consequence: "Optional PDF encryption and time support are not selected.",
  },
  {
    package: "keyring",
    manifestSelection: "v1 plus crate defaults",
    consequence: "The compatibility API and target-native credential stores are selected.",
  },
  {
    package: "objc2-local-authentication",
    manifestSelection: "LAContext,block2; macOS only",
    consequence: "The native biometric boundary is compiled only on macOS.",
  },
];

/** Returns the review classification for one declared licence expression. */
export function classifyLicence(licence) {
  if (!licence?.trim()) return "unknown";
  if (REVIEW_REQUIRED_LICENCES.some((item) => licence.includes(item))) return "review-required";
  const hasNoNoticeChoice =
    (licence.includes(" OR ") || licence.includes("/")) &&
    NO_NOTICE_ALTERNATIVES.some((item) => licence.includes(item));
  if (licence === "CC0-1.0" || licence === "0BSD" || licence === "Unlicense" || hasNoNoticeChoice) {
    return "compatible";
  }
  const remainder = RECOGNISED_NOTICE_LICENCES.reduce((value, item) => value.replaceAll(item, ""), licence)
    .replaceAll(/\b(?:AND|OR|WITH)\b/g, "")
    .replaceAll(/[()/\s-]/g, "");
  return remainder ? "review-required" : "notice-required";
}

/** Applies a completed human review only to one exact ecosystem/name/version identity. */
export function classifyReviewedLicence(ecosystem, name, version, licence) {
  const classification = classifyLicence(licence);
  if (classification === "review-required" && REVIEWED_NOTICE_PACKAGES.has(`${ecosystem}:${name}@${version}`)) {
    return "notice-required";
  }
  return classification;
}

/** Parses Cargo tree's stable pipe-delimited package, licence, and feature output. */
export function parseCargoTree(output) {
  const packages = new Map();
  for (const rawLine of output.split("\n")) {
    const line = rawLine.trim();
    if (!line) continue;
    const [packageText, licence = "", featureText = ""] = line.split("|");
    const match = packageText.match(/^(\S+)\s+v(\S+)/);
    if (!match || match[1] === "bottie") continue;
    const key = `${match[1]}@${match[2]}`;
    const current = packages.get(key) ?? {
      name: match[1],
      version: match[2],
      licence,
      features: new Set(),
    };
    for (const feature of featureText.replace(/\s+\(\*\)$/, "").split(",")) {
      if (feature) current.features.add(feature);
    }
    if (!current.licence && licence) current.licence = licence;
    packages.set(key, current);
  }
  return [...packages.values()]
    .map((entry) => ({ ...entry, features: [...entry.features].sort() }))
    .sort(comparePackages);
}

/** Merges resolved Rust graphs for the reviewed targets without inventing unbuilt platforms. */
export function mergeRustInventories(targetInventories) {
  const merged = new Map();
  for (const inventory of targetInventories) {
    const runtimeKeys = new Set(inventory.runtime.map(packageKey));
    for (const entry of inventory.complete) {
      const key = packageKey(entry);
      const current = merged.get(key) ?? {
        name: entry.name,
        version: entry.version,
        licence: entry.licence,
        classification: classifyReviewedLicence("cargo", entry.name, entry.version, entry.licence),
        direct: false,
        scope: "build-only",
        targets: new Set(),
        features: new Set(),
        source: `https://crates.io/crates/${encodeURIComponent(entry.name)}/${encodeURIComponent(entry.version)}`,
      };
      current.direct ||= inventory.direct.has(key);
      if (runtimeKeys.has(key)) current.scope = "runtime-graph";
      current.targets.add(inventory.target);
      for (const feature of entry.features) current.features.add(feature);
      merged.set(key, current);
    }
  }
  return [...merged.values()]
    .map((entry) => ({
      ...entry,
      targets: [...entry.targets].sort(),
      features: [...entry.features].sort(),
    }))
    .sort(comparePackages);
}

/** Converts package-lock v3 entries into exact path-aware npm inventory records. */
export function parseNpmLock(lock) {
  const root = lock.packages?.[""] ?? {};
  const runtimeDirect = new Set(Object.keys(root.dependencies ?? {}));
  const buildDirect = new Set(Object.keys(root.devDependencies ?? {}));
  return Object.entries(lock.packages ?? {})
    .filter(([path]) => path)
    .map(([path, metadata]) => {
      const name = npmPackageName(path);
      return {
        name,
        version: metadata.version ?? "unknown",
        licence: metadata.license ?? "",
        classification: classifyReviewedLicence("npm", name, metadata.version ?? "unknown", metadata.license ?? ""),
        direct: path === `node_modules/${name}` && (runtimeDirect.has(name) || buildDirect.has(name)),
        scope: metadata.dev ? "development-install" : "production-install",
        optional: Boolean(metadata.optional),
        peer: Boolean(metadata.peer),
        path,
        source: metadata.resolved ?? null,
        integrity: metadata.integrity ?? null,
      };
    })
    .sort((left, right) => comparePackages(left, right) || left.path.localeCompare(right.path));
}

/** Builds the complete inventory from locked, offline package metadata and reviewed local assets. */
export function buildInventory(repositoryRoot = REPOSITORY_ROOT) {
  const targetInventories = RUST_TARGETS.map((target) => ({
    target,
    runtime: cargoTree(repositoryRoot, target, "normal"),
    complete: cargoTree(repositoryRoot, target, "normal,build"),
    direct: new Set(cargoTree(repositoryRoot, target, "normal,build", "1").map(packageKey)),
  }));
  const packageLock = JSON.parse(readFileSync(join(repositoryRoot, "package-lock.json"), "utf8"));
  const cargoLock = readFileSync(join(repositoryRoot, "src-tauri/Cargo.lock"), "utf8");
  const rust = mergeRustInventories(targetInventories);
  const npm = parseNpmLock(packageLock);
  const assets = reviewedAssets(repositoryRoot);
  return {
    schemaVersion: 1,
    reviewedDate: REVIEWED_DATE,
    scope: {
      rustTargets: RUST_TARGETS,
      rustLockPackages: (cargoLock.match(/^\[\[package\]\]$/gm) ?? []).length,
      npmLockPackages: Object.keys(packageLock.packages ?? {}).filter(Boolean).length,
      limitation:
        "Rust entries are the union of locked macOS arm64/x64, Windows x64, and Linux x64 normal and build graphs. " +
        "The Cargo.lock count remains a conservative superset for architectures outside those four reviewed targets.",
    },
    inputs: Object.fromEntries(HASHED_INPUTS.map((path) => [path, sha256(join(repositoryRoot, path))])),
    securityRelevantFeatures: SECURITY_RELEVANT_FEATURES,
    assets,
    summary: classificationSummary([...rust, ...npm, ...assets]),
    rust,
    npm,
  };
}

/** Runs Cargo tree without downloads or package build scripts. */
function cargoTree(repositoryRoot, target, edges, depth) {
  const arguments_ = [
    "tree",
    "--locked",
    "--offline",
    "--manifest-path",
    RUST_MANIFEST,
    "--target",
    target,
    "--edges",
    edges,
    "--prefix",
    "none",
    "--format",
    "{p}|{l}|{f}",
  ];
  if (depth) arguments_.push("--depth", depth);
  return parseCargoTree(
    execFileSync("cargo", arguments_, { cwd: repositoryRoot, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }),
  );
}

/** Records non-package code and data selected by the current app configuration. */
function reviewedAssets(repositoryRoot) {
  return [
    {
      name: "Bottie application icons and browser favicon",
      version: "repository snapshot",
      licence: "MIT",
      classification: "compatible",
      delivery: "Bundled in platform applications or the compiled frontend.",
      source: "assets/bottie-logo-kit/README.md",
      generationSources: Object.fromEntries(
        APPLICATION_ASSET_SOURCES.map((path) => [path, sha256(join(repositoryRoot, path))]),
      ),
      files: Object.fromEntries(APPLICATION_ASSETS.map((path) => [path, sha256(join(repositoryRoot, path))])),
    },
    {
      name: "SQLite amalgamation",
      version: "3.51.1",
      licence: "Public Domain",
      classification: "compatible",
      delivery: "Statically compiled by libsqlite3-sys through rusqlite's bundled feature.",
      source: "https://sqlite.org/copyright.html",
    },
    {
      name: "sqlite-vec native extension",
      version: "0.1.7-alpha.10",
      licence: "MIT OR Apache-2.0",
      classification: "notice-required",
      delivery: "Statically compiled into Bottie's Rust binary.",
      source: "https://crates.io/crates/sqlite-vec/0.1.7-alpha.10",
    },
    {
      name: "Microsoft ONNX Runtime",
      version: "1.28.0 selected by ort-sys 2.0.0-rc.13",
      licence: "MIT plus upstream third-party notices",
      classification: "notice-required",
      delivery:
        "The three supported release archives are selected and hash-checked by ort-sys at build time. " +
        "runtime-assets.json binds their identities to the version-matched licence and upstream notice files.",
      source: "third-party/onnxruntime-1.28.0/LICENSE",
    },
    {
      name: "EmbeddingGemma 300M Q4 ONNX model",
      version: "onnx-community/embeddinggemma-300m-ONNX@75a84c732f1884df76bec365346230e32f582c82",
      licence: "Gemma Terms of Use",
      classification: "notice-required",
      delivery:
        "Downloaded at runtime into Bottie's application cache; not bundled in this repository or application. " +
        "runtime-assets.json pins the revision and all six files, and MODEL-NOTICE.txt records the reviewed terms.",
      source: "runtime-assets.json",
    },
    {
      name: "macOS system frameworks and WebKit",
      version: "Operating-system supplied",
      licence: "Platform terms",
      classification: "compatible",
      delivery: "Dynamically supplied by macOS and not redistributed in the application bundle.",
      source: "https://developer.apple.com/support/terms/",
    },
  ];
}

/** Counts review categories independently for Rust, npm, and reviewed assets. */
function classificationSummary(entries) {
  const summary = { compatible: 0, "notice-required": 0, unknown: 0, "review-required": 0 };
  for (const entry of entries) summary[entry.classification] += 1;
  return summary;
}

/** Returns one stable package identity. */
function packageKey(entry) {
  return `${entry.name}@${entry.version}`;
}

/** Sorts package records by name and then exact resolved version. */
function comparePackages(left, right) {
  return left.name.localeCompare(right.name) || left.version.localeCompare(right.version);
}

/** Extracts a scoped or unscoped package name from an npm lock path. */
function npmPackageName(path) {
  const marker = "node_modules/";
  const index = path.lastIndexOf(marker);
  return path.slice(index + marker.length);
}

/** Returns one lowercase SHA-256 value for a deterministic local input. */
function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

/** Writes a generated snapshot or fails when the committed snapshot has drifted. */
function main() {
  const path = join(REPOSITORY_ROOT, INVENTORY_FILE);
  const generated = `${JSON.stringify(buildInventory(), null, 2)}\n`;
  if (process.argv.includes("--check")) {
    if (!existsSync(path) || readFileSync(path, "utf8") !== generated) {
      throw new Error(`${relative(REPOSITORY_ROOT, path)} is stale; run node scripts/dependency-inventory.mjs.`);
    }
    console.log(`[bottie] ${INVENTORY_FILE} matches the locked offline dependency graph.`);
    return;
  }
  writeFileSync(path, generated);
  console.log(`[bottie] wrote ${INVENTORY_FILE}`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(`[bottie] ${error instanceof Error ? error.message : "Dependency inventory failed."}`);
    process.exitCode = 1;
  }
}
