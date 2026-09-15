# Bottie handover

Last verified: 2026-09-15

## Start here

`main` includes merged PR #178 at `2269d7f`. Branch `codex/local-image-worker-cache` completes the manual installation
boundary for the accepted local image worker. Read `ROADMAP.md` Milestones 8.3-8.5,
`docs/local-image-model-package.md`, and `src-tauri/src/local_image_worker/`.

## Completed slices

- Local 2512 execution now resolves the worker only from a deterministic app-data cache identity derived from the
  accepted runtime, executable digest/size, and canonical bundle digest/size. Signed application resources are not a
  worker source.
- `worker_cache.rs` requires an exact path-free runtime/size approval before mutation and verifies the user-selected
  source before creating cache state. It copies only regular symlink-free content into same-filesystem staging, syncs
  and re-hashes the staged executable and whole bundle, then atomically promotes it. Missing/tampered state remains
  read-only; replacement retains the prior drifted target until an approved source and staged copy both verify.
- The composer offers a native folder picker only for `worker_missing` or `worker_mismatch`. Paths and hashes stay in
  Rust; IPC exposes only the 1,107,880,778-byte requirement, active state, fresh availability, and fixed outcomes.
  Import preflights image generation and model acquisition before selection and promotion, then refreshes model
  eligibility on success.
- Exact model acquisition, execution-time worker/model re-verification, the network-denied warm worker, shared durable
  PNG handling, cancellation, provenance, retry, export, and deletion remain unchanged. There is still no Cloud
  fallback.

## Validation

Prettier check, Svelte diagnostics (0 errors and 0 warnings), all 390 active frontend/script tests (3 skipped), the
production build, `cargo fmt --check`, and `cargo check` pass. The complete host-local Rust run passes all 627 active
library tests (36 ignored), the updater-evidence test, all 16 private-worker integration tests, and doc tests. The
desktop
browser preview was reviewed at 1280x720 in image mode: Cloud remains explicit and the unavailable local panel is
legible; native import behavior is covered by SSR/state and Rust contract tests because browser preview has no picker.

No native app was launched and no worker/model bytes were imported or downloaded. The native launcher development-signs
the app. Final diff review found and fixed cache/source overlap, stale eligibility, and duplicate-gate coverage gaps.
Unrelated untracked logo-kit, screenshot, and Linux public-key files remain untouched.

## Next slice

Begin Milestone 8.5 with the durable hosted-editing foundation: add a migration and Rust storage contracts that preserve
ordered lineage from one generated assistant asset to one-to-three validated source assets, plus a provider-neutral
editing request contract tied to exact hosted `qwen-image-2.0-2026-03-03` provenance. Cover upgrade/reopen,
branch, export, backup, and retention ownership, plus source deletion, duplicate/order bounds, malformed identities, and
path-free serialization.

Do not call DashScope, spend provider credits, add editing UI, forward native paths or unapproved attachment bytes,
implement local editing, claim local Qwen-Image-2.0 weights, or weaken existing generated-asset ownership. Do not merge,
dispatch workflows, sign, release, publish, distribute, or perform Store work without separate authorization. Preserve
unrelated untracked logo-kit, screenshot, and Linux public-key files.
