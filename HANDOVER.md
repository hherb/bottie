# Bottie handover

Last verified: 2026-09-16

## Start here

`main` includes merged PR #193 at `4e10108`. Branch `codex/linux-image-worker-license-evidence` adds the fail-closed
licence-review manifest gate for the exact Linux ARM64 Diffusers runtime closure. Read `ROADMAP.md` Milestone 8.4,
`docs/local-image-linux-worker-bundle.md`, `local-image-worker/diffusers_license_review.py`, and
`local-image-worker/diffusers_runtime_closure.py`.

## Current state

- The configured `dgx` SSH target is reachable. The retained image still resolves exactly to
  `sha256:cded2049f9dbff513406052a191af2588476056d795468c4aacb7814aca5666c` on Linux ARM64.
- The closure host command accepts an optional review manifest mounted read-only into that exact offline image. The
  closed schema binds the image digest, both proof-trace digests, and all 84 sorted component identities.
- Each component must embed at least one exact licence/notice file and a separate independent-review record. Strict
  Base64 decoding, byte counts, SHA-256, portable names, complete coverage, and canonical ordering fail closed. Source
  and review bytes stay in the external manifest; only path-free measurements enter closure output.
- Only a structurally valid complete manifest may replace missing/undeclared package metadata and clear the three
  licence blocker classes. No unchecked expression-only override remains.
- No manifest was created and all 109 blockers remain: eight missing byte records, seventeen undeclared licences, and
  84 unreviewed expressions. `licenseReviewed`, `assemblyEligible`, and `distributionReviewed` remain false.
- No bundle was assembled, accepted, imported, or executed. Linux remains `CandidatePreparationOnly`; no app, IPC/UI,
  hardware probe, signing, release, Store, or updater behavior changed.

## Validation and limits

The new manifest/closure suite passes 17 tests. All 48 local worker tests that do not require Pillow pass; the separate
proof test is unavailable in the local Python 3.13 environment because Pillow is not installed. Prettier, Svelte
diagnostics, all 407 active frontend/script tests (3 skipped), and the production build pass. Cargo formatting/checking
pass; the identical host-local Rust suite passes after the expected sandbox loopback denials (649 library tests passed,
37 ignored, followed by the remaining integration and doc tests).

A fresh offline read-only exact-image inspection found no licence document in the installed `sentencepiece` or
`tokenizers` wheels and no matching HPC-X Open MPI/UCC/UCX or NVPL BLAS/LAPACK document; the only HPC-X match was an
unrelated SHARP licence. The already-known 17,948-byte cuSPARSELt Debian record remains the sole newly located source.
The new source files were not transferred to the DGX, so exact-image execution of this branch remains for the next
session.
Unrelated untracked logo-kit, screenshot, and Linux public-key files remain untouched.

## Next slice

On the same exact image and trace, obtain authoritative bytes for the seven still-unlocated component documents and
bind the known cuSPARSELt record to its exact native identity. Do not download substitutes or accept new terms. Build a
complete sorted 84-component manifest only after an independent review normalizes every expression and records its
source bytes and review bytes. Run the reviewed collector twice on the DGX; require byte-identical path-free output,
`licenseReviewed: true`, and zero licence blockers before considering assembly. Keep `distributionReviewed: false`.

If any authoritative byte or reviewed expression remains unavailable, stop with the exact blocker summary. Do not add
an accepted Linux profile, executable, bundle assembly, hardware probe, worker import/download, application execution,
UI/IPC field, Docker product dependency, cloud fallback, local editing, guessed Qwen Image 2.0 weights, signing,
release, Store, or updater publication.
