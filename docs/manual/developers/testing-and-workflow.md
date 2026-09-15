# Testing and change workflow

[Back to the manual index](index.md)

## Complete vertical slices

A feature may require native domain behavior, Tauri DTO/command, frontend adapter/state/UI, failure and cancellation
behavior, tests, and docs. Do not substitute a frontend security check for native enforcement.

Before coding: read handover/roadmap/contribution docs, find a similar tested feature, enumerate affected boundaries,
write acceptance/failure/restart/platform cases, identify durability/permissions, and begin with a failing test when
possible.

## Test layers

| Layer | Tool/location | Purpose |
| --- | --- | --- |
| Presentation | colocated `*.test.ts`, Vitest | mapping, bounds, derived/state behavior |
| Components | colocated tests, Vitest | events, semantics, focus, visible states |
| Native | `src-tauri/src/**/*tests*.rs`, Cargo | policy, storage, protocols, cancellation |
| Integration | `src-tauri/tests/`, Cargo | crate/worker boundaries |
| Scripts | `scripts/*.test.mjs` | packaging, evidence, release contracts |
| Manual | `npm run tauri dev` | vault, dialogs, provider/audio/platform flows |
| Performance | `npm run performance:test` | explicit budget tests |
| Platform | `.github/workflows/` | package/containment on each OS |

Tests are deterministic/offline by default. Use temp stores/directories, local servers, fake clocks/transports/providers,
and immutable fixtures—never developer credentials or real app data.

## Required baseline

```sh
npm run format:check
npm run check
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

Run all seven for normal code. Documentation-only work may use proportionate link/diff checks. Meaningful presentation
changes also need desktop/responsive visual inspection; provider, credential, persistence, or cancellation changes need
manual Tauri exercise.

When relevant:

```sh
npm run performance:test
npm run dependencies:check
npm run notices:check
npm run release:assets:check
npm run icons:check
npm run python:bundle:test
npm run update:contract:test
```

Packaging can require platform tools/signing. Run contract tests on ordinary hosts and report environment gaps.

## Test design

Assert stable contracts: returned state, persisted records, safe serialization, normalized events, rendered semantics,
or cleanup. Avoid huge snapshots and private helper call counts. Cover empty/zero, exact max, one over max, malformed
Unicode/JSON, duplicate/wrong IDs, invalid state, stale/concurrent requests, cancellation, transport failure, and restart
recovery where applicable.

Prettier covers frontend/scripts/static code; rustfmt covers Rust. Markdown is not in the format script: keep ordered
headings, relative links, labelled code fences, readable tables, and roughly 120-character lines. Docstrings are
mandatory. Update `HANDOVER.md` by default for product slices and every user/developer document whose claim changed.

## Pre-commit review

```sh
git status --short
git diff --check
git diff --stat
git diff
git diff --cached --check
git diff --cached
```

Review adversarially. Exclude keys, local paths, caches, downloads, editor files, and unrelated changes. Stage explicit
paths. PRs should explain problem/solution, security and durability, tests/manual checks, and actual platform gaps.

A slice is done only when success/failure/cancellation/restart are intentional, native bounds are tested, lifecycle and
export/backup semantics are complete, UI is accessible/responsive, required checks pass, platform limitations are
honest, docs agree, and the staged diff contains only reviewed files.
