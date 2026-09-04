# Bottie handover

Last verified: 2026-09-04

## Start here

PR #142 is merged into `main` at `7411668`. The next bounded slice is on
`codex/windows-python-appcontainer-transport`. No provider mapping, answer/context presentation, durable Python audit,
product bundle resolution, signing, release, publication, or Microsoft Store action was taken.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`

## Completed slice

- `WindowsAppContainerPythonRunner` now implements the existing provider-neutral runner interface around injected
  native controller, helper, and runtime paths. It selects only the fixed `execute` mode and
  `com.bottie.python-runner` profile; source and purpose remain in bounded JSON on private stdin.
- Linux, macOS, and Windows now share a dedicated bounded private-process transport. It clears the ambient environment,
  caps stdout and stderr, applies the existing 45-second outer deadline, accepts only the exact closed helper result,
  and maps launch, exit, timeout, pipe, size, and decode failures to fixed path-free errors.
- Shared generation cancellation kills and reaps an already-started Windows controller. Controller death closes the
  existing one-process, kill-on-close Job Object, terminating the contained runner; dropped orchestration retains
  `kill_on_drop`.
- Focused tests cover the fixed profile and native-only argument list, exact stdin/result contract, and cancellation
  after confirmed controller startup. No WebView or provider surface changed.

## Current limits

No provider adapter advertises or maps `run_python`, so normal product generation still cannot reach the execution
boundary. The Tauri application does not yet resolve or inject packaged native clients/controllers, provision the
fixed Windows AppContainer profile, or locate a helper/runtime. The earlier signed XPC and AppContainer proofs remain
transient development evidence rather than shipping-package evidence.

There is no execution-result presentation or Python-specific durable audit flow. The default and protected Tauri
configs remain unchanged. No installed-package containment, protected signing, notarization, release-candidate
binding, publication, or Microsoft Store work is authorized. Store certification and publication remain deferred
until fresh release-owner notice.

## Validation

Local review passed source formatting, Svelte diagnostics, production build, 278 frontend/script tests (3 skipped),
476 Rust application-library tests (33 ignored) plus updater evidence, three focused Windows transport tests, eight
AppContainer contract tests, seven standalone runner tests, six Python-bundle tests, offline dependency and notice
gates, release-asset checks, and `git diff --check`. Clippy completed with the same 39 pre-existing warnings outside
this slice and no warning in a changed file.

No browser-layout or native-app claim is needed because the WebView and live Tauri wiring did not change. Windows
compilation, the unsigned MSI smoke, and the credential-free native AppContainer proof remain pending on the draft
PR's hosted workflows. No protected signing or Store workflow is authorized.

## Next bounded action

Resolve and inject the packaged platform-native client/controller, helper, and runtime into the existing runner
interface under the opt-in Python development bundle, including fixed Windows profile provisioning and cleanup. Do not
advertise or map `run_python`, add answer/context presentation or durable Python audit, alter default/protected package
configs, sign, release, publish, or perform Microsoft Store certification.

Preserve the unrelated untracked logo-kit, screenshot, and Linux signing-public-key files. Do not merge the draft PR
without separate authorization.
