# Bottie handover

Last verified: 2026-09-03

## Start here

PR #137 merged into `main` at `e2984b9`. The next bounded pending Python approval lifecycle and review slice is on
`codex/python-pending-approval-lifecycle`. No helper, provider, signing, release, publication, or Microsoft Store
action was taken.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- One Rust-owned process-local slot retains only a validated `run_python` call. A competing proposal cannot replace it.
- The WebView sees a random opaque request token plus complete bounded purpose/source, never provider call identity.
  Its closed command accepts exactly one `approve` or `deny`; unknown tokens and changed decisions fail closed.
- Future native orchestration can consume the decision once only for the unchanged call identity, tool name, source,
  and purpose. Approval yields the existing exact grant; denial never yields a grant; neither path executes code.
- The focus-trapped approval modal shows purpose before complete inert source and retains explicit not-run wording for
  pending, approved, and denied states. `?python=approval-review` exercises the interaction without native inference.

## Current limits

No provider call creates an approval request yet, and there is no provider wait/resume mapping, helper or containment
launch, cancellation integration, execution-result presentation, or Python-specific durable audit flow. Native state
is process-local and intentionally limited to one proposal. The browser fixture proves presentation and decisions only;
it does not prove native IPC, provider orchestration, containment, or execution.

The runtime and package evidence from PR #136 remains development-only. The default and protected Tauri configs remain
unchanged. No protected signing, notarization, release-candidate binding, publication, installed-package claim, or
Microsoft Store work is authorized. Store certification and publication remain deferred until fresh release-owner
notice.

## Validation

Local review passed source and focused Markdown formatting, Svelte diagnostics, production build, 276 frontend/script
tests (3 skipped), 456 Rust application-library tests plus updater evidence (33 ignored), six standalone Python-bundle
tests, offline dependency and notice gates, release-asset checks, and `git diff --check`. Browser presentation passed at
1280 × 720 and 480 × 800: the modal stays above responsive navigation, purpose precedes complete source, neither width
overflows, Approve once and Deny reach distinct not-run acknowledgements, Shift+Tab wraps, and Escape cannot discard a
pending request. This is browser-fixture and compiled/tested native-contract evidence, not live native IPC, provider
orchestration, containment, or execution evidence.

## Next bounded action

Add provider-neutral wait/resume orchestration for one exact pending decision, with cancellation and denial as terminal
non-execution paths. Continue to exclude provider advertisement/mappings, helper or containment launch, execution
results, durable Python audit, protected signing, release, publication, and Microsoft Store certification.

Preserve the unrelated untracked logo-kit, screenshot, and Linux signing-public-key files. Do not merge the draft PR
without separate authorization.
