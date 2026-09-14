# Bottie handover

Last verified: 2026-09-14

## Start here

PR #169 merged into `main` at `2be34bf`. Branch `codex/local-image-worker-transport` completes the next two bounded
Milestone 8.3 foundations: private process ownership for the local image-worker manager and the explicit native model
acquisition/activation policy. Read `ROADMAP.md` Milestone 8.3 and `src-tauri/src/local_image_worker/` before
continuing.

## Completed slices

- `WorkerTransport` spawns one exact absolute executable without a shell, clears inherited environment and stdio,
  negotiates the existing protocol over private stdin/stdout, and continuously discards stderr under a 64 KiB ceiling.
  Handshake, event reads, frame writes, shutdown, and reaping have named deadlines.
- Fragmented and consecutive frames feed the existing strict lifecycle/correlation manager. Protocol, ordering, EOF,
  read/write, stderr, cancellation, and shutdown failures kill and reap the child and clear process-specific model and
  capability state. Cooperative cancellation retains the warm worker only when its terminal result arrives inside the
  existing three-second grace period.
- A deterministic Rust fixture process covers private-pipe negotiation, environment isolation, long-lived load and
  generation, early exit, hung handshake/operation/shutdown, malformed and out-of-order output, stderr overflow,
  cooperative cancellation, forced cancellation teardown, and clean shutdown. No localhost service is involved.
- `ModelAcquisition` validates bounded exact model/runtime/license/source/file metadata before approval, exposes only
  typed path-free status, requires monotonic download progress and explicit phase order, retains paths and hashes in
  Rust, and produces a worker `ModelLocation` only after every declared file passes exact size and SHA-256 verification.
  Portable file names are collision-checked case-insensitively, symlink escapes fail, and hosted-only
  `qwen-image-2.0-2026-03-03` cannot masquerade as a local package.
- Exact hosted Qwen Image 2.0 remains distinct from local open-weight `Qwen/Qwen-Image-2512`. The local image worker
  remains separate from Bottie's user-approved Python tool runtime.

## Validation

- Focused private-process suite: 8 passed.
- Focused model-acquisition suite: 6 passed on this macOS host, including the Unix symlink-escape case.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and
  `cargo check --manifest-path src-tauri/Cargo.toml` pass.
- The host-local full Rust suite passes 563 library tests with 36 intentionally ignored, 1 updater-evidence test, and
  all 8 private-process integration tests. Its initial sandboxed run failed only because the three existing
  image-download fixtures could not bind loopback listeners; the identical host-local rerun passed.
- `npm run format:check`, `npm run check`, `npm test` (360 passed, 3 skipped), and `npm run build` pass. No WebView
  presentation changed, so browser/native UI review is not applicable.
- No model/runtime package, process outside the deterministic fixture, localhost listener, hardware probe, provider
  request, billable action, local generation, UI behavior, or user-approved Python execution was exercised.

## Next slice

Add an app-owned transactional model cache behind the acquisition policy. Derive fixed staging and final directories
from a native manifest identity, accept writes only for declared portable paths, hash and size while streaming, resume
only an exact matching model/runtime/source manifest, discard drifted or failed partials, durably promote a completely
verified package, and reopen it through the existing all-files activation gate. Exercise interrupted writes, restart
resume, manifest drift, symlink/path escape, digest/size mismatch, atomic promotion, and cleanup with injected local
byte sources.

Do not bundle a network downloader, a guessed MLX-Gen package, model weights, runtime execution, hardware probing,
Svelte IPC/UI, provider fallback, or the user-approved Python tool runtime into that slice. The acquisition roadmap item
remains incomplete until a measured package manifest, downloader, and explicit presentation exist. Do not claim the
cache prevents post-verification mutation until the worker load boundary re-verifies or otherwise binds the exact bytes.

Do not merge, dispatch workflows, sign, release, publish, distribute, or perform Store work without separate
authorization. Preserve unrelated untracked logo-kit, screenshot, and Linux public-key files.
