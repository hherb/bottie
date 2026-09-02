# Python sandbox feasibility slice

Status: the standalone runner and its denial tests are implemented, but Bottie does not register or ship a Python
tool yet.

## Chosen core

Bottie can run generated Python through a separate Rust helper that embeds CPython/WASI in Wasmtime. The helper uses
Wasmtime's Pulley interpreter target so the guest does not require native WebAssembly JIT execution.

```text
future native tool dispatcher
        |
        | bounded JSON over private pipes
        v
bottie-python-runner process
        |
        | explicit WASI capabilities only
        v
CPython/WASI -> read-only /runtime + read-only /work/main.py
```

The helper accepts source and a user-visible purpose through stdin. It never places source in a shell command or
process argument. It returns one bounded JSON result and replaces internal Wasmtime errors with stable statuses.
Paths, traps, and WebAssembly backtraces do not cross that boundary.

The runner currently enforces:

| Resource | Limit or policy |
| --- | --- |
| Request JSON | 256 KiB |
| Python source | 32 KiB UTF-8 |
| Execution purpose | 512 Unicode scalar values |
| Wall time | 30 seconds, including interpreter startup |
| WebAssembly linear memory | 256 MiB |
| stdout | 32 KiB |
| stderr | 32 KiB |
| Filesystem | Two explicit read-only mounts; no ambient host paths |
| Environment | Only `PYTHONHOME=/runtime` |
| Network | TCP, UDP, name lookup, and address use denied |
| Subprocesses | Not provided by WASI |
| Random-data request | At most 1 MiB per host call |

The opt-in runtime suite proves ordinary execution and checks host-file denial, network denial, subprocess denial,
read-only mounts, environment isolation, timeout interruption, output ceilings, and memory-growth denial. The normal
unit suite checks the request and stable-result contracts without requiring a Python runtime.

## Runtime input and supply chain

CPython documents WASI as a Tier 2 platform, but CPython does not publish the binary used by this spike. The pinned
development archive comes from Brett Cannon's `cpython-wasi-build` project and must therefore be treated as an
unofficial supply-chain input. `python-runner/runtime-manifest.json` records its immutable URL, size, SHA-256 digest,
and required layout. The archive is not committed, downloaded at application runtime, or included in a Bottie
package.

The verified development input is CPython 3.14.7 built with WASI SDK 24. Its archive is 14,291,017 bytes, with SHA-256
`2e064d3fb8172471d39d741348efa722349c40b96301f69968dff714999c584b`; the extracted runtime is approximately 40.8
MB. Production packaging should use a reproducible Bottie-owned build or separately reviewed provenance and
attestation, then feed the exact artifact into dependency inventory, licence notices, release-candidate hashes, and
all three platform package inspections.

On the current Apple-silicon macOS host, the optimized unsigned helper is 14,263,344 bytes. A cold release-mode
statistics script completed end to end in 3.50 seconds; the runner reported 394 ms inside the execution window after
module compilation. Together with the extracted runtime, the uncompressed feasibility footprint is approximately 55
MB before package compression or symbol stripping. These are development-host measurements, not cross-platform
budgets or signed-package evidence.

Wasmtime and `wasmtime-wasi` are exactly pinned to 45.0.3 for this slice. That patch contains the fix for Wasmtime's
June 2026 read-only-directory bypass advisory. Re-audit the current supported Wasmtime release and RustSec/GitHub
advisories before any distributed build rather than treating this feasibility pin as permanent.

Sources:

