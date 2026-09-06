# Bottie handover

Last verified: 2026-09-07

## Start here

PR #160 merged into `main` at `965be0e`. The current branch is
`codex/windows-shipping-python-containment`. Microsoft Store certification and publication remain deferred until fresh
release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- `python:protected:windows:prove-shipping` accepts only one source revision, already signed MSI, separately installed
  application root, accepted candidate, and separate inspection/containment outputs. It cannot build, sign, install, or
  mutate the supplied package.
- The producer requires one caller-selected absolute Windows SDK SignTool, removes that value plus signing, updater,
  Python-runtime, Node, .NET profiling, and startup-hook overrides from every child, and verifies the MSI before
  administrative extraction.
- The extracted Bottie executable, AppContainer controller, and Python runner are verified independently. The extracted
  controller, runner, runtime, and package-owned marker must form a closed inspection whose runtime matches the accepted
  Windows candidate, then the same installed resources must equal that inspection canonically.
- Only after those checks pass does the producer reuse the existing installed zero-capability AppContainer proof for low
  integrity, stripped privileges, host-fixture denial, resource limits, private-pipe execution, cancellation, and
  controller-close cleanup. It emits only path-free exact-inspection-bound evidence.
- The Windows AppContainer pull-request workflow runs the new runtime-free contract tests but does not invoke the
  shipping producer or alter any protected distribution path.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX,
Ollama, OpenAI-compatible, or Anthropic-compatible model. A configured remote provider receives the tool definition
and the source/purpose it proposes; execution remains local and requires exact one-use approval.

The Windows producer is not composed into the manual protected workflow. This macOS host cannot verify, extract,
install, or exercise the Windows MSI, so the new evidence is contract-only until a separately authorized exact-revision
workflow run. The existing protected macOS and Linux compositions remain undispatched. No current protected Python
distribution, installed production, release, updater, publication, or Microsoft Store evidence was produced.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

The focused test first failed because the Windows producer module was absent. Six new tests cover the closed
inspection-bound record, malformed and path-bearing native evidence, exact extracted/installed equality, credential and
process-injection stripping, independent package/executable verification, verification-to-execution ordering, command
registration, pull-request contract coverage, and absence from the protected Windows workflow. The related Windows
AppContainer, runtime, candidate, comparison, and distribution suites pass 42 tests.

Frontend formatting, type/Svelte checks, the production build, and the full test suite pass; the suite reports 335
tests passed and 3 skipped across 68 passing and 1 skipped files. Application Cargo formatting and `cargo check` pass.
The serial application suite passes 501 library tests with 36 ignored plus the updater-evidence binary test; doc tests
pass. Python-runner formatting, strict offline clippy, 8 tests with 3 runtime-dependent ignores, and the locked offline
release build pass.

Dependency inventory, notices, release assets, targeted documentation/workflow formatting, actionlint, JavaScript
syntax, and diff checks pass. Full `ROADMAP.md` Prettier retains its pre-existing unrelated formatting warning; only the
reviewed Python lines changed.

No browser or native-app UI review is required for this packaging contract. No protected workflow was composed or
dispatched, and macOS cannot provide the signed/installed Windows proof. Draft-PR hosted contract checks remain the only
pending automated evidence; they do not create protected distribution evidence.

## Next bounded action

Add an optional same-revision Python composition to the manual protected Windows workflow. Recreate and inspect the
protected MSI before credentials, sign it through the existing Authenticode and updater path, install those exact final
bytes, then run the credential-free producer and `python:protected:compare`. Preserve the default workflow path, and do
not dispatch the protected workflow, release, publish, or perform Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
