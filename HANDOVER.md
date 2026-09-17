# Bottie handover

Last verified: 2026-09-17

## Start here

Start from the draft PR for `codex/linux-image-worker-ucc-archive-mismatch`. Read `ROADMAP.md` Milestone 8.4,
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
- With explicit user authorization for NVIDIA's HPC-X EULA, the exact target archive was downloaded outside the
  repository. Its 362,195,002 bytes match the published SHA-256
  `0bc5c26a4f0ca98fd6292aac9d51cc2a4ee3277d38d4011b64eafd133be633b4`.
- The archive does not contain UCC licence, notice, or source bytes. Its only literal `LICENSE` members belong to
  SHARP. Its regular `ucc/lib/libucc.so.1.0.0` member is 1,119,992 bytes with SHA-256
  `f697c9fd8e1a8d59fd83522eadc4a78f974cc88311f47b128d0ccb410325068e`.
- The retained image's `/opt/hpcx/ucc/lib/libucc.so.1.0.0` is instead 1,119,984 bytes with SHA-256
  `c797c8a60453cdd6b2df48fd2f207ad1f83acd59febdf613a9a190ed9afd080e`. The target-matching archive therefore
  cannot establish provenance or component-owned licence bytes for the exact image binary.
- `licenseReviewed`, `assemblyEligible`, and `distributionReviewed` remain false. No source spec, closure record,
  review manifest, bundle, accepted Linux profile, or executable was added or changed.

## Validation

The exact archive passed byte-count and SHA-256 verification before inspection. Its full member inventory contains the
regular release UCC library but no UCC licence, notice, or source file. A network-disabled, read-only, capability-free
container measured the retained image library on the named DGX and proved the eight-byte size and SHA-256 mismatch. No
product source changed. All 63 local worker Python tests pass. Prettier, Svelte diagnostics, all 407 active
frontend/script tests (3 skipped), and the production build pass. Cargo formatting/checking pass; the host-local Rust
suite passes 649 library tests (37 ignored) plus all integration and doc tests after the expected sandbox loopback
denials. Unrelated untracked logo-kit, screenshot, and Linux public-key files remain untouched.

## Next slice

Resolve only `native:hpcx-ucc@1.5.0+ec95a0a96fc7220e1627157439c508cafc82274e` after an authoritative artifact is
supplied outside the repository:

1. require one component-owned UCC licence member and a regular library member byte-identical to the retained image's
   1,119,984-byte `libucc.so.1.0.0`; reject the verified target archive, nearby tags, generic notices, or project-level
   licence assertions as substitutes;
2. add a fixed source spec and regression tests only after both exact members are available;
3. on the named DGX Spark, rerun the same retained image and trace twice with all five source archives, require
   byte-identical path-free output and zero `missing-license-bytes` blockers, and keep all expression/review gates
   closed.

Until both exact members exist, stop with this blocker. Do not add a downloader, infer a licence expression, create a
review manifest, weaken complete coverage, or add a Linux profile, executable, bundle assembly, hardware probe, worker
import/download, app execution, UI/IPC field, Docker product dependency, cloud fallback, local editing, guessed Qwen
Image 2.0 weights, signing, release, Store, or updater publication.