- [CPython WASI platform notes](https://github.com/python/cpython/blob/main/Platforms/WASI/README.md)
- [Pinned unofficial CPython/WASI build](https://github.com/brettcannon/cpython-wasi-build/releases/tag/v3.14.7)
- [Wasmtime 45.0.3](https://github.com/bytecodealliance/wasmtime/releases/tag/v45.0.3)
- [Wasmtime read-only filesystem advisory](https://github.com/bytecodealliance/wasmtime/security/advisories/GHSA-4ch3-9j33-3pmj)
- [Wasmtime Pulley design and usage](https://github.com/bytecodealliance/wasmtime/blob/main/docs/examples-pulley.md)

## Platform containment options

WASI capability denial is the portable inner boundary. The separate helper limits memory-corruption impact on the
main Bottie process, but process separation alone does not stop a hypothetical Wasmtime escape from using the user's
ambient operating-system permissions. Do not expose the runner to model-generated code until a second, OS-owned
boundary is implemented and tested.

### macOS

Recommended: place the runner behind a separately signed, App-Sandboxed XPC service with no network, user-selected
file, Downloads, or home-directory entitlements. The XPC protocol should carry the same bounded request/result
contract, and the service should create and destroy the native staging directory. The enclosing application can keep
its existing capabilities while the code-execution service receives fewer privileges. Development and distribution
packages must prove nested-code signing, hardened runtime, notarization, Gatekeeper acceptance, and denial against a
fixture placed outside the service container. Deprecated custom sandbox profiles are not a release strategy.

References: [Apple XPC documentation](https://developer.apple.com/documentation/xpc) and
[Apple's XPC service guidance](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingXPCServices.html).

### Windows

Recommended: ship the runner as a signed package executable and start it in an AppContainer with no network
capability. Use private anonymous pipes for the bounded protocol and a Job Object for one-process, memory, CPU-time,
and kill-on-parent-close limits. A restricted token can further remove inherited privileges. Both direct MSI and MSIX
packaging need tests that the exact installed helper is signed, launches inside the intended AppContainer, cannot read
a fixture outside its granted package/temp locations, and is terminated with Bottie.

On Windows versions that support it, evaluate the newer `CreateProcessInSandbox` API before maintaining the complete
low-level AppContainer launch sequence. MSIX package identity alone does not make a full-trust desktop process an
AppContainer.

References: [AppContainer isolation](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation),
[launching an AppContainer](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer),
[Create Process In Sandbox](https://learn.microsoft.com/en-us/windows/win32/secauthz/createprocessinsandbox), and
[Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects).

### Linux

Recommended baseline: have the helper install a Landlock ruleset permitting only the exact read-only runtime and
staged script, then apply seccomp denial for network, process creation, and exec plus `rlimit` ceilings. A Flatpak build
or an available Bubblewrap launcher can add namespaces, an empty home, and a private temporary filesystem, but a DEB
must not silently assume Bubblewrap is installed. Package tests should exercise both the built-in baseline and any
stronger container path on supported distributions.

References: [Landlock](https://docs.kernel.org/userspace-api/landlock.html) and
[seccomp BPF](https://docs.kernel.org/userspace-api/seccomp_filter.html). The kernel explicitly describes seccomp as
one sandbox-building tool rather than a complete sandbox by itself.

## Local verification

Run the runtime-free contract suite with:

```sh
cargo fmt --manifest-path python-runner/Cargo.toml -- --check
cargo clippy --manifest-path python-runner/Cargo.toml --offline --all-targets -- -D warnings
cargo test --manifest-path python-runner/Cargo.toml --offline
cargo build --manifest-path python-runner/Cargo.toml --release --locked --offline
```

After downloading and independently verifying the archive in `runtime-manifest.json`, extract it outside the
repository and run the native boundary suite with:

```sh
BOTTIE_PYTHON_WASI_RUNTIME=/absolute/path/to/extracted/python \
  cargo test --manifest-path python-runner/Cargo.toml --offline --test runtime -- --ignored
```

## Deferred product integration

This slice deliberately does not:

- register a model-visible tool or change any provider schema;
- decide automatically that Python is appropriate for a user question;
- add the approval UI required by `ToolExecutionPolicy::ApprovalRequired`;
- launch the helper from Bottie's Tauri process or connect cancellation and durable audit;
- download, bundle, inventory, sign, or publish the CPython runtime or helper; or
- claim native containment on macOS, Windows, or Linux.

The next bounded slice is a macOS App-Sandboxed XPC containment proof around this exact helper contract. It must pass
signed development-package tests for private-pipe execution, cancellation, parent-exit cleanup, and denial of a host
fixture before any model-visible Python tool work begins.
