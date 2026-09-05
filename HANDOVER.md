# Bottie handover

Last verified: 2026-09-05

## Start here

PR #151 merged into `main` at `6226606`. The current branch is `codex/windows-installed-python-smoke`. Microsoft Store
certification and publication remain deferred until fresh release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- The credential-free runtime-provenance workflow now installs the exact unsigned Windows development MSI after its
  existing administrative extraction and package inspection.
- Extracted and installed evidence now includes each native transport's package-relative name, size, and digest. The
  Windows job requires the installed controller, helper, and runtime result to equal the extracted package result.
- The installed proof copies only those installed resources into a transient AppContainer-owned tree and exercises the
  existing zero-capability, low-integrity, privilege-stripped, host-fixture-denial, private-pipe, cancellation, and
  controller-close contract without rebuilding or substituting packaged native code.
- Default and protected package configurations are unchanged. The workflow remains pull-request-scoped,
  credential-free, and path-free in its uploaded evidence.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX,
Ollama, OpenAI-compatible, or Anthropic-compatible model. A configured remote provider receives the tool definition
and the source/purpose it proposes; execution remains local and requires exact one-use approval. The new evidence is
limited to an unsigned installed Windows development MSI on GitHub's Windows runner. It does not establish a shipping
package, protected signing, release-candidate binding, publication, or Microsoft Store action.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

The focused tests failed first because the installed Windows bundle resolver, native-transport identity evidence, npm
entry point, and workflow step did not exist. The completed AppContainer and Python bundle suites pass 18 tests
covering absolute installed paths, credential-free workflow policy, exact extracted/installed inspection comparison,
and the existing native denial and lifecycle contract.

`npm run format:check`, `npm run check`, `npm test`, and `npm run build` passed: Svelte reported zero errors/warnings,
and 290 frontend/script tests passed with 3 skipped. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
`cargo check --manifest-path src-tauri/Cargo.toml`, and `cargo test --manifest-path src-tauri/Cargo.toml` passed. The
application library reported 501 passed and 36 ignored, plus the updater evidence test and doc tests.

The Python runner's format, strict offline Clippy, offline tests, and locked offline release build passed; seven unit
tests and the explicit missing-runtime guard passed, while three runtime-dependent tests remained intentionally
ignored. Dependency inventory regeneration/check, third-party notices, release assets, workflow lint, and
`git diff --check` passed.

This macOS host cannot install or execute the Windows MSI. The exact installed-resource identity, native denial,
execution, cancellation, and controller-exit proof remains GitHub-hosted evidence and must pass on the draft PR before
the slice is treated as complete.

## Next bounded action

Add a credential-free macOS packaged-development-app XPC smoke for the exact inspected client, service, helper, and
runtime. Prove package-byte identity, the existing App Sandbox/host-fixture denial contract, ordinary private-pipe
execution, caller cancellation, and client-exit cleanup without rebuilding or substituting nested code. Do not change
default/protected package configs, claim shipping containment, sign for distribution, notarize, release, publish, or
perform Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
