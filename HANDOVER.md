# Bottie handover

Last verified: 2026-09-07

## Start here

PR #161 merged into `main` at `7bd47ca`. The current branch is
`codex/windows-protected-python-workflow`. Microsoft Store certification and publication remain deferred until fresh
release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- The manual Windows distribution workflow accepts an optional prior Python provenance run ID while retaining the
  existing standard path when the input is blank. The selected run must be a successful `Python runtime provenance`
  run for the exact checked-out source revision.
- Before signing credentials enter any command environment, the opt-in path downloads only the accepted runtime and
  candidate, rebuilds the locked runner and AppContainer controller, creates the Python-bearing MSI, extracts it
  administratively, and validates a closed unsigned inspection against the candidate.
- `package:windows:distribution:python` signs and independently verifies the staged AppContainer controller and runner,
  refreshes only the runner size/digest in the closed package-owned marker, then reuses the existing Authenticode and
  updater path for Bottie's executable and the final MSI.
- The workflow installs the exact exported final MSI into a fresh application directory, invokes the credential-free
  shipping producer, and runs `python:protected:compare`. Only path-free distribution, signed inspection, containment,
  and comparison evidence plus the existing one-day updater bytes can leave the job.
- Installed product state, certificate material, updater bytes, provenance inputs, and intermediate Python evidence are
  removed after the run. No automatic trigger, default-path change, release action, or Store action was added.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX,
Ollama, OpenAI-compatible, or Anthropic-compatible model. A configured remote provider receives the tool definition
and the source/purpose it proposes; execution remains local and requires exact one-use approval.

This macOS host cannot verify, extract, install, or exercise the Windows MSI, so the composition is contract-only until
a separately authorized exact-revision workflow run. The protected macOS, Linux, and Windows compositions remain
undispatched. No current protected Python distribution, installed production, release, updater publication, or
Microsoft Store evidence was produced.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

The focused tests first failed because the protected distribution helpers, package command, and workflow composition
were absent. Tests now cover the opt-in Tauri arguments, exact nested-code signing plan, closed signed-runner marker
update, signing-before-build order, same-revision provenance gate, pre-credential inspection, unchanged default path,
installed shipping proof/comparison, bounded evidence upload, and cleanup. The related Windows distribution,
shipping-containment, protected-comparison, and release-candidate suites pass 28 tests.

Frontend formatting, type/Svelte checks, the production build, and the full test suite pass; the suite reports 340
tests passed and 3 skipped across 68 passing and 1 skipped files. Application Cargo formatting and `cargo check` pass.
The serial application suite passes 501 library tests with 36 ignored plus the updater-evidence binary test; doc tests
pass. Python-runner formatting, strict offline clippy, 8 tests with 3 runtime-dependent ignores, and the locked offline
release build pass.

The broader Windows AppContainer, runtime, candidate, comparison, shipping, and distribution suites pass 47 tests; the
Windows package contract passes 12 tests. Dependency inventory, notices, release assets, targeted documentation and
workflow formatting, Actionlint, JavaScript syntax, and diff checks pass.

No browser or native-app UI review is required for this packaging/workflow contract. No protected workflow was
dispatched, and macOS cannot provide the signed/installed Windows proof. Hosted pull-request contract checks will be the
only Windows-native evidence for this code slice; they will not create protected distribution evidence.

## Next bounded action

Add a credential-free aggregate contract for the three accepted protected-platform comparison records. Require one
exact source revision, the complete macOS/Windows/Linux set, canonical inspection/containment bindings, and one shared
accepted runtime core while retaining each platform's signed native identities. Do not dispatch protected workflows,
sign, release, publish, or perform Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
