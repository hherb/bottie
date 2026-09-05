# Bottie handover

Last verified: 2026-09-05

## Start here

PR #147 merged into `main` at `993bc3f`. The Ollama Python continuation is on
`codex/ollama-python-tool`. Microsoft Store certification and publication remain deferred until fresh release-owner
notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- An explicitly tool-capable Ollama generation can advertise `run_python` only when Bottie resolved the complete
  marked development runtime. The existing oMLX mapping remains unchanged; OpenAI-compatible and
  Anthropic-compatible routes still omit Python.
- Ollama now uses the same async provider-neutral executor as oMLX, so an exact proposal can suspend for one explicit
  approval without blocking the runtime thread. Approved calls reuse the contained platform runner; denial and shared
  cancellation remain terminal non-execution paths.
- The provider-neutral loop retains its existing call, recursion, aggregate-output, and deadline budgets. Because
  Ollama correlates tool messages by ordered tool name rather than a provider call ID, Bottie assigns each call a fresh
  native audit ID, preserves call/result order, and validates the name again when appending the provider exchange.
- Invocation, explicit decision, and bounded terminal result are durably checkpointed before provider reuse. The
  WebView continues to receive only the opaque approval token, bounded source/purpose, and selected-lineage audit; no
  provider identity or native path crosses the boundary.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX
or Ollama model. OpenAI-compatible and Anthropic-compatible routes do not advertise or map `run_python`. Default and
protected package configs remain unchanged. There is no installed-package containment claim, protected signing,
notarization, release-candidate binding, publication, or Microsoft Store action.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

The focused red tests failed first because Ollama lacked a runtime-gated Python definition and async tool-round seam.
The completed focused suite passed approval, denial, cancellation, durable audit, ordered result correlation, and
runtime-gated definition tests. The ignored two-request loopback fixture was run separately with local-socket access
and proved `run_python` definition, approval, runner-interface dispatch, provider result reuse, final answer, and
cumulative usage across the Ollama exchange.

`npm run format:check`, `npm run check`, `npm test`, and `npm run build` passed: Svelte reported zero errors/warnings,
and 286 frontend/script tests passed with 3 skipped. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
`cargo check --manifest-path src-tauri/Cargo.toml`, and `cargo test --manifest-path src-tauri/Cargo.toml` passed. The
application library reported 493 passed and 34 ignored, plus the updater evidence test and doc tests. Documentation
format and `git diff --check` passed.

No marked development Python bundle or live tool-capable Ollama service was available, so this is contract, fixture,
and durable-data evidence rather than a fresh real-model/helper execution, installed-package, signing, release,
publication, or Store claim.

## Next bounded action

Add `run_python` only to the explicitly tool-capable OpenAI-compatible generation loop, reusing the exact approval,
contained runner, durable audit, cancellation, bounded-result, and provider-call-identity boundaries already proven
for local providers. Do not map Anthropic-compatible providers, change default/protected package configs, claim
installed containment, sign, release, publish, or perform Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
