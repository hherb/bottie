# Bottie handover

Last verified: 2026-09-04

## Start here

PRs #139 and #140 are merged into `main` at `b27a769`. The next bounded slice is on
`codex/python-contained-helper-launch`. No provider mapping, answer/context presentation, durable Python audit,
signing, release, publication, or Microsoft Store action was taken.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- One provider-neutral Rust boundary now waits for the existing exact approval, consumes its one-use grant against the
  unchanged complete call, validates the Python arguments again, and only then invokes an injected runner.
- Denial and cancellation while review is pending are terminal non-execution outcomes. The same shared cancellation
  signal reaches an already-started runner, and a dropped concrete child future is configured to kill the process.
- The helper request is bounded JSON on private stdin only. Product `source` becomes helper `code`; source, purpose,
  and provider call identity never enter process arguments or the child environment.
- Helper stdout must match the exact closed status/stdout/stderr/duration contract. The launcher rejects malformed,
  future-shaped, oversized, non-zero-exit, launch, timeout, and private-pipe failures with fixed path-free errors.
- The only concrete launcher is Linux-only and selects the runner's built-in `--linux-contained`
  Landlock/seccomp/rlimit mode. macOS and Windows deliberately have no direct-process fallback.

## Current limits

No provider adapter advertises or maps `run_python`, so normal product generation cannot reach this execution boundary.
The Tauri application does not yet resolve or inject packaged helper/runtime paths. macOS still needs a product XPC
transport and Windows still needs a product AppContainer transport; the earlier proofs remain transient development
evidence. There is no execution-result presentation or Python-specific durable audit flow.

The runtime and package evidence from PR #136 remains development-only. The default and protected Tauri configs remain
unchanged. No installed-package containment, protected signing, notarization, release-candidate binding, publication,
or Microsoft Store work is authorized. Store certification and publication remain deferred until fresh release-owner
notice.

## Validation

Local review passed source formatting, Svelte diagnostics, production build, 278 frontend/script tests (3 skipped),
470 Rust application-library tests plus updater evidence (33 ignored), seven focused execution tests, six standalone
Python-bundle tests, offline dependency and notice gates, release-asset checks, and `git diff --check`. Strict Clippy
remains an optional diagnostic and reports pre-existing warnings outside this slice. No browser-layout claim is needed
because the WebView did not change. No live helper was launched on this macOS host because direct launch would violate
the required XPC boundary.

## Next bounded action

Add the first macOS product transport behind the existing provider-neutral runner interface. Reuse the separately
App-Sandboxed XPC design, private pipes, shared cancellation, and connection-invalidation cleanup. Continue to exclude
provider advertisement/mappings, answer/context presentation, durable Python audit, Windows AppContainer product
transport, protected signing, release, publication, and Microsoft Store certification.

Preserve the unrelated untracked logo-kit, screenshot, and Linux signing-public-key files. Do not merge the draft PR
without separate authorization.
