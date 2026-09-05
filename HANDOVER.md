# Bottie handover

Last verified: 2026-09-05

## Start here

PR #149 merged into `main` at `d2762c4`. The Anthropic-compatible Python slice is on
`codex/anthropic-python-tool`; its draft PR number is pending. Microsoft Store certification and publication remain
deferred until fresh release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- An explicitly tool-capable Anthropic-compatible generation advertises `run_python` only when Bottie resolved the
  complete marked development runtime. Default and protected packages remain unchanged.
- Anthropic-compatible generation now uses the shared async provider-neutral executor, so an exact proposal can pause
  for one explicit approval without blocking the runtime thread. Approval reuses the contained platform runner;
  denial and shared cancellation remain terminal non-execution paths.
- The existing call, recursion, aggregate-output, deadline, and cumulative-usage budgets span the full exchange.
  Complete thinking/redacted-thinking blocks and the exact opaque Messages `tool_use` identity survive invocation,
  durable approval/result checkpoints, and provider result reuse.
- The WebView still receives only the opaque approval token, bounded source/purpose, and selected-lineage audit. No
  provider call identity, native path, helper bytes, or runtime bytes cross that boundary.

## Current limits

Python remains available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX,
Ollama, OpenAI-compatible, or Anthropic-compatible model. A configured remote provider receives the tool definition
and the source/purpose it proposes; execution remains local and requires exact one-use approval. There is no
installed-package containment claim, protected signing, notarization, release-candidate binding, publication, or
Microsoft Store action.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

The focused tests failed first because Anthropic sessions lacked a runtime gate and asynchronous Python tool-round
seam. The completed focused suite passed runtime-gated definition, approval, denial, cancellation, durable audit,
bounded result, and exact call-identity tests. All five ignored Anthropic two-request loopback fixtures were run
separately with local-socket access and passed memory, web, email, Python, thinking-block, result-correlation, final
answer, and cumulative-usage coverage after the async conversion.

`npm run format:check`, `npm run check`, `npm test`, and `npm run build` passed: Svelte reported zero errors/warnings,
and 286 frontend/script tests passed with 3 skipped. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
`cargo check --manifest-path src-tauri/Cargo.toml`, and `cargo test --manifest-path src-tauri/Cargo.toml` passed. The
application library reported 501 passed and 36 ignored, plus the updater evidence test and doc tests. Documentation
checks with `npx prettier --check HANDOVER.md docs/python-sandbox.md` and `git diff --check` passed; `ROADMAP.md`
retains its existing list indentation.

Strict all-target Clippy is not a clean repository baseline: `cargo clippy --manifest-path src-tauri/Cargo.toml
--all-targets -- -D warnings` still stops on existing lint debt, including pre-existing high-arity orchestration seams.

No marked development Python bundle or live tool-capable Anthropic-compatible service was available, so this is
contract, fixture, and durable-data evidence rather than a fresh real-model/helper execution, installed-package,
signing, release, publication, or Store claim.

## Next bounded action

Add a credential-free Linux installed-development-DEB containment smoke for the exact packaged helper and runtime.
Prove package byte identity, the existing Landlock/seccomp denial contract, ordinary private-pipe execution, caller
cancellation, and parent-exit cleanup after installation. Do not change default/protected package configs, claim
shipping containment, sign, release, publish, or perform Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
