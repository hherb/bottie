# Bottie handover

Last verified: 2026-09-06

## Start here

PR #152 merged into `main` at `79dbb60`. The current branch is `codex/macos-packaged-python-smoke`. Microsoft Store
certification and publication remain deferred until fresh release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- The opt-in macOS development package now places the native XPC client as the main executable of a nested
  `BottiePythonXPCClient.app`, which owns the private service under its `Contents/XPCServices` directory. The Rust
  resolver requires that exact complete layout and fails closed when any client, service, helper, evidence, or runtime
  resource is missing.
- The credential-free provenance job creates a one-day self-signed code-signing identity in a transient keychain,
  signs runner -> service -> client app inside out without a timestamp, and only then trusts the public certificate for
  code signing on the disposable runner. Its bounded unconditional cleanup deletes and verifies removal of the system
  certificate before removing the keychain, certificate, private key, and archive.
- The packaged verifier inspects the exact signed client, service, helper, and runtime, verifies the same packaged code
  signatures, then exercises ordinary private-pipe execution, direct host-fixture denial, caller cancellation, and
  client-exit cleanup without rebuilding or substituting nested code. Uploaded evidence remains bounded and path-free.
- Default and protected package configurations are unchanged. The outer Bottie development app remains unsigned, and
  no Apple credential, distribution signature, notarization, release, publication, or Microsoft Store action is used.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX,
Ollama, OpenAI-compatible, or Anthropic-compatible model. A configured remote provider receives the tool definition
and the source/purpose it proposes; execution remains local and requires exact one-use approval. The new evidence is a
self-signed development-package proof on GitHub's disposable macOS runner. It does not establish shipping containment,
protected signing, release-candidate binding, notarization, installed production behavior, publication, or Microsoft
Store certification.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

The focused tests failed first because the packaged-smoke module did not exist. The next red cycle showed that the old
sidecar layout did not make the client the main executable of the bundle owning the XPC service; the Rust resolver and
package contracts failed until the nested client-app boundary was implemented. The completed macOS XPC, packaged-smoke,
runtime-bundle, and native resolver suites pass 22 focused tests.

`npm run format:check`, `npm run check`, `npm test`, and `npm run build` pass: Svelte reports zero errors/warnings, and
295 frontend/script tests pass with 3 skipped. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
`cargo check --manifest-path src-tauri/Cargo.toml`, and `cargo test --manifest-path src-tauri/Cargo.toml` pass. The
application library reports 501 passed and 36 ignored, plus the updater evidence test and doc tests.

The Python runner's format, strict offline Clippy, offline tests, and locked offline release build pass; seven unit tests
and the explicit missing-runtime guard pass, while three runtime-dependent tests remain intentionally ignored.
Dependency inventory regeneration/check, third-party notices, release assets, Prettier, workflow lint, local unsigned
development-app build and package inspection, and `git diff --check` pass.

This host does not permit the new self-signed identity to be added to system trust without interactive administrator
authorization, which was not requested. The exact packaged App Sandbox denial, execution, cancellation, and client-exit
proof therefore remains GitHub-hosted evidence and must pass on the draft PR before the slice is treated as complete.

## Next bounded action

Add a credential-free release-candidate binding contract over the existing path-free macOS, Windows, and Linux Python
package/containment evidence. Reject missing, mixed-revision, or inconsistent development evidence without creating
signatures, claiming shipping containment, changing default/protected package configs, notarizing, releasing,
publishing, or performing Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
