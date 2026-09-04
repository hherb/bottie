# Bottie handover

Last verified: 2026-09-05

## Start here

PR #144 merged into `main` at `1c7c20a` with every final hosted check passing. Draft PR #145 is open from
`codex/python-durable-audit`. Microsoft Store certification and publication remain deferred until fresh release-owner
notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- Schema 22 adds one append-only `tool_approvals` row for an exact approval-required invocation. Only `approved` or
  `denied` is accepted, duplicate/late/safe-tool decisions fail closed, and a denied call cannot acquire a successful
  result.
- The dormant provider-neutral Python audit seam appends the invocation, waits for the existing native decision,
  appends that decision before any approved runner launch, and then appends one bounded executed, denied, cancelled,
  or fixed-error payload. A failed approval checkpoint prevents execution; an approved call interrupted before its
  result therefore retains an honest decision with no invented terminal record.
- The existing approval/execution function was split at the decision boundary without changing its callers. Native
  work duration now excludes time spent waiting for the user. Helper statuses and bounded stdout/stderr retain their
  closed path-free contract; provider call identity, request tokens, and native paths are not reconstructed.
- Stored audit metadata and version-5 portable JSON now include the optional decision and decision time. Tool activity
  and Markdown export label an available decision as `Approved once` or `Denied`; no Python answer/context
  presentation was added.
- Focused tests cover durable-before-execute ordering, reopen, denial without launch, cancellation without a false
  decision, fixed helper failure, one-decision/result consistency, older-store upgrade, and path-free reconstruction.

## Current limits

No provider adapter advertises or maps `run_python`, and normal generation does not call the new audit seam. There is
no Python execution-result answer/context presentation. Default and protected package configs remain unchanged. No
installed-package containment claim, protected signing, notarization, release-candidate binding, publication, or
Microsoft Store action was taken or authorized.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

`npm run format:check`, `npm run check`, `npm test`, and `npm run build` passed: Svelte reported zero errors/warnings,
and 281 frontend/script tests passed with 3 skipped. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
`cargo check --manifest-path src-tauri/Cargo.toml`, and `cargo test --manifest-path src-tauri/Cargo.toml` passed with
485 application-library tests passing and 33 ignored, plus the updater evidence test. Clippy completed with the same
39 pre-existing warnings outside changed files and no warning in this slice. `git diff --check` passed.

The development-only browser fixture was reviewed at the default desktop viewport with the Python approval modal and
expanded audit card. A development-signed native launch upgraded the existing store from schema 21 to 22; a read-only
immutable SQLite inspection returned `user_version = 22`, `quick_check = ok`, the exact `tool_approvals` table
contract, and zero existing approval rows. This launch is migration evidence only, not distribution signing, package,
release, publication, or Store evidence.

## Next bounded action

Advertise and map `run_python` only on the explicitly tool-capable oMLX route, using the audited async orchestration
seam and testing approve, deny, cancellation, bounded provider reuse, and durable reopen. Do not map another provider,
add answer/context presentation, alter default/protected package configs, claim installed-package containment, sign,
release, publish, or perform Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
