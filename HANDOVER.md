# Bottie handover

Last verified: 2026-09-03

## Start here

PR #133 is merged into `main` at `5b3f336`. The next bounded Windows AppContainer containment proof is implemented on
`codex/windows-python-appcontainer`. Bottie still does not register, launch, download, product-bundle, or publish a
Python tool. No updater, release, protected signing, or Microsoft Store action was taken.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`
4. `distribution/update/README.md` before returning to release work

## Completed slice

`windows-python-appcontainer/Proof.cpp`, `scripts/windows-python-appcontainer.mjs`, and the credential-free Windows
workflow build one transient proof around the unchanged `bottie-python-runner` stdin/stdout JSON contract:

- a fresh AppContainer profile's `AC` subtree owns only a copied proof host, runner, and already checksum-verified
  runtime; its existing `AC\proof` DACL gains one inheritable read/execute ACE for the exact transient AppContainer
  SID, keeping Python package traversal inside AppContainer-local storage without granting runtime writes;
- the wrapper's deterministic in-process ZIP32 writer derives the uncompressed `python314.zip` that CPython/WASI
  searches before its standard-library directory; the copied source tree remains available for exact contained file
  and directory-access probes, with no shell or archive subprocess;
- every contained process combines an empty capability set with a `CreateRestrictedToken`/
  `DISABLE_MAX_PRIVILEGE` primary token;
- private anonymous pipes inherit exactly stdin, stdout, and stderr, with source supplied only over stdin and no host
  environment inherited;
- each child enters a one-process Job Object at creation, before its initially suspended thread can run;
- the Job Object caps committed process memory at 768 MiB and user CPU time at 120 seconds, accommodating bounded
  Wasmtime cold startup before the runner's separate 256 MiB linear-memory limit and 30-second execution deadline;
- explicit cancellation terminates the Job Object, while controller exit closes its last handle and kills the runner;
- a contained probe verifies AppContainer state, that only Windows' non-removable traverse privilege may remain enabled,
  Low integrity, zero capability SIDs, profile runtime access, and denial of a host-owned fixture outside the profile;
- the controller materializes canonical `AC`/`AC\Temp` storage without replacing inherited security, while the child
  requires `TMP` and `GetTempPathW` to agree on a path inside `AC` and proves create/write/delete access there; and
- all path-bearing preparation output stays private to the wrapper; final evidence is path-free, and the temporary
  profile, copied bytes, and fixture are removed.

The checked-in contract tests cover canonical profile locations, locked runner/MSVC arguments, exact containment and
Job Object primitives, absence of network capabilities/shell launch, and the credential-free Windows workflow.

## Current limits

This is a development-only Windows proof, not Bottie product integration or package evidence. It copies an already
downloaded, independently checksum-verified unofficial CPython/WASI runtime into its transient profile; Bottie neither
downloads it at runtime nor adds the archive to the repository. The proof does not change Tauri commands, provider
schemas, native tool policy, approval UI, durable audit, output presentation, production runtime provenance, licence
inventory, or shipping packages.

The low-level AppContainer path remains inspectable and Windows 10-compatible. A future product slice should evaluate
`CreateProcessInSandbox` on supported hosts, but must retain equivalent token, handle, denial, cancellation, and limit
evidence. No installed MSI/MSIX helper, Authenticode signature, Store package, cross-platform product behavior, or
release-candidate hash is claimed.

The unrelated updater work remains pending. Protected macOS publication still lacks its existing Apple distribution
credentials, and protected Windows publication still lacks its Authenticode PFX and password. Microsoft Store
certification and publication remain deferred until fresh release-owner notice.

## Validation

The local review passed:

```text
npm run format:check
npm run check
npm test                                      # 256 passed, 3 skipped
npm run build
cargo fmt/check/test (src-tauri)              # 447 passed, 33 ignored
cargo fmt/clippy/test/release build (runner)  # 8 passed, 3 ignored
npm run dependencies:check
npm run notices:check
git diff --check
```

Windows-native pull-request workflow
[run 33678403622](https://github.com/hherb/bottie/actions/runs/33678403622) passed on implementation head `2205718`.
Windows Server 2025 compiled the controller with MSVC warnings as errors, built the locked runner, independently
verified the pinned development runtime, and returned this path-free result:

```json
{"appContainerDeniedHostFixture":true,"appContainerLowIntegrity":true,"appContainerNoCapabilities":true,
 "cancellation":true,"jobCloseKilledRunner":true,"privatePipeExecution":true,"resourceLimits":true,
 "privilegesStripped":true,"status":"ok"}
```

This is hosted development evidence around copied inputs, not native hardware, installer, shipping-package, signing,
release, or Store evidence. The full frontend, Tauri, and standalone-runner validation results for the final reviewed
head belong in the draft PR.

## Next bounded action

Build a Linux-native containment proof around the unchanged runner contract. Prove Landlock access only to the exact
runtime/workspace, seccomp denial of network/process creation/exec, private-pipe execution, caller cancellation,
kill-on-parent-close, resource limits, and denial of a host-owned fixture. Bubblewrap or Flatpak may add an optional
stronger layer but cannot replace a built-in DEB baseline.

Do not register a provider-visible Python tool, download a runtime in the app, change Bottie's Tauri product path,
consume protected distribution credentials, or alter updater/release/Store publication. Preserve the unrelated
untracked logo-kit, screenshot, and Linux signing-public-key files. Do not merge the draft PR or resume Microsoft Store
work without separate authorization.
