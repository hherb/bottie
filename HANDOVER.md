# Bottie handover

Last verified: 2026-09-03

## Start here

PR #136 merged into `main` at `7eff2fb`. The next bounded approval-required Python contract and review slice is on
`codex/python-tool-approval-contract`. No helper, provider, signing, release, publication, or Microsoft Store action was
taken.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- `run_python` is a closed, provider-independent proposal with exact `source` and `purpose` fields, a 32 KiB UTF-8
  source ceiling, a 512-Unicode-scalar purpose ceiling, and fail-closed blank, NUL, malformed, and extra-field checks.
- Native policy classifies the reserved tool as approval-required. A one-use grant must match the complete call
  identity, name, source, and purpose; changed calls remain blocked.
- The existing Tool activity surface presents exact bounded proposals as inert purpose-first source review and states
  explicitly that Bottie has not run the code. Raw approval-error JSON is suppressed for that exact review shape.
- `?python=approval-review` supplies a development-only disconnected-browser fixture. Current provider-definition
  selection is unchanged and never advertises `run_python`.

## Current limits

There is no approve/deny interaction, callable approval command, pending-call orchestration, provider mapping, helper
launch, containment launch, cancellation integration, or Python-specific durable audit flow. The review is reachable
only from retained tool-shaped data and the explicit development fixture; it does not prove execution.

The runtime and package evidence from PR #136 remains development-only. The default and protected Tauri configs remain
unchanged. No protected signing, notarization, release-candidate binding, publication, installed-package claim, or
Microsoft Store work is authorized. Store certification and publication remain deferred until fresh release-owner
notice.

## Validation

Local review passed formatting, Svelte diagnostics, production build, 272 frontend/script tests (3 skipped), 450 Rust
application-library tests plus updater evidence (33 ignored), offline dependency and notice gates, release-asset checks,
and `git diff --check`. Browser presentation passed at the default desktop viewport and 480 × 800: purpose precedes
complete inert source, the blocked/not-run state remains visible, and document/review widths do not overflow. This is
browser-fixture evidence, not native approval or execution evidence.

## Next bounded action

Add the provider-neutral pending Python approval lifecycle with one explicit approve/deny decision bound to the exact
call identity, source, and purpose. Continue to exclude provider advertisement/mappings, helper or containment launch,
execution results, protected signing, release, publication, and Microsoft Store certification.

Preserve the unrelated untracked logo-kit, screenshot, and Linux signing-public-key files. Do not merge the draft PR
without separate authorization.
