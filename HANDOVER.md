# Bottie handover

Last verified: 2026-09-18

## Start here

Start from the draft PR for `codex/linux-clean-runtime-closure-profile` after it merges. Read `ROADMAP.md` Milestone
8.4 and the clean-runtime sections of `docs/local-image-linux-nvidia-proof.md` and
`docs/local-image-linux-worker-bundle.md`.

## Current state

- Two exact offline Linux/ARM64 rebuilds still agree across 63 Python identities, 112 Debian identities, and 27,035
  normalized regular files. The accepted rebuilt image is
  `sha256:740816cb8f348aa26e3d32f73b15b86b7f5a7228cff7b24d6910ee7aa0ee12a4`.
- A distinct clean-runtime closure profile binds that image, Ubuntu base digest, the conventional PyTorch worker/runtime
  identity, the frozen input lock, and either retained trace context. It rejects caller-defined profiles,
  licence-review manifests, external licence sources, image drift, lock drift, trace drift, and source drift.
- Collection is read-only, non-root, capability-free, and offline. A bounded in-process little-endian AArch64 ELF
  reader closes `NEEDED` and `SONAME` edges without adding `readelf` or changing the frozen runtime.
- The two independent collections agree on 161 files totalling 4,039,257,278 bytes, including 114 ELF files and 62
  components. `runtimeFilesSha256` is `f0f3e2dabe8777609f479ef81b3542e966f312a528183afda3f07368d43424d4`.
- `docs/local-image-linux-clean-runtime-closure-review.json` is the 21,919-byte path-free record with SHA-256
  `f90d57ef58c18a779c9d87301c757d64106f61079bb1edacfcd59b09248c261e`. `closureComplete` is true.
- Exactly 68 licence-only blockers remain: SentencePiece and tokenizers lack licence bytes; Jinja2, safetensors,
  tokenizers, and Triton have undeclared expressions; all 62 component expressions are unreviewed. `licenseReviewed`,
  `assemblyEligible`, and `distributionReviewed` remain false.
- No licence-review manifest, bundle assembly, accepted Linux profile, hardware gate, app execution/UI, signing,
  release, Store, updater, provider, or legal action exists.

## Validation

- All 114 local image-worker Python tests pass, including exact profile/lock/trace rejection, canonical package
  identities, in-process AArch64 ELF parsing, legacy NGC behavior, and two-closure agreement.
- Python byte-compilation and Black checks pass; every changed implementation is below 500 lines.
- Prettier, Svelte diagnostics, all 407 active frontend/script tests (3 skipped), and the production build pass. The
  default Vitest glob also found the unrelated `.claude` worktree; the clean rerun explicitly excluded `.claude/**`.
- Cargo formatting and locked checking pass. The locked Rust suite passes outside the sandbox after expected
  loopback-bind denials: 649 library tests (37 ignored), 1 updater-evidence test, 6 local-image adapter tests, 10
  worker-transport tests, and doc tests.
- Both final DGX collections agree and reproduce the checked-in closure record SHA-256. JSON parsing and
  `git diff --check` pass.

## Next slice

Bind authoritative licence bytes for the two exact missing-byte components without reviewing expressions yet:

1. TDD a clean-profile-only source gate for the exact SentencePiece 0.2.2 and tokenizers 0.23.2 authoritative source
   archives already identified by the earlier NGC evidence; require exact archive digest, member name, bytes, and
   component identity, and do not reuse the original NGC component set wholesale;
2. run both retained clean-runtime traces again and require them to agree on one updated path-free closure with only
   undeclared-expression and unreviewed-expression blockers remaining;
3. do not create or accept a licence-review manifest, infer an expression, assemble bytes, or change product gates.

The user authorized transfer of only the additional reviewed proof/closure modules needed for this DGX evidence chain.
Obtain explicit approval for any new repository-source payload not already named and approved. Do not add packages or
network access to the frozen image, add hardware probing/app execution/UI, or perform signing, release, Store, updater,
provider, or legal-terms activity.
