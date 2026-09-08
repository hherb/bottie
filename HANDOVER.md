# Bottie handover

Last verified: 2026-09-08

## Start here

PR #162 merged into `main` at `d394a5c`. The current branch is
`codex/protected-python-aggregate-comparison`. Microsoft Store certification and publication remain deferred until
fresh release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slices

- `python:protected:bind-platforms` consumes fixed Linux, macOS, and Windows accepted comparison filenames and
  independently revalidates their closed schemas, exact source revision, accepted candidate digest, reconstructed
  protected-inspection hashes, shipping-containment hashes, shared CPython/WASI core, and platform runtime layouts.
  Its path-free aggregate retains every signed runner and native transport identity.
- The manual `Protected Python platform evidence` workflow accepts three explicit run IDs. It requires successful
  manual Linux/macOS/Windows distribution runs at the checked-out revision, downloads dedicated one-file comparison
  artifacts, rejects unexpected filenames/counts, runs the binder, and uploads only the aggregate for seven days.
  It has read-only repository/action permissions and no protected environment or secrets.
- `python:protected:release-eligibility` revalidates one closed, fully passed ordinary Bottie release-candidate
  manifest and the complete same-revision protected aggregate. It emits canonical input hashes plus versioned release
  metadata, the shared runtime core, and signed native identities without building or publishing anything.
- The dependency inventory covers both new scripts and the aggregate/producer workflows. No default distribution path,
  provider behavior, protected trigger, credential flow, signing step, release path, publication path, or Store path
  changed.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX,
Ollama, OpenAI-compatible, or Anthropic-compatible model. A configured remote provider receives the tool definition
and the source/purpose it proposes; execution remains local and requires exact one-use approval.

The protected macOS, Linux, and Windows compositions and the new aggregate workflow remain undispatched. This macOS
host cannot produce signed/installed Linux or Windows evidence. No current protected Python distribution, aggregate,
release eligibility, installed production, release, updater publication, or Microsoft Store evidence was produced.

Release eligibility binds the accepted ordinary and protected records at one source revision, but it does not yet
cryptographically tie each inner Python comparison to the ordinary candidate's outer distribution summary. That
association currently depends on selecting the exact source-bound protected distribution runs.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

The tests first failed for the missing aggregate binder, workflow, and release-eligibility contract. The focused
candidate, protected-package, aggregate, workflow, and eligibility suites now pass 21 tests. They cover missing or
mixed platforms/revisions/candidates, canonical inspection and containment recomputation, shared runtime identity,
platform layouts, retained signed native identities, exact prior workflow-run gates, closed ordinary release gates,
and path-free output.

`npm run format:check`, `npm run check`, `npm test`, and `npm run build` pass. The full frontend suite reports 349 tests
passed and 3 skipped across 69 passing and 1 skipped files. Dependency inventory, notices, release assets, Actionlint,
JavaScript syntax, and diff checks pass.

Application Cargo formatting and `cargo check` pass. The serial application suite passes 501 library tests with 36
ignored plus the updater-evidence binary test; doc tests pass. No browser or native-app UI review is required for these
path-free evidence and workflow contracts.

## Next bounded action

Add a credential-free per-platform envelope that binds each accepted Python comparison to the normalized outer
distribution evidence produced by the same protected workflow run. Reuse the existing release-candidate distribution
normalizers, reject mixed platform/revision or added/path-bearing fields, and carry the canonical outer-distribution
binding through the aggregate and release-eligibility records. Update each dedicated one-file artifact to contain the
envelope, with focused regression tests for substitution and digest drift.

Do not change distribution execution or dispatch protected workflows. Do not sign, release, publish, or perform
Microsoft Store work. Preserve the unrelated untracked assets and public key. Do not merge the draft PR without
separate authorization.
