# Bottie handover

Last verified: 2026-09-02

## Start here

PR #132 is merged into `main` at `92b175e`. The next bounded macOS XPC containment proof is implemented on
`codex/macos-python-xpc-containment`. Bottie still does not register, launch, download, product-bundle, or publish a
Python tool. No updater, release, notarization, or Store action was taken.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`
4. `distribution/update/README.md` before returning to release work

## Completed slice

`macos-python-xpc/` and `scripts/macos-python-xpc.mjs` now build one transient development proof around the existing
`bottie-python-runner` stdin/stdout JSON contract:

- an otherwise inert host app connects only to its private `com.bottie.python-runner` XPC service;
- the separately signed service has exactly `com.apple.security.app-sandbox`, with no network, user-file, Downloads,
  home-directory, app-group, or temporary-exception entitlement;
- the nested Rust runner is separately signed with exactly App Sandbox plus sandbox inheritance, receives an empty
  host environment, and reads the configured runtime only from the service bundle;
- the service sends source to the runner over private stdin and retains bounded stdout while draining and discarding
  stderr; source never enters a shell or process argument;
- caller cancellation terminates the one identified child and escalates to a bounded `SIGKILL` only if necessary;
- XPC connection invalidation immediately kills every child retained by that connection;
- a direct service-process read of a host-owned fixture outside the container is denied; and
- signing is applied runner -> XPC service -> host app, with independent strict verification and one final deep nested
  verification. `--deep` is never used as a signing escape hatch.

The checked-in Node contract tests cover canonical bundle locations, fixed Swift compilation, exact inside-out signing
arguments, private-service metadata, and the least-privilege entitlement files. The native proof always deletes its
temporary app, copied runtime, module cache, and denial fixture.

## Current limits

This is a development-only Apple-silicon macOS proof, not Bottie product integration or package evidence. The proof
copies an already downloaded, independently checksum-verified CPython/WASI runtime into its transient service bundle;
it neither downloads at runtime nor adds the unofficial archive to the repository. It does not change Tauri commands,
provider schemas, native tool policy, approval UI, durable audit, output presentation, production runtime provenance,
licence inventory, or shipping packages.

The XPC transport's `running`, `completed`, `cancelled`, and `failed` states are proof-only lifecycle evidence. They do
not alter the runner's stable `ok`, `python_error`, `timed_out`, `output_limit`, `resource_limit`, `invalid_request`, or
`internal_error` result contract. No signed distribution app, hardened-package inspection, notarization, Gatekeeper,
Windows AppContainer, Linux Landlock/seccomp, or cross-platform runtime behavior is claimed.

The unrelated updater work remains pending. Protected macOS publication still lacks its existing Apple distribution
credentials, and protected Windows publication still lacks its Authenticode PFX and password. Microsoft Store
certification and publication remain deferred until fresh release-owner notice.

## Validation

The focused contract and source checks passed:

```text
npx vitest run scripts/macos-python-xpc.test.mjs --pool=forks --maxWorkers=1
xcrun swift-format lint --strict macos-python-xpc/Shared.swift macos-python-xpc/Service.swift \
  macos-python-xpc/Host.swift
xcrun swiftc ... macos-python-xpc/Shared.swift macos-python-xpc/Service.swift
xcrun swiftc ... macos-python-xpc/Shared.swift macos-python-xpc/Host.swift
```

The native proof used the same independently verified runtime as PR #132 and the existing Apple Development identity:

```text
BOTTIE_PYTHON_WASI_RUNTIME=/private/tmp/bottie-python-wasi-spike/python npm run python:xpc:prove
{"appSandboxDeniedHostFixture":true,"cancellation":true,"clientExitKilledRunner":true,
 "nestedSignaturesVerified":true,"privatePipeExecution":true,"status":"ok"}
```

This is local development-signing evidence only. The full frontend, Tauri, and standalone-runner validation results for
the final reviewed head are recorded in the draft PR.

## Next bounded action

Build a Windows-native AppContainer containment proof around the unchanged runner contract. On a Windows host, prove
private-pipe execution, caller cancellation, kill-on-parent-close through a Job Object, no-network AppContainer state,
and denial of a host-owned fixture outside the granted container. Add a restricted token and bounded process/memory/CPU
limits without registering a provider-visible Python tool, downloading a runtime in the app, changing Bottie's Tauri
product path, consuming protected distribution credentials, or altering release/Store publication.

Preserve the unrelated untracked logo-kit, screenshot, and Linux signing-public-key files. Do not merge the draft PR,
publish an updater release, resume Store work, or perform any release signing without separate authorization.
