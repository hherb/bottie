# Bottie handover

Last verified: 2026-09-22

## Start here

Start from the draft PR for `codex/linux-clean-runtime-license-sources` after it merges. Read `ROADMAP.md` Milestone
8.4 and the clean-runtime sections of `docs/local-image-linux-nvidia-proof.md` and
`docs/local-image-linux-worker-bundle.md`.

## Current state

- Two exact offline Linux/ARM64 rebuilds still agree across 63 Python identities, 112 Debian identities, and 27,035
  normalized regular files. The accepted rebuilt image is
  `sha256:740816cb8f348aa26e3d32f73b15b86b7f5a7228cff7b24d6910ee7aa0ee12a4`.
- The clean-runtime profile now accepts only the exact SentencePiece 0.2.2 and tokenizers 0.23.2 authoritative source
  archives. It requires both archives, exact component identities, archive/member sizes and hashes, and rejects partial
  input, licence-review manifests, the legacy NGC native-source set, image drift, lock drift, trace drift, and source
  drift.
- Collection is read-only, non-root, capability-free, and offline. A bounded in-process little-endian AArch64 ELF
  reader closes `NEEDED` and `SONAME` edges without adding `readelf` or changing the frozen runtime.
- The two independent collections agree on 161 files totalling 4,039,257,278 bytes, including 114 ELF files and 62
  components. `runtimeFilesSha256` is `f0f3e2dabe8777609f479ef81b3542e966f312a528183afda3f07368d43424d4`.
- `docs/local-image-linux-clean-runtime-closure-review.json` is the 23,223-byte path-free record with SHA-256
  `ebbca8034178339e6ba6007ad244ace868157a67f715dcd0628926444bd806fb`. `closureComplete` is true.
- No missing licence-byte blocker remains. Exactly 66 licence-only blockers remain: Jinja2, safetensors, tokenizers,
  and Triton have undeclared expressions, and all 62 component expressions are unreviewed. `licenseReviewed`,
  `assemblyEligible`, and `distributionReviewed` remain false.
- No licence-review manifest, bundle assembly, accepted Linux profile, hardware gate, app execution/UI, signing,
  release, Store, updater, provider, or legal action exists.

## Validation

- All 118 local image-worker Python tests pass, including exact source selection/completeness, profile/lock/trace
  rejection, canonical package identities, in-process AArch64 ELF parsing, legacy NGC behavior, and agreement.
- Python byte-compilation and Black checks pass; every changed implementation is below 500 lines.
- Prettier, Svelte diagnostics, all 407 active frontend/script tests (3 skipped), and the production build pass. The
  default Vitest glob also found the unrelated `.claude` worktree; the clean rerun explicitly excluded `.claude/**`.
- Cargo formatting and locked checking pass. The locked Rust suite passes outside the sandbox after expected
  loopback-bind denials: 649 library tests (37 ignored), 1 updater-evidence test, 6 local-image adapter tests, 10
  worker-transport tests, and doc tests.
- Both final DGX collections agree and reproduce the checked-in closure record SHA-256. JSON parsing and
  `git diff --check` pass.

## Next slice

Complete independent expression review for the exact clean closure without assembling a bundle:

1. obtain a complete independent review record for all 62 sorted component identities, including reviewed non-placeholder
   expressions for Jinja2, safetensors, tokenizers, and Triton; do not infer expressions from package metadata;
2. TDD a clean-profile-only two-trace review intake bound to the exact image, lock, trace contexts, source archives,
   licence/notice bytes, and per-component review evidence without weakening the retained NGC review route;
3. rerun both closures and retain output only if every expression and review byte agrees, no licence blocker remains,
   `licenseReviewed` and `assemblyEligible` are true, and `distributionReviewed` remains false.

Stop if a complete independent review record is unavailable. Do not author a review conclusion, infer an expression,
assemble or publish bytes, accept a Linux profile, add hardware probing/app execution/UI, or perform signing, release,
Store, updater, provider, or legal-terms activity.
