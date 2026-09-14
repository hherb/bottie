# Bottie handover

Last verified: 2026-09-14

## Start here

PR #170 merged into `main` at `e0e8984`. Branch `codex/local-image-model-cache` completes the next two bounded
Milestone 8.3 foundations: transactional app-owned model caching and exact cache re-verification at the private worker
load boundary. Read `ROADMAP.md` Milestone 8.3 and `src-tauri/src/local_image_worker/` before continuing.

## Completed slices

- `ModelCacheTransaction` validates the existing exact native manifest before deriving opaque SHA-256 staging and final
  directory identities. A model identity has one resumable staging slot; only the complete serialized manifest derives
  the promoted-package identity.
- Cache writes accept only exact declared portable paths and exact durable resume offsets. Existing bytes are
  re-hashed, new bytes are size-bounded and hashed while streaming, partials are synced for restart, and digest/size
  failures remove the untrusted file.
- Restart resumes only an exactly matching manifest. Runtime/source/file drift, malformed markers, unknown entries,
  unsafe file types, symlinks, case-colliding paths, traversal, and escaped parents fail closed or discard only the
  exact managed partial transaction.
- A complete staging tree passes the existing all-files size/SHA-256 activation gate before one same-volume directory
  rename. Manifest files, changed entries, and the staging/packages directories are synced around promotion; Windows
  directory sync uses a backup-semantics handle.
- Promoted packages reopen only through the same activation gate. `begin_cached_model_load` repeats that verification
  immediately before the private worker load frame; detected post-promotion mutation leaves the worker idle.
- Exact hosted `qwen-image-2.0-2026-03-03` remains distinct from local open-weight `Qwen/Qwen-Image-2512`, and this
  subsystem remains separate from Bottie's user-approved Python tool runtime.

## Validation

- Focused cache suite: 8 passed, covering interrupted writes/restart resume, manifest drift, path/symlink escape,
  digest/size mismatch, pre-rename atomicity, gate-based reopen, broken-link replacement, and exact cleanup.
- Private-process integration suite: 10 passed, including cached load and rejection of mutated promoted bytes before any
  worker load frame.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and an isolated-target
  `cargo check --manifest-path src-tauri/Cargo.toml` pass.
- The host-local full Rust suite passes 571 library tests with 36 intentionally ignored, 1 updater-evidence test, and
  all 10 private-process integration tests. The sandboxed run failed only because the three existing image-download
  fixtures could not bind loopback listeners; the identical host-local rerun passed.
- `npm run format:check`, `npm run check`, `npm test` (360 passed, 3 skipped), and `npm run build` pass. No WebView
  presentation changed, so browser/native UI review is not applicable.
- No real model/runtime bytes, network downloader, non-fixture image worker, hardware probe, provider request, billable
  action, local generation, UI behavior, or user-approved Python execution was exercised.

## Next slice

Add a strict Rust-owned model source plan and resumable downloader that feeds `ModelCacheTransaction`. Bind every file
to an approved HTTPS repository root, immutable source revision, and declared portable path; disable redirects; accept
resume only when status, `Content-Range`, validator, offset, and remaining length agree; enforce per-file/package byte
and time ceilings; propagate cancellation; and publish progress only after cache bytes sync. Use injected loopback
responses to cover full and ranged downloads, restart resume, ignored/wrong ranges, validator or revision drift,
redirects, truncation/overflow, cancellation, timeouts, and cache cleanup.

Do not guess or claim a supported MLX-Gen package, hard-code unmeasured weights, download multi-gigabyte
model/runtime artifacts, execute a runtime, probe hardware, add Svelte IPC/UI, or add provider fallback in that slice.
A selected local package still requires separately reviewed official revisions, exact file hashes/sizes/license, and
measured disk, memory, output, and cancellation evidence. Do not claim worker network isolation until a real
runtime-specific process proof exists; load-boundary re-verification detects drift but does not make the cache
generally immutable.

Do not merge, dispatch workflows, sign, release, publish, distribute, or perform Store work without separate
authorization. Preserve unrelated untracked logo-kit, screenshot, and Linux public-key files.
