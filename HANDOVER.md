# Bottie handover

Last verified: 2026-09-06

## Start here

PR #159 merged into `main` at `2a285d5`. The current branch is
`codex/linux-protected-python-workflow`. Microsoft Store certification and publication remain deferred until fresh
release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- The manual `Linux distribution validation` workflow accepts an optional `python_provenance_run_id` while leaving its
  standard build, inspection, smoke, and signing path selected when the input is blank.
- The opt-in path accepts only a successful `Python runtime provenance` run for the exact checked-out revision, then
  downloads only its runtime and candidate artifacts. It rebuilds the locked helper and creates the Python-bearing
  product and isolated-smoke DEBs before signing credentials enter a command environment.
- `python:protected:inspect` candidate-validates the exact pre-credential product extraction. The existing Linux
  distribution path signs and verifies that retained DEB and its updater bytes before the workflow installs those exact
  final package bytes.
- The installed path runs `python:protected:linux:prove-shipping`, requires its signed-package inspection to equal the
  pre-credential inspection byte for byte, and then runs `python:protected:compare`. Only path-free protected evidence
  is retained, and the installed package, transient trust roots, package bytes, and Python inputs are always removed.
- The opt-in Linux package arguments live in a small pure configuration module; the default arguments are unchanged and
  remain covered by exact-array tests.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX,
Ollama, OpenAI-compatible, or Anthropic-compatible model. A configured remote provider receives the tool definition
and the source/purpose it proposes; execution remains local and requires exact one-use approval.

The protected Linux composition is manual and has not been dispatched. This macOS host cannot build, sign, install, or
exercise the Linux DEB, so the new evidence is contract-only until a separately authorized exact-revision workflow run.
The existing protected macOS composition also remains undispatched. No current protected Python distribution,
installed production, release, updater, publication, or Microsoft Store evidence was produced.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

The focused tests first failed because the two new package commands and workflow composition were absent. The Linux
package contract now passes 12 tests, including exact unchanged default arguments and opt-in Python product/smoke
arguments. The focused protected-package, Linux shipping-containment, and Linux distribution suites pass 26 tests,
including source-run validation, pre-credential recreation, signing/install/proof ordering, default-path preservation,
cleanup, and absence of automatic triggers.

Frontend formatting, type/Svelte checks, the production build, and the full test suite pass; the suite reports 329
tests passed and 3 skipped across 67 passing and 1 skipped files. Application Cargo formatting and `cargo check` pass.
The serial application suite passes 501 library tests with 36 ignored plus the updater-evidence binary test; doc tests
pass. Python-runner formatting, strict offline clippy, 8 tests with 3 runtime-dependent ignores, and the locked offline
release build pass.

Dependency inventory, notices, release assets, targeted documentation/workflow formatting, actionlint, JavaScript
syntax, and diff checks pass. Full `ROADMAP.md` Prettier retains its pre-existing unrelated formatting warning; only the
reviewed Python lines changed.

No browser or native-app UI review is required for this packaging/workflow contract. No protected workflow was
dispatched, and macOS cannot provide the signed/installed Linux proof. Draft-PR hosted contract checks remain the only
pending automated evidence; they do not create protected distribution evidence.

## Next bounded action

Add the credential-free Windows shipping-containment producer for an already signed and separately installed protected
MSI. Re-inspect the exact controller, helper, runtime, and package-owned marker, require installed equality, then reuse
the existing AppContainer denial, private-pipe, cancellation, and controller-cleanup proof. Do not compose or dispatch
the protected Windows workflow, sign, release, publish, or perform Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
