# Bottie handover

Last verified: 2026-09-15

## Start here

`main` includes merged PR #177 at `86b8a17`. Branch `codex/local-image-acquisition-coordinator` continues Milestones
8.3-8.4 with explicit model installation. Read `ROADMAP.md` Milestones 8.3-8.4,
`docs/local-image-model-package.md`, and `src-tauri/src/local_image_worker/`.

## Completed slices

- Local execution still requires a fresh exact hardware, worker-bundle, and promoted-model inspection. It uses one
  network-denied warm worker, the fixed 512x512 Qwen-Image-2512 contract, shared private PNG storage/actions, exact
  provenance and retry, cooperative cancellation, and no Cloud fallback.
- `LocalImageAcquisitionCoordinator` now permits cache/network mutation only after the worker and hardware gates pass
  and one affirmative request exactly echoes the disclosed model, package, runtime, Apache-2.0 license, immutable
  revision, 16.2 GiB disk use, and 27.5 GiB measured peak memory.
- The single native slot drives the selected strict Hugging Face source plan through resumable staging and atomic
  promotion. Startup inspection is read-only; cancellation/interruption retains exact synced partials; corrupt or
  drifted staging is cleaned only after another explicit action. Paths, hashes, source URLs, response details, and
  native correlation remain outside IPC.
- Image mode keeps Cloud as the default. When the exact worker is installed and the model is absent or mismatched, the
  composer shows an explicit install/resume control, bounded progress, fixed failures, and cancellation. Atomic success
  refreshes readiness and enables Local 2512 without restarting.

## Validation

Prettier write/check, Svelte diagnostics (0 errors and 0 warnings), all 385 active frontend/script tests (3 skipped),
the production build, `cargo fmt --check`, and `cargo check` pass. The identical host-local Rust library suite passes
all 620 active tests (36 ignored). The sandboxed full Rust run could not bind 15 loopback library fixtures, but its
updater-evidence test, all 13 private-worker integration tests, and doc tests pass. A host full-suite invocation then
stalled when its standalone test executables did not reach their harnesses, so Rust coverage is combined rather than
one green full invocation. The browser preview was reviewed in desktop image mode: the local availability panel remains
legible and Cloud remains explicit; exact acquisition states are covered by SSR and state tests because browser preview
has no native acquisition coordinator. No native app was launched because the repository launcher development-signs
the app, and no worker or model bytes were downloaded.

## Next slice

Make the accepted 1.1 GB MLX-Gen worker installable without placing it in the signed application bundle. First define
and test an app-owned transactional worker-cache/import contract tied to the accepted executable and canonical bundle
digests, then change fixed-path readiness/execution to resolve only that promoted worker. Keep user-selected native
paths and bundle hashes out of IPC, require explicit approval before copying bytes, and preserve the current
network-denied execution sandbox.

Do not invent a worker download source or archive digest, auto-download, bundle the proof worker or 16.2 GiB model,
generalize hardware, add editing, silently fall back to Cloud, or reuse Bottie's approved Python-tool runtime. Do not
merge, dispatch workflows, sign, release, publish, distribute, or perform Store work without separate authorization.
Preserve unrelated untracked logo-kit, screenshot, and Linux public-key files.
