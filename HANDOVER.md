# Bottie handover

Last verified: 2026-09-14

## Start here

PR #166 merged into `main` at `7069f8f`. Draft PR #167 is open from `codex/generated-asset-actions`.

Read Milestone 8.2 in `ROADMAP.md`, then `src-tauri/src/storage/generated_asset_actions.rs`,
`src-tauri/src/storage/generated_assets/retry.rs`, `src-tauri/src/storage/portable_export.rs`, and
`src-tauri/src/storage/backup.rs`.

## Completed slices

- Schema version 24 binds each new assistant image response to its exact durable user prompt plus accepted width,
  height, and prompt-extension option; output count remains the ordered asset-row count.
- Initial generation now proves the WebView prompt matches the latest selected durable user request before inserting a
  pending assistant message. Failed and cancelled responses retry from only their opaque assistant-message identity,
  exact native request/options, output count, and provenance; active, completed, superseded, and non-selected responses
  fail closed.
- Completed selected-branch outputs have explicit open, copy, export, and deletion actions. Rust resolves and rehashes
  app-private PNGs for the native default viewer and Save dialog; clipboard copy fetches only the existing bounded PNG
  preview. Deletion requires native confirmation, updates multi-output message state, and tombstones/removes the
  content-addressed blob only after its final durable reference disappears.
- Generated-image messages no longer expose ordinary text-response rating, speech, copy, or regeneration actions.

No live or billable Model Studio generation was run. No local model/runtime, reference-image editing, release, signing,
publication, workflow dispatch, or Store action is included. Unrelated untracked logo-kit, screenshot, and Linux
public-key files remain untouched.

## Validation

Frontend formatting, Svelte checks, and the production build pass with no diagnostics. All 360 frontend tests pass with
3 skipped, including opaque PNG clipboard policy and completed/terminal generated-image action presentation.
The desktop browser preview was reviewed in Image mode at its default viewport; controls and disclosure remain clean.
The preview has no durable generated-image fixture, so action layout is covered by server-rendered component tests rather
than a populated browser/native interaction. No native Open, Save, clipboard, confirmation, or retry action is claimed.

`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and `cargo test --manifest-path src-tauri/Cargo.toml
--no-run` pass. The compiled Rust test binary still parks at macOS `_dyld_start` before the test harness reaches `main`;
`cargo clippy` likewise stalls while running a newly linked build script. This is a host execution limitation, not a
reported product-test failure; do not claim the Rust suite or Clippy passed until they run normally or hosted CI executes
them. Static review and `git diff --check` pass.

## Next slice

Finish Milestone 8.2 items 6–7 as one recovery-focused slice. Extend portable single/batch export bundles, manual and
automatic backup, restore validation, retention deletion, branch ownership, and restart-boundary garbage collection so
generated PNG bytes have the same exact selected-lineage and recovery guarantees as attachment bytes. Then recover a
pending image message whose process disappeared into a stable failed/interrupted state, and add fault tests for missing,
changed, orphaned, shared, superseded-branch, deleted-conversation, backup, and restore bytes.

Preserve schema-24 request identity and closed IPC: no native paths, content hashes, temporary provider URLs,
credentials, or provider correlation identifiers may cross into Svelte. Do not broaden generated assets into the user
attachment association. Keep Qwen-Image-2.0 local execution unavailable until exact 2.0 weights are officially
published and verified; keep the distinct `Qwen/Qwen-Image-2512` local track visibly separate.

Do not merge, dispatch workflows, sign, release, publish, or perform Store work without separate authorization.
