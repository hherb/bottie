# Bottie handover

Last verified: 2026-09-06

## Start here

PR #158 merged into `main` at `3b72351`. The current branch is
`codex/linux-protected-python-inspection-containment`. Microsoft Store certification and publication remain deferred
until fresh release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- `python:protected:linux:prove-shipping` accepts only one source revision, already signed DEB, accepted candidate, and
  separate inspection/containment outputs. It does not build, sign, install, or mutate the supplied package.
- Before extraction, the producer strips signing, updater, GnuPG, and loader overrides, creates transient private trust
  roots from Bottie's checked-in public certificate and policy, and requires `debsig-verify` to accept the DEB.
- The extracted helper, runtime, and package-owned evidence marker must form a closed protected inspection whose full
  runtime identity matches the accepted Linux candidate. The fixed installed helper/runtime inspection must then equal
  that exact extracted inspection canonically.
- Only after signed-package verification and installed-byte equality does the producer reuse the existing installed
  Landlock/seccomp/rlimit proof for host-fixture, network, process, and exec denial, private-pipe execution, cancellation,
  and parent-close cleanup. It emits only the path-free protected inspection and an exact inspection-digest-bound
  shipping-containment record.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX,
Ollama, OpenAI-compatible, or Anthropic-compatible model. A configured remote provider receives the tool definition
and the source/purpose it proposes; execution remains local and requires exact one-use approval.

The Linux producer assumes the caller has separately installed the exact signed DEB and proves only its Python
helper/runtime identity and containment boundary. It is not wired into the protected Linux workflow, no signed Python
DEB is available on this macOS host, and no native Linux shipping proof was run. The existing macOS opt-in composition
also remains undispatched. No current protected Python distribution, installed production, release, updater,
publication, or Microsoft Store evidence was produced.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

The focused test first failed because the Linux producer module did not exist. Six new tests cover the closed
inspection-bound record, malformed and path-bearing native evidence, exact extracted/installed equality, public-only
signature verification, credential/loader stripping, verification-to-execution ordering, command registration, and
absence from both protected workflows. The related runtime, candidate, comparison, containment, and distribution suites
pass 46 tests.

Frontend formatting, type/Svelte checks, the production build, and the full test suite pass; the suite reports 329
tests passed and 3 skipped across 67 passing and 1 skipped files. Application Cargo formatting and `cargo check` pass.
The serial application test suite reaches 497 passed and 36 ignored; the same four unchanged macOS/Windows
private-process fixture tests time out or return their fixed helper failure at their unchanged boundaries. The
limitation is retained without altering the native transport contract.
Python-runner formatting, strict offline clippy, 8 tests with 3 runtime-dependent ignores, and the locked offline
release build pass. Dependency inventory, notice, release-asset, targeted Prettier, workflow-lint, command syntax, and
diff checks pass. Full `ROADMAP.md` Prettier retains its pre-existing unrelated formatting warning; the changed section
is formatted consistently.

No browser or native-app UI review is required for this packaging contract. No protected distribution workflow was
composed or dispatched, macOS cannot run the signed/installed Linux proof, and draft-PR hosted contract checks are
pending. Native signed/installed Linux evidence remains unavailable by design until the protected workflow is composed
and separately authorized for dispatch.

## Next bounded action

Add an optional same-revision Python composition to the manual protected Linux workflow. Recreate and inspect the
protected DEB before credentials, sign it through the existing path, install those exact final bytes, then run the
credential-free producer and `python:protected:compare`. Preserve the default workflow path, and do not dispatch the
protected workflow, release, publish, or perform Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
