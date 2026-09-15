# Bottie handover

Last verified: 2026-09-15

## Start here

`main` includes merged PR #180 at `5e2fc86`. Branch `codex/qwen-image-editing-execution` completes the Rust-only hosted
Qwen-Image-2.0 editing execution and command slices. Read `ROADMAP.md` Milestone 8.5,
`src-tauri/src/image_generation/controller/editing.rs`, and `src/routes/page-state.svelte.ts`.

## Completed slices

- `ImageEditingProvider` and `DashScopeQwenImageProvider::edit` send the existing ordered native source bytes as exact
  Base64 data URIs followed by the edit instruction. They reuse the fixed endpoint and authentication, strict terminal
  response decoder, 256 KiB response ceiling, redirect-free bounded PNG downloader, and abortable hosted lifecycle.
- `start_image_editing` is a closed path-free Tauri command. It accepts only ordered opaque attachment or generated
  asset IDs, revalidates the selected request lineage and exact native bytes before insertion, stores source snapshots
  under every pending output, and emits the existing bounded image-run events.
- Failed and cancelled edits reopen and hash their identical per-output native source snapshot before exact retry.
  Text-to-image retry remains unchanged, while any source-bearing local retry fails closed without cloud fallback.

## Validation

Prettier, Svelte diagnostics (0 errors and 0 warnings), all 390 active frontend/script tests (3 skipped), the production
build, `cargo fmt --check`, and `cargo check` pass. The complete host-local Rust run passes all 646 active library tests
(36 ignored), the updater-evidence test, all 16 private-worker integration tests, and doc tests. This includes all 7
editing request/execution cases, both closed-command cases, both exact retry routes, and the storage reopen regression.
Loopback Rust tests require host-local execution because the sandbox denies listener binding.

No native app was launched and no live provider request was made. No credentials, credits, model bytes, or source
assets left the device. The command is registered and has a typed frontend wrapper, but no UI calls it. Unrelated
untracked logo-kit, screenshot, and Linux public-key files remain untouched.

## Next slice

Add the first hosted editing UI using only one to three ready normalized images attached to the current draft. In Cloud
Image mode, allow image picking, require every selected source to be ready and within the native count policy, persist
the exact attachment IDs on the user request, show that the prompt and source image bytes go to Alibaba Model Studio
and may incur charges, then invoke `startImageEditing` with the attachment IDs in visible order. Keep ordinary
text-to-image available when no sources are selected, and keep Local 2512 source selection disabled.

Cover pure eligibility/order policy, composer accessibility/disclosure, and page-state persistence/invocation tests.
Do not yet add selection of earlier generated ancestry, a live DashScope call, local editing, source-byte/path IPC, or
silent Cloud fallback. Do not merge, dispatch workflows, sign, release, publish, distribute, or perform Store work
without separate authorization.
