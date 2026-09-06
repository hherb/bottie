# Bottie handover

Last verified: 2026-09-06

## Start here

PR #153 merged into `main` at `de8547f`. The current branch is `codex/python-evidence-binding`. Microsoft Store
certification and publication remain deferred until fresh release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- Each macOS, Windows, and Linux development-package proof now writes a closed marker for its exact checked-out Git
  revision beside the existing package and containment evidence. The marker accepts only a supported platform and one
  lowercase 40-character source revision.
- A final credential-free provenance job downloads the complete three-platform evidence set and fails closed on a
  missing platform, mixed revision, unknown field, unsupported target, incomplete containment result, invalid digest or
  byte count, unexpected native transport, changed installed Windows/Linux bytes, an inconsistent shared CPython/WASI
  runtime core, or an unexpected platform runtime layout.
- The accepted release-candidate evidence contains one source revision, one normalized shared runtime core, each exact
  platform runtime identity, canonical hashes of every accepted package/containment input, and only package-relative
  native transport metadata. It does not retain runner paths, host paths, identities, credentials, or raw command
  output.
- Default and protected package configurations are unchanged. The outer Bottie development app remains unsigned, and
  no Apple credential, distribution signature, notarization, release, publication, or Microsoft Store action is used.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX,
Ollama, OpenAI-compatible, or Anthropic-compatible model. A configured remote provider receives the tool definition
and the source/purpose it proposes; execution remains local and requires exact one-use approval. The new binding accepts
only credential-free development-package evidence. It does not establish shipping containment, protected signing,
notarization, installed production behavior, publication, or Microsoft Store certification.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

The initial focused test failed because the release-candidate evidence binder did not exist. The hosted aggregate then
correctly exposed that Windows adds one deterministic `python314.zip` and therefore cannot share the macOS/Linux runtime
tree hash. The regression fixture now preserves that 539-file versus 540-file distinction. Six focused tests cover the
exact source marker, deterministic accepted manifest, missing and mixed revisions, extracted/installed mismatch, shared
runtime-core mismatch, platform layout, incomplete containment, added path-bearing fields, and workflow wiring. The
related macOS, Windows, Linux, runtime-bundle, and binder suites pass 35 tests.

`npm run format:check`, `npm run check`, `npm test`, and `npm run build` pass: Svelte reports zero errors/warnings, and
302 frontend/script tests pass with 3 skipped. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
`cargo check --manifest-path src-tauri/Cargo.toml`, and `cargo test --manifest-path src-tauri/Cargo.toml` pass. The
application library reports 501 passed and 36 ignored, plus the updater evidence test and doc tests.

The Python runner's format, strict offline Clippy, offline tests, and locked offline release build pass; seven unit
tests and the explicit missing-runtime guard pass, while three runtime-dependent tests remain intentionally ignored.
Dependency inventory regeneration/check, third-party notices, release assets, Prettier, workflow lint, local unsigned
development-app build and package inspection, and `git diff --check` pass.

The exact aggregate evidence still requires the three GitHub-hosted package jobs; this macOS host cannot produce the
Windows AppContainer or Linux Landlock/seccomp installed-package results. No protected distribution workflow was
dispatched. Draft-PR hosted evidence is pending.

## Next bounded action

Add a credential-free comparison contract for future protected packages to prove that their bundled Python runtime
matches the accepted development release-candidate runtime identity while requiring separate platform-native shipping
containment evidence. Do not use credentials, dispatch protected workflows, sign, notarize, release, publish, or perform
Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
