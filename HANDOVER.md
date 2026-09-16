# Bottie handover

Last verified: 2026-09-16

## Start here

`main` includes merged PR #192 at `92feae2`. Branch `codex/linux-image-worker-closure-identities` resolves the runtime
closure identity blockers for the Linux ARM64 Diffusers bundle-preparation gate. Read `ROADMAP.md` Milestone 8.4,
`docs/local-image-linux-worker-bundle.md`, `docs/local-image-linux-runtime-closure-review.json`, and
`local-image-worker/diffusers_runtime_native.py`.

## Current state

- The exact image is
  `sha256:cded2049f9dbff513406052a191af2588476056d795468c4aacb7814aca5666c`, based on
  `sha256:417cbf33f87b5378849df37983552cd1f8bc8b62fe1ceabe004de816a55dff21`.
- Exact in-image version markers now bind the fourteen previously unowned files to six native components: HPC-X Open
  MPI, UCC, and UCX; NVIDIA cuSPARSELt; and NVPL BLAS and LAPACK. Provenance is path-free and marker drift fails closed.
  ELF basename collisions block only when an actual `NEEDED` edge requests the colliding identity.
- Two independent GPU-injected DGX collections were byte-identical. The host-bound record is 30,671 bytes at SHA-256
  `8a3d8cdf7c4d35f70740bc9391c3747a39b94a3898dab21f3d339f4fc55e6024`. It contains 340 files totalling
  6,133,717,252 bytes, 295 ELF files, and 84 components: 51 Python, 27 Debian, and six unmanaged native.
- All ownership and ELF blockers are gone and `closureComplete` is true. Assembly still stops with status 3 on 109
  licence-only blockers: eight missing licence-byte records, seventeen undeclared licences, and 84 unreviewed
  expressions. `licenseReviewed`, `assemblyEligible`, and `distributionReviewed` remain false.
- No bundle was assembled, accepted, imported, or executed. Linux remains `CandidatePreparationOnly`; no product
  profile, executable, hardware probe, app execution, IPC/UI, signing, release, Store, or updater behavior changed.

## Validation and limits

The exact-image focused suite passes 17 active tests with one host-only orchestration test skipped, and all 47 local
Python worker tests pass. Two GPU-injected classifier runs produced identical 24,287-byte unbound records at SHA-256
`26c847d730e6ff323a390af802d97e3dc0584f6274d330780346d92c45545330`. Prettier, Svelte diagnostics, all 407 active
frontend/script tests (3 skipped), and the production build pass. Cargo formatting/checking pass; the host-local Rust
suite passes 649 active library tests (37 ignored), updater evidence, all 6 execution-adapter tests, all 10 private-worker
transport tests, and doc tests. No browser or native-app review is required because this slice changes no presentation
or app execution.

A read-only exact-image search found a 17,948-byte package-owned NVIDIA SDK licence record for cuSPARSELt at SHA-256
`e8d158885a681b95ec7a6fc06dd8d4a52989f374cb1380c8a4c8fb27fd3d5d5e`, but no matching in-image licence files for
HPC-X Open MPI/UCC/UCX or NVPL BLAS/LAPACK. `sentencepiece` and `tokenizers` are the other two missing-byte components.
This is discovery evidence only, not legal review or redistribution approval. Unrelated untracked logo-kit, screenshot,
and Linux public-key files remain untouched.

## Next slice

On the same exact image, resolve only the 109 licence blockers. First obtain authoritative licence/notice bytes for the
eight missing-byte components, failing closed when the exact installed component cannot be bound to those bytes. Then
independently review and normalize the declared expression for all 84 exact components. Preserve the source bytes and
review evidence in a deterministic path-free manifest; require `licenseReviewed: true` before considering assembly.

Do not accept new legal terms, infer a licence from a project name, download substitute bytes, or assemble a bundle
without explicit authorization. If authoritative evidence is unavailable, stop with the exact path-free blocker
summary. Keep Linux at `CandidatePreparationOnly`: do not add an accepted profile, executable, hardware probe, worker
import/download, application execution, UI/IPC field, Docker product dependency, cloud fallback, local editing,
guessed Qwen Image 2.0 weights, signing, release, Store, or updater publication.
