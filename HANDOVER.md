# Bottie handover

Last verified: 2026-09-17

## Start here

Start from the draft PR for `codex/linux-image-worker-ucc-source-blocker`. Read `ROADMAP.md` Milestone 8.4,
`docs/local-image-linux-worker-bundle.md`, `local-image-worker/diffusers_runtime_license_sources.py`, and
`local-image-worker/diffusers_runtime_native.py`.

## Current state

- The retained DGX image is
  `sha256:cded2049f9dbff513406052a191af2588476056d795468c4aacb7814aca5666c`; its Python and process-map trace
  digests are unchanged. The checked-in closure remains complete at 84 components and blocked on 106 licence-only
  findings because the five-source closure has not been collected.
- Four authoritative external archives bind exact licence bytes for SentencePiece, tokenizers, NVPL BLAS, and NVPL
  LAPACK. The NVPL sources additionally require their installed full-version libraries to match regular archive
  members byte-for-byte. No source adds a licence expression or review conclusion.
- `native:hpcx-ucc@1.5.0+ec95a0a96fc7220e1627157439c508cafc82274e` remains the sole identity without licence
  bytes. The full commit is absent from the official OpenUCX repository and GitHub's public full-hash commit search.
- NVIDIA's public `HPCX:ucc_hpcx_v2.24` component and notice PDFs contain only `rdma-core`; they do not identify UCC,
  the `ec95a0a...` revision, or the installed library. The exact v2.24.1 PDF names are unavailable. These documents
  cannot replace component-owned exact licence bytes.
- NVIDIA's official downloader identifies
  `hpcx-v2.24.1-gcc-doca_ofed-ubuntu24.04-cuda13-aarch64.tbz` with reported size `346M` and SHA-256
  `0bc5c26a4f0ca98fd6292aac9d51cc2a4ee3277d38d4011b64eafd133be633b4`, but routes acquisition through an
  EULA. It was not downloaded because accepting new terms was not authorized.
- The configured DGX has no existing UCC/HPC-X archive in the user or temporary paths. A network-disabled read-only
  inspection reconfirmed that the retained image has `/opt/hpcx/ucc/lib/libucc.so.1.0.0` but no UCC licence or source
  document. The image history exposes only the installed `/opt/hpcx` copy, not an authoritative archive.
- `licenseReviewed`, `assemblyEligible`, and `distributionReviewed` remain false. No source spec, closure record,
  review manifest, bundle, accepted Linux profile, or executable was added or changed.

## Validation

The official OpenUCX commit endpoint rejected the full revision and GitHub's public full-hash search returned zero
matches. The three public NVIDIA v2.24 PDFs were measured locally and inspected as text; the two UCC-specific files are
17,354 and 24,114 bytes and name only `rdma-core`. The DGX archive search and retained-image inspection were read-only;
the container was launched with networking disabled, a read-only root, no capabilities, and no-new-privileges. No
product source changed. All 63 local worker Python tests pass. Prettier, Svelte diagnostics, all 407 active
frontend/script tests (3 skipped), and the production build pass. Cargo formatting/checking pass; the host-local Rust
suite passes 649 library tests (37 ignored) plus all integration and doc tests after the expected sandbox loopback
denials. Unrelated untracked logo-kit, screenshot, and Linux public-key files remain untouched.

## Next slice

Resolve only `native:hpcx-ucc@1.5.0+ec95a0a96fc7220e1627157439c508cafc82274e` after an exact archive is supplied
outside the repository by someone who has independently handled any required terms:

1. require the exact authoritative archive identity and reject nearby tags, releases, generic notices, or
   project-level licence assertions;
2. measure one component-owned licence member and require `/opt/hpcx/ucc/lib/libucc.so.1.0.0` to match its exact
   regular archive member before adding a fixed source spec and regression tests;
3. on the named DGX Spark, rerun the same retained image and trace twice with all five source archives, require
   byte-identical path-free output and zero `missing-license-bytes` blockers, and keep all expression/review gates
   closed.

If the exact archive does not contain both required members, stop with that blocker. Do not accept terms on the user's
behalf, add a downloader, infer a licence expression, create a review manifest, weaken complete coverage, or add a
Linux profile, executable, bundle assembly, hardware probe, worker import/download, app execution, UI/IPC field,
Docker product dependency, cloud fallback, local editing, guessed Qwen Image 2.0 weights, signing, release, Store, or
updater publication.
