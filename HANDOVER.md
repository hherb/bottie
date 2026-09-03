# Bottie handover

Last verified: 2026-09-03

## Start here

PR #138 merged into `main` at `31eff8b`. The next bounded provider-neutral Python approval wait/resume slice is on
`codex/python-approval-wait-resume`. No helper, provider mapping, signing, release, publication, or Microsoft Store
action was taken.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- One async Rust-owned boundary now publishes a validated exact `run_python` proposal, waits without blocking, and
  resumes only after the existing opaque-token decision lifecycle resolves.
- Approval yields the existing one-use grant bound to the unchanged provider call identity, tool name, source, and
  purpose. Denial is a terminal outcome with no grant, and neither path executes code.
- The same cancellation signal already shared by provider and native-tool work wakes the approval waiter. Cancellation
  before publication creates no review; cancellation while pending clears the slot and makes its token stale.
- A dropped or aborted waiter also clears only its exact retained call, so provider-task abortion cannot strand the
  one-proposal slot or remove a different proposal.

## Current limits

No provider adapter advertises or maps `run_python`, so no product generation invokes the new wait boundary yet. There
is no helper or containment launch, execution-result presentation, or Python-specific durable audit flow. Native state
is process-local and intentionally limited to one proposal. Tests prove the provider-neutral lifecycle contract only;
they do not prove provider mapping, helper containment, execution, installed-package behavior, or durable recovery.

The runtime and package evidence from PR #136 remains development-only. The default and protected Tauri configs remain
unchanged. No protected signing, notarization, release-candidate binding, publication, installed-package claim, or
Microsoft Store work is authorized. Store certification and publication remain deferred until fresh release-owner
notice.

## Validation

Local review passed source formatting, Svelte diagnostics, production build, 276 frontend/script tests (3 skipped),
461 Rust application-library tests plus updater evidence (33 ignored), six standalone Python-bundle tests, offline
dependency and notice gates, release-asset checks, and `git diff --check`. Focused Rust coverage proves approval resume
with an exact grant, terminal denial, cancellation before and during a wait, stale-token rejection, slot reuse, and
cleanup when the waiting task is aborted. No presentation changed, so the previously reviewed approval modal remains
unchanged and no new browser claim is made. This is compiled/tested provider-neutral contract evidence, not provider
mapping, helper containment, execution, installed-package, signing, release, publication, or Store evidence.

## Next bounded action

Add the first provider-neutral contained-helper launch behind one exact approved grant, preserving shared cancellation
and keeping denial as non-execution. Continue to exclude provider advertisement/mappings, answer/context presentation,
durable Python audit, protected signing, release, publication, and Microsoft Store certification.

Preserve the unrelated untracked logo-kit, screenshot, and Linux signing-public-key files. Do not merge the draft PR
without separate authorization.
