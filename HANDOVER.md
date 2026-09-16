# Bottie handover

Last verified: 2026-09-16

## Start here

`main` includes merged PR #188 at `a79ff2d`. Branch `codex/local-image-backend-catalog` completes the next Milestone
8.4 preparation: native local-image package selection now carries an explicit backend, runtime, target, evidence, and
product-executable contract while keeping Linux NVIDIA unavailable. Read `ROADMAP.md` Milestone 8.4,
`src-tauri/src/local_image_worker/package_catalog.rs`, `docs/local-image-model-package.md`, and
`docs/local-image-linux-nvidia-proof.md`.

## Current state

- The closed native catalog has two exact candidates. Apple binds MLX-Gen worker
  `mlx-gen-0.18.2-proof-1`, runtime `mlx-gen@fca64a283737c68b67a7bfd88d93f7aa9101a95c`, the immutable mixed-q4/q8
  model revision, macOS ARM64, and the accepted Apple M3 Max 128 GiB profile. Linux binds the proved Diffusers worker
  and runtime, full model revision, Linux ARM64, DGX Spark GB10 evidence profile, and evidence document.
- Only Apple has an accepted product profile and importable executable basename. Linux has neither, so catalog
  selection fails closed before worker import, readiness, acquisition, or execution.
- `SelectedModelPackage` retains the selected runtime and verifies that its model, revision, and runtime identities
  equal the existing Apple manifest. The availability service derives the executable basename from that selection
  instead of a global MLX literal.
- The pure availability policy compares native OS, architecture, and hardware evidence against the selected runtime's
  target and accepted profile. Apple readiness precedence, manifest, acknowledgement, acquisition, path-free IPC,
  worker-cache layout, and execution behavior remain unchanged.
- No Linux hardware probe, distributable worker, import/download, application execution, availability, UI/IPC field,
  or support claim was added.

## Validation and limits

Prettier, Svelte diagnostics, all 407 active frontend/script tests (3 skipped), the production build,
`cargo fmt --check`, and `cargo check` pass. The host-local Rust run passes all 648 active library tests (37 ignored),
the updater evidence test, all 6 execution-adapter tests, all 10 private-worker transport tests, and doc tests. The
focused local-image run passes all 81 tests. Its first sandboxed attempt had 12 expected loopback-bind denials; the
identical host-local command passed. No browser or native-app review was required because this slice changes no
presentation or executable runtime behavior.

The Linux Diffusers entry is proof metadata only. It does not establish redistributable Python/NVIDIA runtime bytes,
an accepted executable or bundle digest, product hardware support, or native containment. The existing Apple M3 Max
profile remains the only accepted local route. Unrelated untracked logo-kit, screenshot, and Linux public-key files
remain untouched.

## Next slice

Prepare one proof-only distributable Linux worker-bundle candidate from the exact pinned Diffusers/NGC environment.
Define and test a deterministic regular-file inventory plus executable/bundle size and SHA-256 evidence, bind it to the
existing worker/runtime/model revisions and complete third-party license metadata, and document the distribution-review
boundary. Keep its catalog product profile and executable absent until exact produced bytes and their licensing are
independently reviewed.

Do not add a Linux hardware probe, worker import/download, native availability, runtime execution, UI/IPC field, or
support claim in that slice. Do not add Docker as an application dependency, infer redistributability from the proof,
accept mutable tags, enable cloud fallback, add local editing, guess Qwen Image 2.0 weights, expose source bytes or
paths, or resume signing, release, Store, or updater-publication work.
