# Contributing to bottie

Keep changes as small, complete vertical slices that preserve the Rust/WebView security boundary described in
`HANDOVER.md`. Read `HANDOVER.md` and `ROADMAP.md` before starting product work.

## Coding rules

These rules apply to new code and to existing code touched by a change:

1. Docstrings are mandatory. Document modules, exported types, public APIs, and functions. Explain contracts and
   non-obvious policy rather than repeating the implementation.
2. Keep each source file under 500 lines where practical. Split by cohesive responsibility, not arbitrary line ranges.
3. Keep lines at or below 120 characters where practical. Generated data and indivisible values such as SVG paths may
   exceed the limit.
4. Prefer pure functions in reusable modules. Keep provider protocol normalization separate from I/O and keep reusable
   presentation logic separate from Svelte component state.
5. Do not use unexplained magic numbers. Give behavioral limits, timeouts, capacities, breakpoints, and conversion
   factors descriptive constant names.
6. Use test-driven development for all testable functionality: write a failing test, implement the smallest passing
   behavior, then refactor while the suite remains green. Bug fixes require a regression test when the behavior can be
   exercised automatically.
7. Complete every slice by updating its relevant documentation. Update `HANDOVER.md` by default; update `ROADMAP.md`,
   `README.md`, and existing developer or user-manual pages when their claims, commands, or workflows changed.

## Required checks

Run the checks relevant to the changed surfaces before handing off a slice:

```sh
npm run format:check
npm run check
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

Use the explicit Cargo manifest because the repository root is not a Cargo workspace. Live-provider tests remain opt-in
because they require local oMLX or Ollama services; their commands are documented in `README.md`.

For meaningful presentation changes, also inspect the browser preview at the desktop default and a relevant responsive
breakpoint. For native provider, credential, persistence, or cancellation changes, manually exercise the affected Tauri
flow before completing the slice.

## Pre-PR review

Before opening even a draft PR:

1. Inspect the complete local diff and worktree, run `git diff --check`, and confirm every intended new file is included
   while unrelated work remains unstaged.
2. Review the changed contracts adversarially for failure recovery, path and secret containment, cancellation, durable
   ordering, and platform-specific path, handle, permission, and filesystem semantics.
3. Add regression tests for every locally identified defect. Exercise each supported target locally when available;
   state unsupported-target gaps explicitly instead of inferring cross-platform success from the host build.
4. Rerun the relevant focused tests and the complete required check set after review fixes.
5. Stage only explicit reviewed paths and inspect `git diff --cached` before committing, pushing, or opening the PR.
