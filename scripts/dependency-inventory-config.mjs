/** Defines the reviewed local components and Python-runtime inventory metadata. */

export const RUST_COMPONENTS = [
  { name: "bottie-app", manifest: "src-tauri/Cargo.toml" },
  { name: "python-runner", manifest: "python-runner/Cargo.toml" },
];
export const LOCAL_RUST_PACKAGES = new Set(["bottie", "bottie-python-runner"]);

export const PYTHON_RUNTIME_INVENTORY_INPUTS = [
  "python-runner/Cargo.toml",
  "python-runner/Cargo.lock",
  "python-runner/runtime-manifest.json",
  "src-tauri/tauri.python-development.conf.json",
  "scripts/python-runtime-bundle.mjs",
  "scripts/dependency-inventory-config.mjs",
  ".github/workflows/python-runtime-provenance.yml",
  "third-party/cpython-3.14.7/LICENSE",
];

export const PYTHON_RUNTIME_SECURITY_RELEVANT_FEATURE = {
  package: "wasmtime",
  manifestSelection: "default-features=false; anyhow,cranelift,pulley,runtime,std; exact 45.0.3",
  consequence:
    "The development-only Python helper selects Pulley plus Cranelift without Wasmtime's component-model " +
    "or cache features.",
};

export const PYTHON_RUNTIME_ASSET = {
  name: "CPython/WASI development runtime",
  version: "3.14.7 built with WASI SDK 24",
  licence: "Python-2.0 plus bundled upstream notices",
  classification: "notice-required",
  delivery:
    "Built from the official checksum-pinned CPython source only in the opt-in development provenance workflow. " +
    "The runtime and native helper are inspected in unsigned development packages and are not selected by " +
    "Bottie's default or protected release configurations.",
  source: "third-party/cpython-3.14.7/LICENSE",
};
