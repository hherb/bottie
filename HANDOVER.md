# Bottie handover

Last verified: 2026-09-14

## Start here

PR #168 merged into `main` at `d2ae4be`. The current branch completes the first two bounded Milestone 8.3 foundations:
the closed private worker protocol and its pure long-lived manager lifecycle policy. Read Milestone 8.3 in `ROADMAP.md`
and `src-tauri/src/local_image_worker/` before adding transport behavior.

## Completed slices

- Protocol version 1 defines closed `hello`, `capabilities`, `load`, `generate`, `progress`, `cancel`, `result`, and
  `shutdown` schemas. Four-byte unsigned big-endian framing has a 1 MiB payload ceiling and supports fragmented and
  consecutive private-pipe frames.
- Every identity, prompt, dimension, count, capability, progress, model directory, output name, and safe worker error
  has an explicit bound. Unknown messages/fields, unsupported versions, malformed/truncated/oversized frames, invalid
  result relationships, duplicate terminal results, relative model directories, path-shaped errors, and
  credential-shaped errors fail closed without retaining raw detail.
- The pure Rust manager requires ordered hello/capability negotiation, keeps one verified model identity warm, admits
  one load or generation at a time, correlates progress/results to the exact operation, rejects stale or cross-request
  events, and clears process-specific state on failure or exit.
- Cooperative cancellation has a named three-second grace policy. A terminal result inside the grace restores idle
  state; expiry requires the future transport to kill and reap the worker. Clean shutdown is admitted only while idle.
- This subsystem remains separate from Bottie's user-approved Python tool runtime. Exact hosted
  `qwen-image-2.0-2026-03-03` remains distinct from local open-weight `Qwen/Qwen-Image-2512`.

## Validation

- `npm run format:check`, `npm run check`, `npm test`, and `npm run build` pass.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`, `cargo check --manifest-path src-tauri/Cargo.toml`, and
  the serialized Rust suite pass using isolated `/private/tmp/bottie-local-worker-target` because the standard target
  directory remains held by a pre-existing Cargo build lock.
- The host-local Rust suite passes 554 tests with 36 intentionally ignored, plus 1 updater-evidence test. Its initial
  sandboxed run failed only because three existing image-download fixtures could not bind loopback listeners; the
  identical host-local rerun passed. The focused local-worker suite passes 17 protocol and lifecycle tests.
- `git diff --check` and final self-review pass.
- No process, localhost listener, model/runtime load, hardware probe, local generation, UI behavior, provider request,
  or billable action was exercised. No WebView presentation changed, so browser/native UI review was not applicable.

## Next slice

Attach a private-process transport to the existing manager policy. Spawn only one exact trusted executable without a
shell, clear inherited environment and stdio, use its private stdin/stdout for the versioned frames, bound and discard
stderr, enforce handshake/read/write/shutdown deadlines, and make every protocol/order/EOF/timeout failure kill and reap
the child. Exercise the transport with a deterministic fixture worker, including fragmented frames, early exit, hung
handshake, malformed output, cooperative cancellation inside three seconds, forced teardown after three seconds, clean
shutdown, and absence of inherited environment values.

Do not bundle MLX-Gen, Python, Diffusers, model acquisition, network access, hardware probing, actual weight loading or
generation, Svelte IPC/UI, provider fallback, or the user-approved Python tool runtime into that slice. Do not claim
the roadmap's worker-manager item complete until process ownership, kill, and reap are connected and tested.

Do not merge, dispatch workflows, sign, release, publish, distribute, or perform Store work without separate
authorization. Preserve unrelated untracked logo-kit, screenshot, and Linux public-key files.
