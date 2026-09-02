# Bottie handover

Last verified: 2026-09-02

## Start here

The standalone CPython/WASI feasibility slice is present only in the working tree. Bottie still does not expose,
launch, bundle, or publish a Python tool. PR #130 remains merged into `main` at `c412f8a`; draft PR #131 remains open
from `codex/updater-publication` at implementation commit `43c4431`. No updater workflow or release action was taken in
this slice.

Read, in order:

1. `HANDOVER.md`
2. `docs/python-sandbox.md`
3. the Milestone 5 sandboxed-Python section of `ROADMAP.md`
4. `distribution/update/README.md` before returning to release work

## Completed slice

A separate `python-runner/Cargo.toml` binary now:

- accepts a deny-unknown-fields JSON request over stdin with 32 KiB source, 512-character purpose, and 256 KiB wire
  ceilings;
- stages source without shell interpolation or process arguments;
- executes CPython/WASI under Wasmtime 45.0.3's Pulley interpreter target;
- exposes only read-only `/runtime` and `/work` mounts, only `PYTHONHOME`, no guest stdin, no TCP/UDP/DNS, and no
  subprocess facility;
- caps wall time at 30 seconds, WebAssembly linear memory at 256 MiB, each output stream at 32 KiB after JSON escaping,
  and host random requests at 1 MiB;
- returns stable `ok`, `python_error`, `timed_out`, `output_limit`, `resource_limit`, `invalid_request`, or
  `internal_error` outcomes without host paths or Wasmtime backtraces; and
- keeps its 206-package locked dependency graph isolated from the existing Tauri manifest.

`python-runner/runtime-manifest.json` pins the development-only CPython 3.14.7/WASI SDK 24 archive by immutable URL,
size, SHA-256, and required layout. The archive is unofficial, remains outside the repository, and is neither fetched
at application runtime nor included in any package.

## Current limits

The helper process is an inner capability sandbox and crash boundary, not yet a complete product sandbox. A
hypothetical Wasmtime escape would still inherit the user's OS identity. Do not register the model-visible tool until
the separately signed OS boundary passes native denial tests:

- macOS: App-Sandboxed XPC service with no file or network entitlements;
- Windows: AppContainer, restricted token, and kill-on-close Job Object; and
- Linux: Landlock, seccomp, and rlimits, with Bubblewrap/Flatpak only as an optional stronger layer.

Provider schemas, approval UI, `ApprovedToolCall`, dispatcher launch, cancellation, durable audit, output presentation,
runtime provenance, licence inventory, packaging, signing, and release gates are all deferred. The Rust helper was
tested on macOS only; no signed app bundle or Windows/Linux native behavior was claimed.

The unrelated updater work remains pending. Protected macOS publication still lacks the existing Apple credentials,
and protected Windows publication still lacks its Authenticode PFX and password. Microsoft Store certification and
publication remain deferred until fresh release-owner notice.

## Validation

Passed for the standalone runner:

```text
cargo fmt --manifest-path python-runner/Cargo.toml -- --check
cargo clippy --manifest-path python-runner/Cargo.toml --offline --all-targets -- -D warnings
cargo test --manifest-path python-runner/Cargo.toml --offline
```

The opt-in runtime suite passed all three tests against the independently downloaded checksum-matching CPython/WASI
archive. It proved ordinary execution; host-file, network, subprocess, environment, and write denial; 30-second
interruption; output ceilings; memory-growth denial; and removal of internal backtraces from results:

```text
BOTTIE_PYTHON_WASI_RUNTIME=/private/tmp/bottie-python-wasi-spike/python \
  cargo test --manifest-path python-runner/Cargo.toml --offline --test runtime -- --ignored --nocapture

test result: ok. 3 passed; 0 failed; finished in 123.47s
```

The runtime-free suite includes seven library tests plus an integration-contract check. It covers encoded-output
ceilings after UTF-8 replacement and JSON escaping, and an explicit ignored-suite run without
`BOTTIE_PYTHON_WASI_RUNTIME` now fails immediately instead of certifying skipped work.

The optimized unsigned arm64 macOS helper built in 56.74 seconds and is 14,263,344 bytes. A cold statistics-script
probe returned the correct result in 3.50 seconds end to end, with 394 ms reported inside the execution window. This
is a local feasibility measurement, not signed-package or cross-platform performance evidence.

Malformed, unknown-field, and missing-runtime CLI probes also returned only the fixed `invalid_request` or
`internal_error` JSON contract. No frontend, Tauri, package, or workflow code changed, so their suites were not rerun.

## Next bounded action

Build a macOS App-Sandboxed XPC containment proof around this exact runner contract. In a development-signed app
bundle, prove bounded private-pipe execution, cancellation, kill-on-parent-exit, nested-code signing, and denial of a
fixture outside the service container. Do not register a provider-visible Python tool, add runtime downloads, or alter
release publication in that slice.

Preserve the unrelated untracked logo-kit, screenshot, and Linux signing-public-key files. Do not commit, push, open a
PR, publish an updater release, or resume Store work without explicit authorization.
