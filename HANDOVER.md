# Bottie handover

Last verified: 2026-09-13

## Start here

PR #165 merged into `main` at `ece9518`. The current branch is `codex/durable-image-generation`.

Read Milestone 8.2 in `ROADMAP.md`, then `src-tauri/src/storage/generated_assets.rs`,
`src-tauri/src/image_generation/controller.rs`, and `src/routes/page-state.svelte.ts`.

## Completed slices

- Schema version 23 owns generated assets from assistant messages and persists ordered lifecycle state, content hash,
  PNG metadata, exact provider/model/execution provenance, optional seed, and path-free errors.
- The Rust controller admits one image run, creates the pending assistant message before provider I/O, supports exact
  cancellation shared with microphone capture, and emits bounded path-free progress and terminal events.
- Hosted Qwen-Image-2.0 results are downloaded immediately inside Rust with redirects disabled, HTTPS-only production
  URLs, fixed time and 25 MiB limits, exact PNG type/signature/decode/dimension checks, metadata-free normalization,
  all-or-nothing cleanup, and content-addressed app-private storage.
- The separate composer Image mode exposes prompt, aspect ratio, count, exact cloud checkpoint, delivery/cost disclosure,
  and cancellation. Pending, completed, failed, and cancelled assistant image messages survive reopen; completed images
  use natural-ratio previews over an opaque GET-only protocol with no paths, URLs, hashes, or provider correlation
  identifiers in IPC. Image prompts are normalized and bounded before durable user-message insertion, then revalidated
  authoritatively by Rust.

No live or billable Model Studio generation was run. No local model/runtime, reference-image editing, release, signing,
publication, workflow dispatch, or Store action is included. Unrelated untracked logo-kit, screenshot, and Linux
public-key files remain untouched.

## Validation

Frontend formatting, type checks, build, dependency/icon checks, and all 357 tests pass with 3 skipped. The serial Rust
suite passes 522 library tests with 36 ignored, the updater-evidence test, and doc tests; formatting and `cargo check`
pass with only the existing `block 0.1.6` future-incompatibility notice. The development-signed native launch reached
the Bottie binary, and immutable inspection reports schema 23, `quick_check` `ok`, and the generated-assets table.

The desktop browser preview was reviewed in Image mode: exact checkpoint, aspect ratio/count controls, cloud/cost
disclosure, disabled attachment action, and editing boundary render cleanly. Native-window accessibility inspection was
unavailable, so no native interaction is claimed.

## Next slice

Finish Milestone 8.2 item 5 with generated-asset actions and ownership, without widening IPC. First add migration 24 for
the accepted request dimensions/options so failed and cancelled messages can be retried exactly from the preceding
durable user prompt; output count already survives as ordered asset rows. Then add explicit retry plus native
open/copy/export and deletion flows that use only opaque asset/message identities. Revalidate selected-branch ownership
and ensure actions cannot expose native paths, content hashes, temporary provider URLs, credentials, or provider
correlation identifiers.

After that, extend portable export, backup/restore, retention, branch ownership, deletion, and garbage collection so
generated blobs have the same recovery guarantees as attachment bytes. Add startup recovery for a pending image message
whose process disappeared, and fault/cancellation tests at each new boundary.

Keep Qwen-Image-2.0 local execution unavailable until exact 2.0 weights are officially published and verified. Keep the
distinct open `Qwen/Qwen-Image-2512` local track visibly separate. App-store and distribution work remain paused while
Milestone 8 is the active product priority.

Do not merge, dispatch workflows, sign, release, publish, or perform Store work without separate authorization.
