# Bottie handover

Last verified: 2026-09-16

## Start here

`main` includes merged PR #189 at `847a6ce`. Branch `codex/linux-image-worker-bundle-evidence` completes the next
Milestone 8.4 preparation slice without enabling Linux image generation. Read `ROADMAP.md` Milestone 8.4,
`docs/local-image-linux-worker-bundle.md`, `local-image-worker/diffusers_bundle_candidate.py`, and
`src-tauri/src/local_image_worker/package_catalog.rs`.

## Current state

- The proof-only Linux candidate inspector is bound to the exact digest-pinned NGC Dockerfile, pinned Diffusers
  requirements, private worker sources, worker/runtime/model identities, model revision, base-image digest, and Linux
  ARM64 target. Any checked-in proof-input drift fails closed.
- It inventories only a closed, symlink-free regular-file tree in deterministic path order and records exact per-file,
  executable, and complete-bundle sizes and SHA-256 values. Its domain-separated bundle digest matches Bottie's native
  `worker_bundle.rs` verifier; a fixed cross-language vector protects that contract.
- The included schema permits only the `bottie` subtree and the licence manifest itself to be first-party. Every other
  file must have exactly one third-party owner with exact version/source/licence metadata and included licence bytes.
  Missing, placeholder, ambiguous, relabelled, or unclassified evidence fails closed.
- Every emitted record is explicitly `distributionReviewed: false`. The native catalog marks Apple's exact bundle
  evidence accepted for its existing import route and Linux as `CandidatePreparationOnly`; Linux still has no accepted
  product profile or importable executable.
- No Linux worker bytes were produced or accepted. No hardware probe, worker import/download, availability, execution,
  UI/IPC field, Docker application dependency, signing, release, Store, updater-publication, or support claim was added.

## Validation and limits

Prettier, Svelte diagnostics, all 407 active frontend/script tests (3 skipped), the production build, and all 19
local-image-worker Python tests pass. `cargo fmt --check` and `cargo check` pass. The host-local Rust run passes all 649
active library tests (37 ignored), the updater evidence test, all 6 execution-adapter tests, all 10 private-worker
transport tests, and doc tests. The first sandboxed Rust run had 19 expected loopback-bind denials; the identical
host-local run passed. No browser or native-app review was required because this slice changes no presentation or
executable behavior.

The inspector proves structural evidence consistency, not provenance, reproducibility, redistributability, licence
accuracy, or product acceptance. NVIDIA container/product terms and every emitted native/Python dependency still need
independent review. Unrelated untracked logo-kit, screenshot, and Linux public-key files remain untouched.

## Next slice

On the named DGX Spark Linux ARM64 environment, define and exercise one deterministic proof-only assembly procedure
that produces a worker bundle from the exact pinned NGC/Diffusers inputs without model weights. Include a complete
installed Python/native component inventory and real licence/notice bytes, run
`diffusers_bundle_candidate.py` against the closed output, and independently compare its file ownership, source pins,
licence obligations, executable hash, and native-compatible bundle hash to the produced environment. Retain only
path-free review evidence in the repository; do not commit the large runtime bytes.

Keep the catalog's Linux stage at `CandidatePreparationOnly`, with no accepted profile or executable, unless those
exact produced bytes and their complete distribution/licence record pass independent review. If the named environment
or authoritative licence evidence is unavailable, stop rather than synthesize either. Do not add a Linux hardware
probe, worker import/download, native availability, application execution, UI/IPC field, Docker dependency, or support
claim. Do not enable cloud fallback, local editing, guessed Qwen Image 2.0 weights, signing, release, Store, or updater
publication work.
