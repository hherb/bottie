# Bottie handover

Last verified: 2026-09-16

## Start here

`main` includes merged PR #190 at `018c751`. Branch `codex/linux-image-worker-bundle-assembly` adds the fail-closed
environment review required before assembling a Linux ARM64 Diffusers worker candidate. Read `ROADMAP.md` Milestone
8.4, `docs/local-image-linux-worker-bundle.md`, `docs/local-image-linux-environment-review.json`, and
`local-image-worker/diffusers_bundle_environment.py`.

## Current state

- The environment gate resolves the requested image through the host Docker daemon, requires the exact immutable
  derived-image ID and Linux ARM64 metadata, then launches the unbound collector by that verified ID with networking
  disabled. The collector requires Python 3.12.3, every direct Python pin, and every checked-in proof input.
- It inventories every installed Python distribution and Debian package, measuring package-declared licence/notice
  bytes plus the NVIDIA container-terms bytes. Its deterministic JSON binds the exact base and derived image digests
  and retains no absolute filesystem paths.
- The named DGX Spark produced a complete package-manager record covering 248 Python and 434 Debian components. Two
  runs were byte-identical; the exact byte count and SHA-256 are retained in the checked-in summary.
- Assembly stopped with the intended status 3. The full environment has 76 exact blockers: twelve components lack
  measured package-owned licence bytes, and additional Python packages lack a non-placeholder licence declaration. The
  checked-in summary preserves every blocker and remains `assemblyEligible: false` and `distributionReviewed: false`.
- No worker bundle was assembled, accepted, imported, or executed. Linux remains `CandidatePreparationOnly`, without
  an accepted product profile or executable. No hardware probe, availability, UI/IPC, signing, release, Store,
  updater-publication, or support claim was added.

## Validation and limits

Prettier, Svelte diagnostics, all 407 active frontend/script tests (3 skipped), and the production build pass. All 32
Python worker tests pass inside the exact DGX image. Cargo formatting/checking pass; the host-local Rust run passes 649
active library tests (37 ignored), updater evidence, all 6 execution-adapter tests, all 10 private-worker transport
tests, and doc tests. The first sandboxed Rust run had the expected 19 loopback-bind denials; the identical host-local
suite passed. The exact gate passed its collection contract on the DGX and refused assembly; two target runs produced
identical records. No browser or native-app review was required because this slice changes no presentation or app
execution behavior.

The package-manager inventory is a conservative baseline, not a complete native dependency closure or proposed product
runtime: it includes unrelated development, Jupyter, and NVIDIA tools and does not classify unmanaged native files.
The summary and gate are distribution-review inputs, not legal approval or evidence that the installed environment may
be redistributed. Unrelated untracked logo-kit, screenshot, and Linux public-key files remain untouched.

## Next slice

On the same DGX Spark and exact digest-pinned image, derive and test one deterministic minimal runtime closure for
`diffusers_worker.py`: include every recursively required Python distribution, imported package file, ELF dependency,
and explicit host-driver boundary, while excluding model weights and unrelated environment tooling. Compare the closure
against the complete package-manager record, and fail if any imported or dynamically linked component is unowned.

For every component in that exact closure, obtain real authoritative licence/notice bytes and a reviewed licence
expression; do not infer either from package names or silently omit a component to make the gate pass. Only after the
closure is complete and licence-clean should a later step assemble symlink-free regular-file bytes and run
`diffusers_bundle_candidate.py`. If authoritative evidence is unavailable, stop and retain only a path-free blocker
summary. Keep Linux at `CandidatePreparationOnly`: do not add an accepted profile, executable, hardware probe,
worker import/download, application execution, UI/IPC field, Docker dependency, or support claim. Do not enable cloud
fallback, local editing, guessed Qwen Image 2.0 weights, signing, release, Store, or updater publication.
