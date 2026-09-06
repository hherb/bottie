# Bottie handover

Last verified: 2026-09-06

## Start here

PR #157 merged into `main` at `92a64e6`. The current branch is `codex/macos-protected-python-composition`. Microsoft
Store certification and publication remain deferred until fresh release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- The manual macOS distribution workflow now has one optional prior-provenance run ID. It accepts only a successful
  `Python runtime provenance` run for the exact checked-out source revision, then downloads only its runtime, accepted
  candidate, and protected inspection artifacts.
- Before Apple credentials are exposed, the opt-in path rebuilds the helper and protected app through the existing
  credential-free producer and requires its inspection to equal the carried inspection byte for byte. Apple values are
  no longer job-wide; only credential verification/import and the selected signing step receive them.
- The exact `package:macos:distribution:python` mode revalidates the candidate, inspection, and staged app, then signs
  the runner, XPC service, and XPC client inside out with hardened runtime, secure timestamps, and their existing
  least-privilege entitlements before the outer Bottie app enters the unchanged notarization, stapling, Gatekeeper, and
  updater-evidence path.
- After final trust verification, the mode emits a fresh candidate-validated signed inspection. The workflow then runs
  the credential-free shipping-containment producer and `python:protected:compare` against those exact bytes and
  uploads only bounded path-free evidence. Leaving the input blank retains the existing standard distribution command,
  including the updater-publication caller.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX,
Ollama, OpenAI-compatible, or Anthropic-compatible model. A configured remote provider receives the tool definition
and the source/purpose it proposes; execution remains local and requires exact one-use approval. The locally available
macOS staging app is unsigned and not a protected distribution. The opt-in workflow was not dispatched, so no current
signed, notarized, stapled, Gatekeeper-accepted, shipping-containment, protected-package, or updater evidence was
produced.

The composition is a manual contract, not current distribution evidence. It does not establish installed production
behavior, release identity, publication, or Microsoft Store certification, and it adds no Windows or Linux shipping
evidence. No pull request, push, release, or automatic protected-workflow trigger was added.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

The focused tests first failed because the protected nested-signing plan, package command, and workflow composition did
not exist. Three new tests cover inside-out production signing, candidate/inspection validation before signing, final
inspection after notarization, the exact prior-run gate, pre-credential rebuild/equality, default-path preservation,
and final containment/comparison ordering. The related distribution, shipping-containment, and comparison suites pass
30 tests.

Frontend formatting, type/Svelte checks, the production build, and the full test suite pass; the suite reports 323
tests passed and 3 skipped across 66 passing and 1 skipped files. Application Cargo formatting and `cargo check` pass.
The serial application test suite reaches 497 passed and 36 ignored; the same four unchanged macOS/Windows
private-process fixture tests time out or return their fixed helper failure at their unchanged boundaries. The
limitation is retained without altering the native transport contract.
Python-runner formatting, strict offline clippy, tests, and the locked offline release build pass. Dependency inventory,
notice, release-asset, workflow-lint, targeted Prettier, and diff checks pass. Full `ROADMAP.md` Prettier retains its
pre-existing unrelated formatting warning; the changed documentation and code are formatted.

No browser or native-app UI review is required for this packaging contract. No protected distribution workflow was
dispatched, and draft-PR hosted evidence is pending.

## Next bounded action

Add a credential-free Linux protected-DEB inspection and installed-containment producer against an already signed
package. Reuse the existing Landlock/seccomp runner and fixed installed layout, but do not compose or dispatch the
protected Linux workflow, release, publish, or perform Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
