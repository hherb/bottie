# Bottie handover

Last verified: 2026-09-04

## Start here

PR #143 is merged into `main` at `40c8dfc`. Draft PR #144 is open from
`codex/python-bundle-runtime-injection`. No provider advertisement or mapping, answer/context presentation, durable
Python audit, default/protected package change, signing, release, publication, or Microsoft Store action was taken.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- Tauri now resolves packaged Python resources only when the opt-in evidence marker exists. A marked bundle fails
  closed unless every fixed native client/controller, helper, runtime, and platform service path has the expected
  regular-file or directory type; native paths stay in Rust.
- The resolved Linux, macOS, or Windows runner is retained behind the existing provider-neutral trait in native state.
  Windows additionally provisions a controller-safe profile moniker scoped to Bottie's native process and requests
  cleanup only for that owned profile when the state is dropped, so overlapping app instances cannot remove one
  another's AppContainer registration or local storage.
- Three platform-specific development overlays keep Python absent from default/protected packages. The credential-free
  provenance workflow now stages the macOS client plus nested XPC service, the Windows controller plus deterministic
  uncompressed standard-library archive, or the Linux contained helper, then checks the extracted package layout and
  helper/runtime evidence.
- Review widened the prior Windows controller-start test deadline from one to five seconds after it reproduced as a
  full-suite-only scheduling failure and immediately passed in isolation. A later review caught the shared-profile
  teardown race; focused lifecycle and transport tests now cover distinct process monikers and exact reuse through
  preparation, execution, and cleanup.

## Current limits

No provider adapter advertises or maps `run_python`, so normal generation still cannot reach the injected runner.
There is no Python-specific durable audit or execution-result presentation. The unsigned macOS development package
proves resource placement, byte identity, resolver startup, and native compilation only; it does not prove XPC App
Sandbox execution from that package. Windows controller compilation, profile lifecycle, MSI placement, and Linux DEB
placement remain pending on the draft PR's credential-free hosted workflow.

The default and protected Tauri configs remain unchanged. No installed-package containment, protected signing,
notarization, release-candidate binding, publication, or Microsoft Store work is authorized. Store certification and
publication remain deferred until fresh release-owner notice.

## Validation

Local review passed source formatting, Svelte diagnostics, production build, 281 frontend/script tests (3 skipped),
480 Rust application-library tests (33 ignored) plus updater evidence, four focused resource-resolution tests, seven
standalone runner tests (three runtime tests ignored), seven Python-bundle tests, six XPC contract tests, nine
AppContainer contract tests, offline dependency and notice gates, release-asset checks, and `git diff --check`. Clippy
completed with the same 39 pre-existing warnings outside this slice and no warning in a changed file.

On Apple silicon, the new staging command compiled the target-suffixed XPC client and service. An unsigned opt-in
`.app` built successfully; extracted-package inspection matched the 539-file, 40,864,108-byte runtime with tree digest
`293a02f7cc9bf01945c53a0fa68429cd7d7570b94da5bdde8502c857a2c97b2b` and the 14,273,328-byte helper with SHA-256
`a686b768840c45231e505f5fc611698d51a6b05d7181950b65ff15fef1200fb9`. The packaged Bottie executable started with
the resolver active and was then stopped manually. No signing, notarization, protected workflow, or Store action ran.
Hosted draft-PR checks were queued or in progress at handoff; re-query the final head before treating them as evidence.

## Next bounded action

Record Python approval decisions and bounded execution outcomes in the existing append-only native tool audit through
a provider-neutral orchestration test harness. Do not advertise or map `run_python`, add answer/context presentation,
alter default/protected package configs, make installed-package containment claims, sign, release, publish, or perform
Microsoft Store certification.

Preserve the unrelated untracked logo-kit, screenshot, and Linux signing-public-key files. Do not merge the draft PR
without separate authorization.
