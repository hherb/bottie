# Bottie handover

Last verified: 2026-09-15

## Start here

`main` includes merged user-manual PR #185 and developer-manual PR #184 at `95f22fb`. Branch
`codex/qwen-image-edit-lineage-presentation` completes the remaining Milestone 8.5 durable edit-lineage presentation
slice. Read `ROADMAP.md` Milestone 8.5, `src/lib/GeneratedImageGallery.svelte`, `src/lib/image-generation.ts`, and
`src/routes/image-preview.ts`.

## Current state

- Every generated edit presents a compact native disclosure with its durable ordered source count, source kind,
  dimensions, media type, and byte size. Ordinary text-to-image results with no sources have no lineage panel.
- Pure presentation helpers and component coverage prove mixed attachment/generated sources without rendering opaque
  source IDs. Reopened-message coverage proves the existing native mapper retains the exact path-free source records.
- `ConversationView.svelte` delegates the cohesive generated-image surface to `GeneratedImageGallery.svelte` and is now
  comfortably below the practical 500-line limit. A development-only `?image=edit-lineage` fixture supports repeatable
  desktop and responsive review.
- An official-source check on 2026-09-15 found the Qwen Image 2.0 launch and technical report, but no exact 2.0 weight
  release in the official Qwen repository/model registries. Released 2512/2511 weights remain different models.

## Validation and limits

Prettier, Svelte diagnostics (0 errors and 0 warnings), all 407 active frontend/script tests (3 skipped), the production
build, `cargo fmt --check`, and `cargo check` pass. The identical host-local Rust run passes all 646 active library tests
(36 ignored), updater evidence, all 16 private-worker integration tests, and doc tests. Desktop and 540×800 browser
review confirmed the collapsed/expanded disclosure, ordered list, narrow layout, and ordinary generation without
lineage. No native app or live provider request was run because this is WebView-only; no credentials, credits, model
bytes, source assets, or private paths left the device. Unrelated untracked logo-kit, screenshot, and Linux public-key
files remain untouched.

## Next slice

Recheck only official Qwen repositories and model registries for an exact Qwen Image 2.0 weight release. If it exists,
freeze one immutable revision, complete file list and hashes, license, architecture metadata, and reference output
before selecting any MLX or CUDA/ROCm/Windows runtime. If it does not exist, make no local-2.0 product change; a paper,
API alias, community conversion, similarly named model, or 2512 package is not sufficient.

Do not add live DashScope calls, guessed 2.0 weights, local editing, source-byte/path IPC, automatic fallback, or a new
provider/storage contract. Do not merge, dispatch workflows, sign, release, publish, distribute, or perform Store work
without separate authorization.
