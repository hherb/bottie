# Bottie handover

Last verified: 2026-09-15

## Start here

`main` includes merged PR #181 at `8cc3c27`. Branch `codex/qwen-image-editing-ui` completes the hosted editing UI and
generated-ancestry selection slices. Read `ROADMAP.md` Milestone 8.5, `src/lib/image-generation.ts`,
`src/lib/ConversationView.svelte`, and `src/routes/page-state.svelte.ts`.

## Current state

- Cloud Image mode accepts one to three ready normalized current-draft images, persists their exact attachment IDs on
  the user request, and calls `startImageEditing` with ordered opaque IDs. No sources still uses text-to-image.
- Completed generated images in the visible selected lineage can be added or removed as references. Draft attachments
  precede generated sources; generated sources retain explicit selection order; the aggregate limit is three.
- The composer exposes reference-image picking, accessible selected/disabled states, exact hosted model identity, and
  prompt plus source-byte delivery/charge disclosure. Local 2512 editing stays disabled without fallback.
- Rust still owns source resolution, bytes, paths, hashes, provider traffic, exact ancestry revalidation, durable
  lineage, cancellation, download validation, and retry. Only path-free metadata and opaque IDs cross IPC.

## Validation and limits

Prettier, Svelte diagnostics (0 errors and 0 warnings), all 399 active frontend/script tests (3 skipped), the production
build, `cargo fmt --check`, and `cargo check` pass. The identical host-local Rust run passes all 646 active library tests
(36 ignored), updater evidence, all 16 private-worker integration tests, and doc tests. A browser-preview desktop review
confirmed the Cloud controls, disclosure, disabled over-limit state, and layout. No native app or live provider request
was run; no credentials, credits, model bytes, or source assets left the device. Unrelated untracked logo-kit,
screenshot, and Linux public-key files remain untouched.

## Next slice

Present durable edit lineage on each generated result using its existing ordered path-free `sources` metadata. Show a
compact accessible source count plus source type, dimensions, media type, and byte size without rendering opaque IDs or
adding native file access. Cover pure presentation and reopened-message rendering, including mixed attachment/generated
sources and ordinary text-to-image outputs with no lineage panel.

Do not add live DashScope calls, local editing, source-byte/path IPC, exact-2.0 weight assumptions, automatic fallback,
or new provider/storage contracts. Do not merge, dispatch workflows, sign, release, publish, distribute, or perform
Store work without separate authorization.
