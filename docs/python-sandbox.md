# Python sandbox feasibility slice

Status: the standalone runner, its inner denial tests, and a development-only macOS XPC containment proof are
implemented. Bottie does not register, product-bundle, or ship a Python tool yet.

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

The macOS proof adds an outer native boundary without changing this portable contract:

```text
transient proof host.app
        |
        | private XPC connection
        v
App-Sandboxed service.xpc (no file/network entitlements)
        |
        | private stdin/stdout/stderr pipes
        v
inherited-sandbox bottie-python-runner -> CPython/WASI
```

The runner currently enforces:

| Resource | Limit or policy |
| --- | --- |
| Request JSON | 256 KiB |
| Python source | 32 KiB UTF-8 |
| Execution purpose | 512 Unicode scalar values |
| Wall time | 30 seconds, including interpreter startup |
| WebAssembly linear memory | 256 MiB |
| stdout | 32 KiB after UTF-8 replacement and JSON string escaping |
| stderr | 32 KiB after UTF-8 replacement and JSON string escaping |
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

Implemented as a development proof: a private, separately signed App-Sandboxed XPC service has no network,
user-selected-file, Downloads, home-directory, app-group, or temporary-exception entitlement. It starts the exact Rust
runner with an empty host environment and private pipes; the nested runner is separately signed with only App Sandbox
and sandbox inheritance. The host retains no source in process arguments. Cancellation owns the identified child, XPC
connection invalidation kills every retained child, and a direct service-process read of a host-owned fixture outside
the container is denied.

The transient app, service, helper, copied checksum-verified runtime, and fixture are deleted after the proof. This is
not Bottie's Tauri product bundle. Distribution packages must still prove the exact shipping nested code, hardened
runtime, notarization, Gatekeeper acceptance, runtime inventory/licensing, and release-candidate hashes. Deprecated
custom sandbox profiles are not a release strategy.

References: [Apple XPC documentation](https://developer.apple.com/documentation/xpc) and
[Apple's XPC service guidance](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingXPCServices.html).

### Windows

Implemented as a development proof: a transient AppContainer profile's `AC` subtree owns only a copied proof host, the
unchanged Rust runner, and the already checksum-verified runtime, keeping Python package traversal inside
AppContainer-local storage. The existing `AC\proof` DACL gains one inheritable read/execute ACE for the exact transient
AppContainer SID; it does not grant runtime writes or replace the inherited DACL. The controller supplies an empty
capability list, combines the AppContainer launch attribute with a `DISABLE_MAX_PRIVILEGE` primary token, and inherits
only the three anonymous-pipe protocol handles. The child inherits no host environment; the controller supplies only
transient profile locations, and Windows maps them into the AppContainer. Source is supplied on stdin and never enters
a command line or shell.

Before launch, the wrapper's deterministic in-process ZIP32 writer derives an uncompressed `python314.zip` from the
copied standard-library tree, with no shell or archive subprocess. CPython/WASI searches that archive before the
directory tree; storing rather than deflating it is required because this pinned WASI build does not provide `zlib`.
The copied source tree remains present so the contained probe separately proves exact standard-library file reads and
directory listing.

Every child is assigned at process creation to a Job Object limited to one process, 768 MiB committed memory, 120
seconds of user CPU time, and kill-on-last-handle-close. That outer allowance includes bounded Wasmtime cold startup;
the unchanged runner retains its separate 256 MiB linear-memory limit and 30-second execution deadline. The native proof
checks that the child token is AppContainer, has zero capability SIDs, and has no enabled privilege except Windows'
non-removable directory-traverse privilege. The controller materializes the profile's canonical `AC` and `AC\Temp`
directories without replacing their inherited security. Inside the child, the proof requires `TMP` and `GetTempPathW`
to resolve to the same directory within that `AC` subtree, materializes that resolved directory there, then creates,
writes, and deletes a file at the resolved location. It uses the same launch path to deny a host-owned fixture outside
the profile, executes the unchanged runner contract, cancels a running request through the Job Object, and observes
that controller exit kills the retained runner. The transient profile, copied runtime, executables, and fixture are
deleted afterward.

On Windows versions that support it, evaluate the newer `CreateProcessInSandbox` API before maintaining the complete
low-level AppContainer launch sequence in the product. This proof retains the Windows 10-compatible low-level path so
its exact token, handle, and Job Object policy stays inspectable. MSIX package identity alone does not make Bottie's
full-trust desktop process or a future helper an AppContainer.

This is not Bottie's Tauri product binary or an MSI/MSIX package. Both shipping routes still need evidence that the
exact installed helper/runtime are inventoried and signed, launch under the intended containment policy, retain their
denials after installation, and terminate with Bottie.

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

On macOS, use the same verified extracted runtime for the transient signed XPC proof:

```sh
BOTTIE_PYTHON_WASI_RUNTIME=/absolute/path/to/extracted/python npm run python:xpc:prove
```

The command requires one usable Apple Development identity, signs runner -> service -> app inside out, verifies every
signature independently, exercises execution/cancellation/client-exit cleanup/host-file denial, and performs no
notarization or publication.

On Windows with MSVC available, use the same runtime for the transient AppContainer proof:

```powershell
$env:BOTTIE_PYTHON_WASI_RUNTIME = "C:\absolute\path\to\extracted\python"
npm run python:appcontainer:prove
```

The credential-free pull-request workflow independently verifies the pinned development-runtime size and digest,
compiles the controller with warnings as errors, builds the locked runner, and exercises private-pipe execution,
zero-capability/Low-integrity/privilege-stripped token state, profile-contained writable temporary storage,
host-fixture denial, cancellation, and kill-on-controller-close.

## Deferred product integration

This slice deliberately does not:

- register a model-visible tool or change any provider schema;
- decide automatically that Python is appropriate for a user question;
- add the approval UI required by `ToolExecutionPolicy::ApprovalRequired`;
- launch the helper from Bottie's Tauri process or connect product cancellation and durable audit;
- download, bundle, inventory, sign, or publish the CPython runtime or helper; or
- claim Linux containment or shipping-package macOS/Windows containment.

The next bounded slice is a Linux Landlock/seccomp/rlimits containment proof around this exact helper contract. It must
pass Linux-native tests for explicit runtime/workspace access, private-pipe execution, cancellation, parent-close
cleanup, denied networking/process creation/exec, and denial of a host fixture before product integration begins.
