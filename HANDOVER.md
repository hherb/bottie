# Bottie handover

Last verified: 2026-09-15

## Start here

`main` includes merged PR #176 at `3e9c856`. Branch `codex/local-image-execution-adapter` continues Milestones 8.3-8.4.
Read `ROADMAP.md` Milestones 8.3-8.4, `docs/local-image-model-package.md`, and
`src-tauri/src/local_image_worker/`.

## Completed slices

- A fresh `ready` inspection now yields a Rust-only verified installation capability. Local start never reads a Cloud
  credential and accepts only the proved one-image 512x512 Qwen-Image-2512 request with prompt extension disabled.
- One Rust-owned warm worker runs behind the accepted macOS network-denied sandbox profile. Runtime/model identity,
  bounded protocol correlation, cooperative cancellation with forced teardown, and output-directory containment all
  fail closed; paths, hashes, hardware identity, process IDs, and worker payloads remain native-only.
- Local PNGs pass through the same bounded Rust normalization, content-addressed durable storage, generated-asset
  actions, recovery, and path-free presentation as Cloud outputs. Exact local model/seed/backend/options persist, and a
  failed or cancelled run retries only through that stored backend with no fallback.
- Image mode keeps Cloud as the default and exposes an explicit Local 2512 selector only when native readiness is exact.
  Local is visibly fixed to one 512x512 image and has device-local disclosure; unavailable readiness disables selection.

## Validation

Formatting, Svelte diagnostics (0 errors/warnings), production build, 375 frontend/script tests (3 skipped), 612 Rust
library tests (36 ignored), the updater-evidence test, all 16 private-worker integration tests, and Rust doc tests pass.
The sandboxed Rust suite could not bind 15 loopback fixtures; the identical host-local full suite passed. Browser
presentation was reviewed at the desktop viewport and 760px with the Context panel closed. A native launch was not run
because the repository's launcher performs development signing, which was outside this slice's authorization.

## Next slice

Add an app-owned, explicitly user-approved acquisition coordinator for the selected model package. Gate it on the exact
verified worker, disclose model/runtime IDs, Apache-2.0 license, revision, 16.2 GiB disk and 27.5 GiB measured peak memory
before any network or cache mutation, then expose only path-free progress/cancel/resume status and refresh readiness on
atomic promotion. Reuse the existing selected manifest, source-plan, resumable downloader, and transactional cache.

Do not auto-download, bundle the 1.1 GB proof worker or 16.2 GiB model, generalize hardware, add editing, silently fall
back to Cloud, or reuse Bottie's approved Python-tool runtime. Do not merge, dispatch workflows, sign, release, publish,
distribute, or perform Store work without separate authorization. Preserve unrelated untracked logo-kit, screenshot,
and Linux public-key files.
