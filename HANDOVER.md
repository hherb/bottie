# Bottie handover

Last verified: 2026-09-03

## Start here

PR #134 is merged into `main` at `3d65d2b`. The next bounded Linux containment proof is implemented on
`codex/linux-python-containment`. Bottie still does not register, launch from Tauri, download, product-bundle, or
publish a Python tool. No updater, protected signing, release, or Microsoft Store action was taken.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`
4. `distribution/update/README.md` before returning to release work

## Completed slice

The standalone runner now has one explicit Linux-only containment mode around its existing bounded stdin/stdout JSON
contract:

- it requires a single-threaded process, arms `PR_SET_PDEATHSIG` with a parent-race check, then applies fixed
  address-space, data, CPU, output-file, and descriptor `rlimit` ceilings after its inherited-domain deadline thread
  exists and before generated code can run;
- Landlock fails closed when unavailable and grants only read-file/read-directory access to the exact configured
  runtime and per-request workspace; its deadline thread is created after the domain is active and inherits it;
- an architecture-checked seccomp BPF filter is synchronized across every runner thread, denies socket operations,
  `io_uring`, process-form `clone`, `clone3`, namespace creation, and exec, while retaining thread-form `clone` for the
  deadline;
- source and purpose remain stdin-only, the child receives only its private `TMPDIR`, and the wrapper uses private
  stdin/stdout/stderr pipes without a shell;
- the proof directly verifies runtime/workspace reads, host-fixture denial, network/process/exec denial, ordinary
  execution, caller cancellation, and kernel kill on parent exit; and
- checked-in contract tests lock the runner arguments, containment primitives, exact path-free evidence schema, and
  credential-free Ubuntu workflow.

The Linux-specific Rust module compile-checks for both `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` in an
isolated dependency-only harness. The full runner's native execution belongs to the Ubuntu pull-request workflow.

## Current limits

This is a development-only standalone-runner proof, not Bottie product integration or package evidence. It uses an
already downloaded and independently checksum-verified unofficial CPython/WASI development runtime; Bottie neither
downloads it at runtime nor adds it to the repository. The proof does not change Tauri commands, provider schemas,
native tool policy, approval UI, durable audit, output presentation, production runtime provenance, licence inventory,
or shipping packages.

Bubblewrap or Flatpak may add a stronger container layer later but cannot replace the built-in DEB baseline. No
installed DEB/AppImage/RPM, cross-distribution behavior, exact shipping helper/runtime, signature, release-candidate
hash, or product cancellation integration is claimed.

The unrelated updater work remains pending. Protected macOS publication still lacks its existing Apple distribution
credentials, and protected Windows publication still lacks its Authenticode PFX and password. Microsoft Store
certification and publication remain deferred until fresh release-owner notice.

## Validation

Local review on macOS passed the focused JavaScript contract, the unchanged host runner suite, Rustfmt, runner Clippy,
and cross-target compilation of the Linux module. The final full repository and Linux-native pull-request results must
be recorded here before handoff.

The Linux workflow independently verifies the pinned runtime size and digest on Ubuntu 24.04, builds the locked runner,
and must return one path-free result with every containment and lifecycle field set to `true`. This remains hosted
development evidence, not native hardware, installer, shipping-package, signing, release, or Store evidence.

## Next bounded action

Establish reproducible CPython/WASI build provenance and feed the exact development helper/runtime into cross-platform
bundling, licence inventory, and package inspection. Keep this credential-free and development-only: do not register a
provider-visible Python tool, add Tauri launch integration, accept legal terms, consume protected signing credentials,
publish releases, or resume Microsoft Store certification/publication.

Preserve the unrelated untracked logo-kit, screenshot, and Linux signing-public-key files. Do not merge the draft PR
without separate authorization.
