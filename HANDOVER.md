# Bottie handover

Last verified: 2026-09-18

## Start here

Start from the draft PR for `codex/linux-clean-runtime-rebuild-evidence` after it merges. Read `ROADMAP.md` Milestone
8.4 and the clean-runtime sections of `docs/local-image-linux-nvidia-proof.md` and
`docs/local-image-linux-worker-bundle.md`.

## Current state

- Two independently named, network-disabled, no-cache, no-pull Linux/ARM64 rebuilds from the frozen 175-archive input
  tree agree. Their 63 Python and 112 Debian identities are exact; 27,035 normalized regular files totaling
  5,024,684,703 bytes share SHA-256 `e48d83b963158b94163c8c8d8c7a0d128d8ee77355ed22bb83725e8383089055`.
- `docs/local-image-linux-clean-runtime-rebuild-evidence.json` is the 993-byte path-free record with SHA-256
  `817148f314b709b2d9c82c6a4acbd8632db1b5b16f12412c2b3528d6dbac56e7`.
- The gate now handles Ubuntu pre-dependencies with three bounded exact `dpkg` passes, stages inspection scripts
  read-only but traversable by UID/GID 65534, and removes only generated Python bytecode and linker caches before the
  strict comparison. It did not add package egress, dependency resolution, or filesystem exclusions.
- The first rebuilt image passed two independent full private-protocol proofs using the exact venv interpreter with no
  UCC mask. Both found no UCC installation or mapping, denied network access, reproduced the previously reviewed PNG
  and RGB hashes, cancelled in 101 ms, and shut down cleanly. `docs/local-image-linux-clean-runtime-proof.json` retains
  the path-free measurements and four fresh trace digests.
- No rebuilt-image closure, licence review, bundle, accepted Linux profile, app wiring, product availability, signing,
  release, Store, updater, provider, or legal action exists. All product/distribution gates remain false.

## Validation

- Focused rebuild/proof suites pass: 28 tests covering exact offline construction, deterministic cleanup,
  unprivileged inspection, clean-profile interpreter selection, real UCC absence, and mapping rejection.
- All 103 local image-worker Python tests pass, as do byte-compilation and Black checks for every changed Python file.
- Prettier, Svelte diagnostics, all 407 active frontend/script tests (3 skipped), and the production build pass.
- Cargo formatting and locked checking pass. The locked Rust suite passes outside the sandbox after expected
  loopback-bind denials: 649 library tests (37 ignored), 1 updater-evidence test, 6 local-image adapter tests, 10
  worker-transport tests, and doc tests.
- JSON parsing and `git diff --check` pass. Memory sampling is isolated so each changed Python implementation remains
  below 500 lines.

## Next slice

Add a distinct clean-runtime closure profile and run it against both fresh traces:

1. TDD a closure host/profile that accepts only rebuilt image
   `sha256:740816cb8f348aa26e3d32f73b15b86b7f5a7228cff7b24d6910ee7aa0ee12a4`, the PyTorch worker/runtime identity,
   Ubuntu base digest, and either of the exact retained trace contexts;
2. preserve the existing NGC closure unchanged, keep collection read-only/non-root/offline, and require both new
   closures to agree before retaining one path-free summary;
3. report every missing-byte, undeclared-expression, ownership, Python-dependency, ELF, and host-driver blocker. Do not
   infer licence expressions or reuse the original NGC component set.

The user authorized transfer of only the additional reviewed proof/closure modules needed for this DGX evidence chain.
Do not create or accept a licence-review manifest, assemble or publish bytes, add hardware probing/app execution/UI, or
perform signing, release, Store, updater, provider, or legal-terms activity.
