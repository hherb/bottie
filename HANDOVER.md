# Bottie handover

Last verified: 2026-09-05

## Start here

PR #150 merged into `main` at `67268c1`. The current branch is `codex/linux-installed-python-smoke`. Microsoft Store
certification and publication remain deferred until fresh release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- The credential-free runtime-provenance workflow now installs the exact unsigned Linux development DEB after its
  existing extraction and package inspection.
- The installed `/usr/bin` helper and `/usr/lib/bottie` runtime are reinspected against the package-owned evidence;
  the workflow requires that path-free result to be byte-identical to the pre-install inspection.
- The existing Linux Landlock, seccomp, rlimit, private-environment, and parent-death verifier can now run directly
  against those fixed installed resources. It proves the existing denial contract, ordinary private-pipe execution,
  caller cancellation, and parent-exit cleanup without rebuilding or substituting the helper.
- Default and protected package configurations are unchanged. The workflow remains pull-request-scoped,
  credential-free, and path-free in its uploaded evidence.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX,
Ollama, OpenAI-compatible, or Anthropic-compatible model. A configured remote provider receives the tool definition
and the source/purpose it proposes; execution remains local and requires exact one-use approval. The new evidence is
limited to an unsigned installed Linux development DEB on GitHub's Ubuntu runner. It does not establish a shipping
package, protected signing, notarization, release-candidate binding, publication, or Microsoft Store action.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

The two focused tests failed first because the installed fixed-path resolver and workflow step did not exist. The
completed Linux containment and Python bundle suites pass 14 tests covering fixed installed paths, credential-free
workflow policy, exact extracted/installed inspection comparison, and the existing native denial and lifecycle
contract.

`npm run format:check`, `npm run check`, `npm test`, and `npm run build` passed: Svelte reported zero errors/warnings,
and 288 frontend/script tests passed with 3 skipped. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
`cargo check --manifest-path src-tauri/Cargo.toml`, and `cargo test --manifest-path src-tauri/Cargo.toml` passed. The
application library reported 501 passed and 36 ignored, plus the updater evidence test and doc tests.

The Python runner's format, strict offline Clippy, offline tests, and locked offline release build passed; seven unit
tests and the explicit missing-runtime guard passed, while three runtime-dependent tests remained intentionally
ignored. Dependency inventory, third-party notices, release assets, workflow lint, documentation formatting, and
`git diff --check` passed.

This macOS host cannot install or execute the Linux DEB. The exact installed-resource identity, native denial,
execution, cancellation, and parent-exit proof remains GitHub-hosted evidence and must pass on the draft PR before the
slice is treated as complete.

## Next bounded action

Add a credential-free Windows installed-development-MSI AppContainer smoke for the exact packaged controller, helper,
and runtime. Prove installed byte identity, the existing zero-capability/token/host-fixture denial contract, ordinary
private-pipe execution, caller cancellation, and controller-exit cleanup. Do not change default/protected package
configs, claim shipping containment, sign, release, publish, or perform Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
