# Bottie handover

Last verified: 2026-09-15

## Start here

`main` includes merged PR #179 at `fece17e`. Branch `codex/qwen-image-editing-foundation` completes the durable
Milestone 8.5 editing-lineage and offline request-serialization foundation. Read `ROADMAP.md` Milestone 8.5,
`src-tauri/src/storage/generated_assets/lineage.rs`, and `src-tauri/src/image_generation/dashscope.rs`.

## Completed slices

- Schema version 25 adds ordered source lineage beneath each generated output. One to three sources may come only from
  a ready normalized image attached to the exact current request or an earlier completed generated image in its
  selected ancestry.
- Acceptance re-reads and hashes native bytes, rejects malformed/cross-branch/duplicate content, caps each input at
  10 MiB and the aggregate at 30 MiB, and requires exact hosted `qwen-image-2.0-2026-03-03` Cloud provenance.
- Exact lineage survives reopen and retry. It blocks source deletion while referenced, participates in attachment
  retention and garbage collection, and carries ordered path-free metadata plus exact source bytes through selected
  export and verified backup/restore.
- The provider-neutral edit request fixes provenance and bounded generation options. Its offline DashScope serializer
  emits ordered Base64 data-URI images followed by one text item; no request is sent.

## Validation

Prettier, Svelte diagnostics (0 errors and 0 warnings), all 390 active frontend/script tests (3 skipped), the production
build, `cargo fmt --check`, and `cargo check` pass. The complete host-local Rust run passes all 638 active library tests
(36 ignored), the updater-evidence test, all 16 private-worker integration tests, and doc tests. This includes all 8 new
lineage/lifecycle cases, all 3 request/serialization cases, and all 11 historical schema upgrades.

No native app was launched and no provider request was made. No credentials, credits, model bytes, or source assets
left the device. Unrelated untracked logo-kit, screenshot, and Linux public-key files remain untouched.

## Next slice

Add the Rust-only hosted editing execution seam. Introduce a dedicated editing-provider contract and a DashScope
adapter method that consumes the existing `ImageEditingRequest`, reuses the fixed endpoint/authentication, strict
response decoder, and bounded downloader, and is covered by loopback fixtures for exact ordered data URIs,
cancellation, transport/status failures, and response-size limits. Keep this slice below the Tauri command boundary.

Do not make a live DashScope call, spend provider credits, expose a command or editing UI, forward native paths or
unapproved attachment bytes, implement local editing, or claim local Qwen-Image-2.0 weights. Do not merge, dispatch
workflows, sign, release, publish, distribute, or perform Store work without separate authorization.
