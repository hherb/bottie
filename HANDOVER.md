# Bottie handover

Last verified: 2026-09-12

## Start here

PR #163 merged into `main` at `9e5502f`. The current branch is
`codex/protected-python-outer-envelope`.

Read `docs/python-sandbox.md`, then the Milestone 5 sandboxed-Python section of `ROADMAP.md`.

## Completed slice

- `python:protected:bind-envelope` revalidates one accepted protected-package comparison, normalizes the final outer
  distribution from that same protected run with the existing ordinary release-candidate normalizer, rejects incomplete
  or wrong-target evidence, and emits a closed path-free record with canonical comparison, outer-distribution, and
  relationship hashes.
- Each optional protected macOS, Linux, and Windows producer now writes that envelope after its existing comparison and
  exposes only the one-file envelope as the aggregate workflow input.
- `python:protected:bind-platforms` now revalidates the three fixed envelope records and carries each canonical
  outer-distribution binding through the aggregate.
- `python:protected:release-eligibility` now requires exact equality between those three protected-run distribution
  summaries and the corresponding normalized artifacts in the ready ordinary release candidate. Its platform records
  retain the outer-distribution and relationship hashes.
- Tests cover normalization, source/platform substitution, comparison and distribution digest drift, open/path-bearing
  envelopes, incomplete evidence, shared runtime constraints, workflow filenames, and ordinary-candidate substitution.
  Test fixtures were split out to preserve the practical 500-line source limit.

No default distribution path, provider behavior, protected trigger, credential flow, signing implementation, release
path, publication path, or Store path changed. Unrelated untracked logo-kit, screenshot, and Linux public-key files remain
untouched.

## Validation

The focused envelope, aggregate, release-eligibility, and ordinary release-candidate suite passes 17 tests. The full
frontend/script suite passes 352 tests with 3 skipped across 69 passing and 1 skipped files. `npm run format:check`,
`npm run check`, `npm run build`, dependency-inventory verification, JavaScript syntax checks, and diff checks pass.

Application Cargo formatting and `cargo check` pass. The serial application suite passes 501 library tests with 36
ignored plus the updater-evidence binary test; doc tests pass. Cargo reports only the existing future-incompatibility
notice for `block 0.1.6`.

No browser or native-app UI review is required for this path-free evidence/workflow contract. The optional protected
workflows remain undispatched, and this macOS host cannot produce signed/installed Linux or Windows evidence.

## Next boundary

The next meaningful step is to produce fresh same-revision protected macOS, Linux, and Windows evidence, then run the
read-only aggregate and eligibility review. Do not dispatch it without explicit release-owner authorization: it uses
protected environments, hosted-runner source egress, platform signing credentials, and macOS notarization. Confirm the
required credentials are configured before any authorized run.

Do not sign, release, publish, perform Microsoft Store work, or merge the draft PR without separate authorization.
