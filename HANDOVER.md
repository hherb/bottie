# Bottie handover

Last verified: 2026-09-05

## Start here

PR #145 merged into `main` at `fff92af` with every final hosted check passing. This branch continues only the next
bounded Python slice. Microsoft Store certification and publication remain deferred until fresh release-owner notice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- An available opt-in contained Python runtime adds `run_python` only to the explicitly tool-capable oMLX Chat
  Completions schema. Default packages expose no runner and therefore do not advertise Python. Ollama, OpenAI-compatible,
  and Anthropic-compatible mappings remain unchanged.
- The oMLX loop preserves provider call IDs and now routes Python through the existing async approval, exact one-use
  grant, platform-native runner, and append-only audit seam. Approved bounded results and fixed denial/helper errors can
  be returned to oMLX for the next round; shared cancellation stops before provider reuse.
- The shared tool-loop state machine gained an async executor path without changing its call-count, recursion,
  aggregate-output, overall-deadline, cancellation, or terminal-state policy. Non-Python memory, Web, Email, and clock
  calls still run on the blocking worker boundary.
- App state retains the approval controller and contained runner behind native `Arc` ownership. Only the runner trait
  crosses into generation orchestration. The WebView still receives only the existing opaque approval token plus exact
  bounded source/purpose; bundle paths, provider call identity, helper bytes, and native errors remain Rust-only.
- Focused tests cover oMLX-only definition gating, exact provider correlation, approve, deny without launch,
  cancellation without provider reuse, bounded loop accounting, and durable reopen.

## Current limits

Python is available only in an explicitly marked development bundle and only to a discovered tool-capable oMLX model.
There is no execution-result answer/context presentation beyond the existing approval modal and generic durable Tool
activity. Other providers do not advertise or map `run_python`. Default and protected package configs remain unchanged.
No installed-package containment claim, protected signing, notarization, release-candidate binding, publication, or
Microsoft Store action was taken or authorized.

The unrelated untracked logo-kit, screenshot, and Linux signing-public-key files remain untouched.

## Validation

`npm run format:check`, `npm run check`, `npm test`, and `npm run build` passed: Svelte reported zero errors/warnings,
and 281 frontend/script tests passed with 3 skipped. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`,
`cargo check --manifest-path src-tauri/Cargo.toml`, and `cargo test --manifest-path src-tauri/Cargo.toml` passed with
489 application-library tests passing and 33 ignored, plus the updater evidence test. Normal Clippy completed; strict
warning denial remains blocked by the repository's existing warning set. Focused oMLX protocol, shared tool-loop, and
42 Python-related tests passed, including the three new generation mapping cases. `git diff --check` passed.

No marked development Python bundle or live tool-capable oMLX service was available for a native end-to-end run. The
new evidence is therefore contract, orchestration, cancellation, persistence, and provider-wire coverage, not a live
model-choice, platform containment, installed-package, signing, release, publication, or Store claim.

## Next bounded action

Add selected-lineage Python execution-result presentation that clearly labels approved source, bounded stdout/stderr,
stable errors, and execution provenance using only the existing durable path-free audit. Do not map another provider,
change default/protected package configs, claim installed-package containment, sign, release, publish, or perform
Microsoft Store certification.

Preserve the unrelated untracked assets and public key. Do not merge the draft PR without separate authorization.
