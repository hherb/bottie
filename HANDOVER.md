# Bottie handover

Last verified: 2026-09-17

## Start here

Start from the draft PR for `codex/linux-image-worker-license-sources`. Read `ROADMAP.md` Milestone 8.4,
`docs/local-image-linux-worker-bundle.md`, `docs/local-image-linux-runtime-closure-review.json`, and
`local-image-worker/diffusers_runtime_native.py`.

## Current state

- The retained DGX image is still exactly
  `sha256:cded2049f9dbff513406052a191af2588476056d795468c4aacb7814aca5666c` on Linux ARM64, with the same
  Python and process-map trace digests.
- Marker-backed native components may now bind exact licence bytes from either a measured package-manager component or
  one regular member of a bounded exact in-image source archive. Source-package licence expressions are never inherited.
- cuSPARSELt binds the exact 17,948-byte `libcusparselt0-cuda-13@0.8.1.1-1` copyright record. Open MPI and UCX bind
  top-level licence members from their exact HPC-X source archives. Archive/member drift, duplicate members, missing
  package evidence, and oversized sources fail closed.
- Two fresh offline collections are byte-identical: 31,685 bytes, SHA-256
  `fc8ed1d465640bd1c5c13dda5c6b5850207d0baa638f070e31c19203c924f7c9`. The collector exits 3 with 106 blockers:
  five missing licence-byte records, seventeen undeclared licences, and 84 unreviewed expressions.
- `closureComplete` remains true. `licenseReviewed`, `assemblyEligible`, and `distributionReviewed` remain false. No
  review manifest or bundle was created, and no license expression, redistribution approval, or product availability
  was inferred.

## Validation

The focused source-binding and closure suite passes 21 tests; all 50 local worker tests that do not require Pillow pass.
The identical collector ran twice on the exact DGX image and trace with byte-identical output and expected status 3.
Prettier, Svelte diagnostics, all 407 active frontend/script tests (3 skipped), and the production build pass. Cargo
formatting/checking pass; the final host-local Rust suite passes 649 library tests (37 ignored) plus all integration and
doc tests after the expected sandbox loopback denials. Unrelated untracked logo-kit, screenshot, and Linux public-key
files remain untouched.

## Next slice

Resolve only these five exact missing-byte identities:

- `native:hpcx-ucc@1.5.0+ec95a0a96fc7220e1627157439c508cafc82274e`;
- `native:nvidia-nvpl-blas@0.2.0`;
- `native:nvidia-nvpl-lapack@0.2.2`;
- `python:sentencepiece@0.2.2`; and
- `python:tokenizers@0.23.2`.

The exact image contains no UCC source archive or matching UCC/NVPL document, and both installed Python wheels omit
licence files. Obtain only authoritative bytes bound to these exact versions or revisions; do not substitute nearby
packages, infer an expression from a project name, accept new terms, or weaken complete-manifest coverage. After all 84
components have independently reviewed source and review bytes, run the complete manifest twice and require
byte-identical path-free output, `licenseReviewed: true`, zero licence blockers, and `distributionReviewed: false`.

If any exact source or independent review remains unavailable, stop with the blocker summary. Do not add an accepted
Linux profile, executable, bundle assembly, hardware probe, worker import/download, app execution, UI/IPC field, Docker
product dependency, cloud fallback, local editing, guessed Qwen Image 2.0 weights, signing, release, Store, or updater
publication.
