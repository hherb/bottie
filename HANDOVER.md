# Bottie handover

Last verified: 2026-09-15

## Start here

`main` includes merged PR #173 at `e219644`. Branch `codex/local-image-runtime-proof` completes the bounded Apple M3
Max runtime proof for Milestones 8.3-8.4. Read `docs/local-image-model-package.md`, `ROADMAP.md` Milestones 8.3-8.4,
and `src-tauri/src/local_image_worker/`.

## Completed slice

- The frozen 17,442,350,812-byte q4 package downloaded through the explicit feature-gated proof tool, resumed from
  durable verified prefixes, passed all 18 size/SHA-256 contracts, promoted atomically, and reopened successfully.
  Exact percent-encoded Hugging Face nested cache paths are accepted; invalid HTTP envelopes retain safe prefixes,
  while source, path, and integrity drift still discard them.
- A private Python 3.13.14/PyInstaller 6.16.0 worker built from MLX-Gen 0.18.2 commit
  `fca64a283737c68b67a7bfd88d93f7aa9101a95c`. Its onedir layout explicitly places `mlx.metallib` beside PyInstaller's
  relocated `libmlx.dylib`; the bundle is network-denied by `sandbox-exec` and a Python audit hook.
- The exact Apple M3 Max 128 GB, 512x512, 15-step proof passed: decoded and visually reviewed RGB pixel SHA-256
  `4cd2921c3cf0a43f791cd725cf72da1ff0be04fe97883a9a4b32332cc9cfc0a5`, 29,526,129,448-byte whole-process
  lifetime peak footprint, and 110 ms cooperative cancellation at a denoising boundary.
- `selected_qwen_image_2512_q4_package` freezes the accepted worker executable/bundle hashes and all proof evidence.
  The proof cache, 1.1 GB worker bundle, pinned checkout, and PNG remain ignored local evidence, not app payloads.

## Next slice

Add a pure native local-image availability evaluator that combines exact hardware facts, the selected package's measured
memory requirement, installed worker executable/bundle re-hashing, and verified-cache readiness into path-free states.
Test missing/mismatched bundles, insufficient memory, unsupported architectures, and ready state. Do not add Svelte UI,
auto-download, package the 1.1 GB proof bundle, start generation, or infer support from macOS/model names alone.

Keep local 2512 distinct from hosted Qwen Image 2.0. Do not silently fall back to cloud or reuse Bottie's approved
Python-tool runtime. Do not merge, dispatch workflows, sign, release, publish, distribute, or perform Store work without
separate authorization. Preserve unrelated untracked logo-kit, screenshot, and Linux public-key files.
