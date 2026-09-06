# Bottie handover

Last verified: 2026-09-06

## Start here

PR #156 merged into `main` at `0d7ea02`. The current branch is `codex/macos-shipping-python-containment`. Microsoft
Store certification and publication remain deferred until fresh release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- A new local credential-free macOS shipping-containment producer accepts only an exact source revision, already signed
  protected app, its closed path-free inspection, and an output path. It does not build or change application bytes.
- The producer re-inspects the packaged client, service, runner, and runtime before execution. Both the supplied and
  fresh inspection pass the existing closed macOS validator, and their canonical inspection digests must agree.
- The packaged runner, XPC service, XPC client, and outer Bottie app are verified independently with strict `codesign`
  checks and no recursive verification. Only then does the producer reuse the existing private-pipe execution,
  cancellation, App Sandbox host-fixture denial, and client-exit cleanup proof.
- Bottie, Apple, and Tauri signing environment values plus dynamic-loader and code-signing tool overrides are removed
  from verification and proof children. Signature checks use the fixed system `codesign` executable. The output is the
  existing closed shipping record bound to the source revision, target, and protected-inspection digest; identities,
  credentials, host paths, and raw command output are absent.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX,
Ollama, OpenAI-compatible, or Anthropic-compatible model. A configured remote provider receives the tool definition
and the source/purpose it proposes; execution remains local and requires exact one-use approval. The locally available
macOS app is the unsigned PR #156 staging artifact, not a protected distribution, and strict signature verification
rejects it as expected. No shipping containment record was produced. The existing protected macOS workflow neither
selects the Python overlay nor invokes the new producer or comparison gate.

The producer contract does not establish or perform signing, notarization, stapling, Gatekeeper acceptance, installed
production behavior, release identity, workflow dispatch, publication, or Microsoft Store certification. It adds no
Windows or Linux shipping evidence.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

The focused test first failed because the shipping-containment producer did not exist. Five new tests cover the exact
closed output, canonical reinspection equality, stale and expanded inspection rejection, source-revision validation,
independent non-signing verification, local command registration, dependency inventory, and absence from hosted
workflow dispatch. The five related producer, XPC, packaged-smoke, inspection, and comparison suites pass 30 tests.

Read-only inspection of the existing unsigned app still records the exact four package-relative XPC transport entries,
the 14,273,328-byte runner, and the accepted 539-file, 40,864,108-byte runtime with tree digest
`293a02f7cc9bf01945c53a0fa68429cd7d7570b94da5bdde8502c857a2c97b2b`. Strict `codesign` verification rejects that
staging app, so the new proof correctly cannot run locally and no containment record is claimed.

Frontend formatting, type/Svelte checks, the production build, and the full test suite pass; the suite reports 320
tests passed and 3 skipped across 66 passing and 1 skipped files. Application Cargo formatting and `cargo check` pass.
The serial application test suite reaches 497 passed and 36 ignored; the same four unchanged macOS/Windows
private-process fixture tests time out or return their fixed helper failure, including an isolated macOS private-pipe
rerun at its unchanged 45-second boundary. The limitation is retained without altering the native transport contract.
Python-runner formatting, strict offline clippy, tests, and the locked offline release build pass. Dependency inventory,
notice, release-asset, workflow-lint, targeted Prettier, and diff checks pass.

No browser or native-app UI review is required for this packaging contract. No protected distribution workflow was
changed or dispatched, and draft-PR hosted evidence is pending.

## Next bounded action

Add an opt-in composition to the existing protected macOS distribution path that carries the accepted candidate and
protected inspection through final app signing, notarization, stapling, and Gatekeeper verification, then invokes this
credential-free producer and `python:protected:compare` against those exact bytes. Do not dispatch the protected
workflow, change the default distribution path, release, publish, or perform Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
