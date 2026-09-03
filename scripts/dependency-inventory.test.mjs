import { describe, expect, it } from "vitest";

import {
  classifyLicence,
  classifyReviewedLicence,
  mergeRustInventories,
  parseCargoTree,
  parseNpmLock,
} from "./dependency-inventory.mjs";

describe("dependency inventory", () => {
  it("deduplicates Cargo packages while retaining selected features", () => {
    const packages = parseCargoTree(`
bottie v0.1.0 (/repo/src-tauri)||default
bottie-python-runner v0.1.0 (/repo/python-runner)|MIT|default
serde v1.0.228|MIT OR Apache-2.0|derive,std
serde v1.0.228|MIT OR Apache-2.0|std (*)
tauri-build v2.5.3|Apache-2.0 OR MIT|default
`);

    expect(packages).toEqual([
      {
        name: "serde",
        version: "1.0.228",
        licence: "MIT OR Apache-2.0",
        features: ["derive", "std"],
      },
      {
        name: "tauri-build",
        version: "2.5.3",
        licence: "Apache-2.0 OR MIT",
        features: ["default"],
      },
    ]);
  });

  it("merges target graphs and distinguishes build-only packages", () => {
    const merged = mergeRustInventories([
      {
        component: "bottie-app",
        target: "aarch64-apple-darwin",
        runtime: [{ name: "serde", version: "1.0.228", licence: "MIT", features: ["std"] }],
        complete: [
          { name: "serde", version: "1.0.228", licence: "MIT", features: ["derive", "std"] },
          { name: "tauri-build", version: "2.5.3", licence: "MIT", features: [] },
        ],
        direct: new Set(["serde@1.0.228", "tauri-build@2.5.3"]),
      },
    ]);

    expect(merged).toMatchObject([
      { name: "serde", components: ["bottie-app"], scope: "runtime-graph", direct: true },
      { name: "tauri-build", components: ["bottie-app"], scope: "build-only", direct: true },
    ]);
  });

  it("classifies absent and non-permissive declarations for review", () => {
    expect(classifyLicence("")).toBe("unknown");
    expect(classifyLicence("MPL-2.0")).toBe("review-required");
    expect(classifyLicence("MIT OR Apache-2.0")).toBe("notice-required");
    expect(classifyLicence("CC0-1.0")).toBe("compatible");
    expect(classifyLicence("Unlicense/MIT")).toBe("compatible");
    expect(
      classifyLicence(
        "ISC AND (Apache-2.0 OR ISC) AND Apache-2.0 AND MIT AND BSD-3-Clause AND " + "(Apache-2.0 OR ISC OR MIT-0)",
      ),
    ).toBe("notice-required");
    expect(classifyLicence("proprietary-custom")).toBe("review-required");
  });

  it("applies only exact human-reviewed package licence decisions", () => {
    expect(classifyReviewedLicence("cargo", "cssparser", "0.36.0", "MPL-2.0")).toBe("notice-required");
    expect(classifyReviewedLicence("npm", "argparse", "3.0.0", "Python-2.0")).toBe("notice-required");
    expect(classifyReviewedLicence("cargo", "speech-dispatcher", "0.16.0", "LGPL-2.1 OR MIT OR Apache-2.0")).toBe(
      "notice-required",
    );
    expect(classifyReviewedLicence("cargo", "speech-dispatcher-sys", "0.7.0", "LGPL-2.1 OR MIT OR Apache-2.0")).toBe(
      "notice-required",
    );
    expect(classifyReviewedLicence("cargo", "future-mpl-package", "1.0.0", "MPL-2.0")).toBe("review-required");
    expect(classifyReviewedLicence("cargo", "speech-dispatcher", "0.17.0", "LGPL-2.1 OR MIT OR Apache-2.0")).toBe(
      "review-required",
    );
  });

  it("retains exact npm paths, scopes, integrity, and directness", () => {
    const packages = parseNpmLock({
      packages: {
        "": {
          dependencies: { markdown: "1" },
          devDependencies: { vite: "1" },
        },
        "node_modules/markdown": {
          version: "1.2.3",
          license: "MIT",
          resolved: "https://registry.npmjs.org/markdown/-/markdown-1.2.3.tgz",
          integrity: "sha512-runtime",
        },
        "node_modules/vite": {
          version: "6.0.0",
          license: "MIT",
          resolved: "https://registry.npmjs.org/vite/-/vite-6.0.0.tgz",
          integrity: "sha512-build",
          dev: true,
        },
      },
    });

    expect(packages).toMatchObject([
      { name: "markdown", direct: true, scope: "production-install", integrity: "sha512-runtime" },
      { name: "vite", direct: true, scope: "development-install", integrity: "sha512-build" },
    ]);
  });
});
