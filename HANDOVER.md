# Bottie handover

Last verified: 2026-09-05

## Start here

PR #148 merged into `main` at `ff23053`. Draft PR #149 is open from `codex/openai-python-tool`. Microsoft Store
certification and publication remain deferred until fresh release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- An explicitly tool-capable OpenAI-compatible generation can advertise `run_python` only when Bottie resolved the
  complete marked development runtime. Existing oMLX and Ollama behavior remains unchanged; Anthropic-compatible
  routes still omit Python.
- OpenAI-compatible generation now uses the shared async provider-neutral executor, so an exact proposal can suspend
  for one explicit approval without blocking the runtime thread. Approved calls reuse the contained platform runner;
  denial and shared cancellation remain terminal non-execution paths.
- The existing call, recursion, aggregate-output, and deadline budgets remain in force. Bottie preserves the exact
  Chat Completions tool-call ID through native invocation, durable approval/result checkpoints, and provider result
  reuse, while cumulative usage still spans every request in the exchange.
- The WebView continues to receive only the opaque approval token, bounded source/purpose, and selected-lineage audit;
  no provider identity or native path crosses the boundary.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX,
Ollama, or OpenAI-compatible model. An OpenAI-compatible endpoint receives the tool definition and the source/purpose
it proposes; execution remains local and requires exact one-use approval. Anthropic-compatible routes do not
advertise or map `run_python`. Default and protected package configs remain unchanged. There is no installed-package
containment claim, protected signing, notarization, release-candidate binding, publication, or Microsoft Store action.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

The focused red tests failed first because OpenAI-compatible sessions lacked a runtime gate and asynchronous Python
tool-round seam. The completed focused suite passed runtime-gated definition, approval, denial, cancellation, durable
audit, bounded result, and exact call-identity tests. The ignored two-request loopback fixture was run separately with
local-socket access and proved definition advertisement, approval, runner-interface dispatch, exact provider result
correlation, final answer, and cumulative usage across the OpenAI-compatible exchange.

`npm run format:check`, `npm run check`, `npm test`, and `npm run build` passed: Svelte reported zero errors/warnings,
and 286 frontend/script tests passed with 3 skipped. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
`cargo check --manifest-path src-tauri/Cargo.toml`, and `cargo test --manifest-path src-tauri/Cargo.toml` passed. The
application library reported 497 passed and 35 ignored, plus the updater evidence test and doc tests. Documentation
format and `git diff --check` passed.

No marked development Python bundle or live tool-capable OpenAI-compatible service was available, so this is contract,
fixture, and durable-data evidence rather than a fresh real-model/helper execution, installed-package, signing,
release, publication, or Store claim.

## Next bounded action

Add `run_python` only to the explicitly tool-capable Anthropic-compatible generation loop, reusing the exact approval,
contained runner, durable audit, cancellation, bounded-result, thinking-block, and `tool_use` identity boundaries
already proven for the other mapped providers. Do not change default/protected package configs, claim installed
containment, sign, release, publish, or perform Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
