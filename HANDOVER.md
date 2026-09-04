# Bottie handover

Last verified: 2026-09-04

## Start here

PR #141 is merged into `main` at `cae77db`. The next bounded slice is on
`codex/macos-python-xpc-transport`. No provider mapping, answer/context presentation, durable Python audit, product
bundle resolution, signing, release, publication, or Microsoft Store action was taken.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- `MacosXpcPythonRunner` now implements the existing provider-neutral runner interface around one injected native XPC
  client path. Its sole process argument is the fixed `execute` mode; source and purpose remain in bounded JSON on
  private stdin, and the ambient environment is cleared.
- Linux and macOS share one bounded private-process transport. It caps stdout and stderr, applies the existing
  45-second outer deadline, accepts only the exact closed helper result, and maps launch, exit, timeout, pipe, size,
  and decode failures to fixed path-free errors.
- Shared generation cancellation kills and reaps an already-started macOS client. Dropped orchestration retains
  `kill_on_drop`; client death invalidates the XPC connection, activating the service's existing per-connection cleanup
  that kills retained runners.
- Focused macOS tests cover the fixed argument list, exact stdin/result contract, and cancellation after confirmed
  client startup. No WebView or provider surface changed.

## Current limits

No provider adapter advertises or maps `run_python`, so normal product generation still cannot reach the execution
boundary. The Tauri application does not yet resolve or inject a packaged macOS XPC client, service, helper, or runtime.
The earlier signed XPC proof remains transient development evidence rather than shipping-package evidence.

Windows still has no product AppContainer transport. There is no execution-result presentation or Python-specific
durable audit flow. The default and protected Tauri configs remain unchanged. No installed-package containment,
protected signing, notarization, release-candidate binding, publication, or Microsoft Store work is authorized. Store
certification and publication remain deferred until fresh release-owner notice.

## Validation

Local review passed source formatting, Svelte diagnostics, production build, 278 frontend/script tests (3 skipped),
473 Rust application-library tests plus updater evidence (33 ignored), three focused macOS transport tests, seven
standalone runner tests, six Python-bundle tests, offline dependency and notice gates, release-asset checks, and
`git diff --check`. The checksum-pinned runtime's capability-denial and deadline cases passed in the release profile
used for the helper; output/resource classification also passed. One combined debug-profile opt-in run exceeded the
guest deadline during the broader capability probe and poisoned the next test mutex, so it is not product evidence.
Strict Clippy remains an optional diagnostic and reports 39 pre-existing warnings outside this slice.

No browser-layout claim is needed because the WebView did not change. The credential-backed signed XPC proof was not
rerun because this slice did not alter the service and signing was not authorized.

## Next bounded action

Add the Windows AppContainer product transport behind the existing provider-neutral runner interface. Reuse the
restricted primary token, zero-capability AppContainer, private pipes, one-process Job Object, shared cancellation, and
controller-close cleanup. Continue to exclude provider advertisement/mappings, answer/context presentation, durable
Python audit, shipping bundle resolution, protected signing, release, publication, and Microsoft Store certification.

Preserve the unrelated untracked logo-kit, screenshot, and Linux signing-public-key files. Do not merge the draft PR
without separate authorization.
