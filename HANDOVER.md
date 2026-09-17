# Bottie handover

Last verified: 2026-09-17

## Start here

Start from the draft PR for `codex/linux-image-worker-missing-license-bytes`. Read `ROADMAP.md` Milestone 8.4,
`docs/local-image-linux-worker-bundle.md`, `local-image-worker/diffusers_runtime_license_sources.py`, and
`local-image-worker/diffusers_runtime_native.py`.

## Current state

- The retained DGX image remains
  `sha256:cded2049f9dbff513406052a191af2588476056d795468c4aacb7814aca5666c`, with the same Python and
  process-map trace digests. The checked-in closure summary is unchanged: 84 components, `closureComplete: true`, and
  106 licence blockers.
- The closure host now accepts an explicit `--license-sources` directory, mounts it read-only, and keeps Docker
  networking disabled. Only fixed catalog filenames are consumed; partial recognized input remains partial, while an
  empty or unrecognized input fails closed.
- Exact PyPI sdists now bind licence bytes for `python:sentencepiece@0.2.2` and `python:tokenizers@0.23.2`. Exact
  NVIDIA NVPL 24.03 archives bind licence bytes for `native:nvidia-nvpl-blas@0.2.0` and
  `native:nvidia-nvpl-lapack@0.2.2` only after both installed full-version libraries match regular archive members
  byte-for-byte.
- Source bytes add no licence expression or review conclusion. `licenseReviewed`, `assemblyEligible`, and
  `distributionReviewed` remain false. No review manifest, bundle, accepted Linux profile, or executable was created.
- The exact `native:hpcx-ucc@1.5.0+ec95a0a96fc7220e1627157439c508cafc82274e` revision remains the sole
  missing-byte identity without a source. It is absent from the retained image's source archives, and the authoritative
  upstream commit endpoint did not provide that revision. Do not substitute the nearby UCC 1.5.0 release.

## Validation

All 63 local worker Python tests pass. The four downloaded authoritative artifacts match the fixed archive and licence
member measurements; the NVPL runtime-member verifier was exercised against extracted official archive members. The
retained DGX image was unavailable on this host, so no new closure record was generated and no installed NVPL match is
claimed. Prettier, Svelte diagnostics, all 407 active frontend/script tests (3 skipped), and the production build pass.
Cargo formatting/checking pass; the host-local Rust suite passes 649 library tests (37 ignored) plus all integration and
doc tests after the expected sandbox loopback denials. Unrelated untracked logo-kit, screenshot, and Linux public-key
files remain untouched.

## Next slice

Resolve only `native:hpcx-ucc@1.5.0+ec95a0a96fc7220e1627157439c508cafc82274e`:

1. obtain authoritative archive and licence bytes bound to that exact revision without accepting new terms or using a
   nearby tag, release, package, or project-level licence assertion;
2. add one fixed source spec and require `/opt/hpcx/ucc/lib/libucc.so.1.0.0` to match its exact regular archive member;
3. on the named DGX Spark, rerun the same retained image and trace twice with all five source archives, require
   byte-identical path-free output and zero `missing-license-bytes` blockers, and keep all expression/review gates closed.

If exact UCC source or licence bytes remain unavailable, stop with that blocker. Do not add a downloader, infer a
licence expression, accept terms, create a review manifest, weaken complete coverage, or add a Linux profile,
executable, bundle assembly, hardware probe, worker import/download, app execution, UI/IPC field, Docker product
dependency, cloud fallback, local editing, guessed Qwen Image 2.0 weights, signing, release, Store, or updater
publication.
