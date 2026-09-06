# Bottie handover

Last verified: 2026-09-06

## Start here

PR #155 merged into `main` at `29b2d9e`. The current branch is `codex/macos-protected-python-staging`. Microsoft Store
certification and publication remain deferred until fresh release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- A dedicated opt-in macOS overlay now composes the existing updater configuration with staged Python resources only
  for the credential-free protected-package producer. Bottie's default configuration and existing protected
  distribution workflow remain unchanged.
- The producer copies only the exact source runner, runtime tree, and runtime evidence into a fresh ignored staging
  root. It rejects unsupported targets, equal or nested source/destination roots, non-regular inputs, and links or
  special entries anywhere in the runtime before replacing stale staged bytes.
- Every Bottie, Tauri, Apple, updater, and platform signing environment value is removed before the producer invokes
  Rust, Swift, frontend, or Tauri build tools. The application build is app-only, locked, non-interactive, and explicitly
  `--no-sign`.
- The packaged client, service, runner, and runtime are independently inspected into the existing bounded path-free
  shape. A new comparison seam validates that inspection against the exact accepted macOS candidate runtime before any
  separate containment evidence exists. The credential-free pull-request workflow runs this producer only after the
  three-platform candidate is accepted and uploads only the inspection JSON.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX,
Ollama, OpenAI-compatible, or Anthropic-compatible model. A configured remote provider receives the tool definition
and the source/purpose it proposes; execution remains local and requires exact one-use approval. The new macOS app is
an unsigned staging artifact, not a protected distribution. No platform has produced a shipping containment record,
and the existing protected macOS workflow does not select this overlay. The producer does not establish nested signing,
App Sandbox launch/denial, cancellation, client-exit cleanup, notarization, Gatekeeper acceptance, installed production
behavior, release identity, publication, or Microsoft Store certification.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

The focused test first failed because the producer module, protected overlay, workflow job, and pre-containment
inspection validator did not exist. Thirteen focused tests cover exact locked build composition, credential scrubbing,
fresh staging, unsupported targets, top-level and nested links, local command/overlay wiring, post-candidate workflow
ordering, normalized inspection validation, runtime drift, and the existing closed comparison contract. The six related
runtime, XPC, development-package, candidate, comparison, and producer suites pass 38 tests.

The real local producer built the outer app with signing explicitly skipped, created only the opt-in app and updater
archive, and returned a path-free macOS inspection. It records the exact four package-relative XPC transport entries,
the 14,273,328-byte runner, and the accepted 539-file, 40,864,108-byte runtime with tree digest
`293a02f7cc9bf01945c53a0fa68429cd7d7570b94da5bdde8502c857a2c97b2b`. This is package identity evidence only.

Frontend formatting, type/Svelte checks, the production build, and the full test suite pass; the suite reports 315
tests passed and 3 skipped across 65 passing and 1 skipped files. Application Cargo formatting and `cargo check` pass.
The application test suite reaches 497 passed and 36 ignored, but four unchanged private-process fixture tests time out
under the default parallel runner; a serial rerun reaches 498 passed and 36 ignored with three of those fixture tests
still timing out. Executing the generated closed-pipe fixture directly in a clean environment succeeds, so the local
limitation is recorded without weakening or changing the transport contract. Python-runner formatting, strict offline
clippy, tests, and the locked offline release build pass. Dependency inventory, notice, release-asset, targeted Prettier,
workflow-lint, and diff checks pass.

No browser or native-app review is required for this packaging contract. No protected distribution workflow was
dispatched, and draft-PR hosted evidence is pending.

## Next bounded action

Add a credential-free macOS shipping-containment producer that consumes an already signed protected app plus its exact
inspection, reruns the packaged App Sandbox denial and lifecycle checks, and emits the closed inspection-bound shipping
record. Do not make it sign or notarize bytes, dispatch protected workflows, release, publish, or perform Microsoft
Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
