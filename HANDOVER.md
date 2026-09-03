# Bottie handover

Last verified: 2026-09-03

## Start here

PR #135 is merged into `main` at `4efc0e7`. The next bounded Python-runtime provenance and development-package slice
is implemented on `codex/python-runtime-provenance`. Bottie still does not register a provider-visible Python tool,
launch the helper from Tauri, or select the development bundle inputs for protected distribution. No signing,
publication, release, or Microsoft Store action was taken.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`
4. `distribution/update/README.md` before returning to release work

## Completed slice

- `python-runner/runtime-manifest.json` now pins official CPython 3.14.7 source and SBOM inputs, WASI SDK 24,
  Wasmtime 45.0.3, and a fixed build/staging contract; the unofficial runtime remains compatibility-test-only.
- The credential-free pull-request workflow rebuilds official CPython/WASI twice at the same fixed path and requires
  byte-identical staged runtimes and path-free evidence before sharing that exact artifact with package jobs.
- A separate opt-in Tauri config bundles the exact runtime, evidence, and target-suffixed native helper into unsigned
  macOS, Windows, and Linux development packages; extraction verifies runtime-tree and helper digests.
- Dependency inventory covers both locked Cargo graphs across the four reviewed desktop targets with component
  ownership. The official CPython licence and runner dependencies are included in deterministic notices, with zero
  unknown or review-required inventory entries.
- The default and protected package configurations remain unchanged.

## Current limits

The hosted workflow is development-only. Its same-path double build does not prove cross-host reproducibility, and its
unsigned package checks prove byte placement rather than containment launch, installed behavior, signature, notarized
identity, or release-candidate binding. The development bundle does not expose a tool or run the helper from Bottie.

Platform containment proofs remain separate: transient App-Sandboxed XPC on macOS, transient zero-capability
AppContainer on Windows, and built-in Landlock/seccomp/rlimits on Linux. Product launch, cancellation, durable audit,
approval UI, provider mappings, and answer/context presentation remain pending.

Protected macOS publication still lacks its Apple distribution credentials, and protected Windows publication still
lacks its Authenticode PFX and password. Microsoft Store certification and publication remain deferred until fresh
release-owner notice.

## Validation

The local review passed formatting, Svelte checks, production build, 267 frontend/script tests (3 skipped), 446 Rust
application-library tests plus updater evidence (33 ignored), 8 standalone-runner tests (3 ignored), locked runner
Clippy/release build, offline dependency and notice gates, release-asset checks, workflow lint, and `git diff --check`.
A locally built official runtime contained 539 files (40,864,108 bytes) with tree digest
`293a02f7cc9bf01945c53a0fa68429cd7d7570b94da5bdde8502c857a2c97b2b`. An unsigned Apple-silicon `.app` built with
the opt-in config, and extracted-package inspection matched that runtime and the 14,273,328-byte helper exactly.

This is development-host evidence, not native containment, Windows/Linux package, installed-package, signing,
notarization, release, or Store evidence. See the draft PR checks for the credential-free hosted repeatability and
three-platform package evidence.

## Next bounded action

Define the approval-required native Python tool contract and user-visible source/purpose review without launching the
helper from Tauri yet. Keep provider schemas, product cancellation/audit, helper launch, answer presentation, protected
signing, publication, release, and Microsoft Store certification outside that slice.

Preserve the unrelated untracked logo-kit, screenshot, and Linux signing-public-key files. Do not merge the draft PR
without separate authorization.
