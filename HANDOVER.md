# Bottie handover

Last verified: 2026-09-05

## Start here

PR #146 merged into `main` at `3fc4abd` with every final hosted check passing. Draft PR #147 is open from
`codex/python-result-presentation`. Microsoft Store certification and publication remain deferred until fresh
release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- Selected assistant responses now specialize their existing durable Tool activity for `run_python`. Approved source
  and purpose are labeled separately from unapproved proposals; the UI does not derive Python activity outside the
  selected conversation lineage.
- The pure presentation boundary accepts only the exact native executed, denied, cancelled, or failed audit shapes and
  requires their approval, generic outcome, and error marker to agree. Unexpected fields, unknown statuses, oversized
  streams, invalid durations, and contradictory audit metadata fail closed to one fixed unavailable message rather
  than reflecting payload data.
- Executed results show the stable helper outcome, independently labeled 32 KiB-bounded stdout and stderr, helper
  duration, and Bottie's contained Python runtime as execution provenance. Empty streams receive explicit placeholders;
  Svelte renders source and output as inert escaped text.
- Denial, cancellation, approval failure, request mismatch, helper failure, and invalid helper result use fixed,
  path-free explanations. Generic JSON disclosure remains unchanged for non-Python tools.
- Dark/light styling retains the existing calm approval/result distinction. The metadata collapses to one column below
  560 px without widening the result card.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX
model. Ollama, OpenAI-compatible, and Anthropic-compatible routes do not advertise or map `run_python`. Default and
protected package configs remain unchanged. There is no installed-package containment claim, protected signing,
notarization, release-candidate binding, publication, or Microsoft Store action.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

`npm run format:check`, `npm run check`, `npm test`, and `npm run build` passed: Svelte reported zero errors/warnings,
and 286 frontend/script tests passed with 3 skipped. Focused Tool activity and pure parser coverage passed 14 tests,
including exact success, denial, cancellation, malformed/future-shaped data, stream ceilings, escaping, and path-free
fallback behavior. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
`cargo check --manifest-path src-tauri/Cargo.toml`, and `cargo test --manifest-path src-tauri/Cargo.toml` passed with
489 application-library tests passing and 33 ignored, plus the updater evidence test. `git diff --check` passed.

The browser preview was reviewed at the default desktop viewport and at 540 px using a temporary exact executed-result
fixture that was removed after inspection. The visible labels, inert source/output, empty-stderr state, provenance, and
narrow metadata stacking were present; the 394 px result card had a 392 px scroll width, so it introduced no horizontal
overflow. No marked development Python bundle or live tool-capable oMLX service was available, so this is durable-data
and presentation evidence, not a fresh helper execution, installed-package, signing, release, publication, or Store
claim.

## Next bounded action

Add `run_python` only to the explicitly tool-capable Ollama generation loop, reusing the exact approval, contained
runner, durable audit, cancellation, bounded-result, and provider-correlation boundaries already proven for oMLX. Do
not map OpenAI-compatible or Anthropic-compatible providers, change default/protected package configs, claim installed
containment, sign, release, publish, or perform Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
