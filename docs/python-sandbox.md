# Python sandbox feasibility slice

Status: the standalone runner, its inner denial tests, development-only macOS, Windows, and Linux containment proofs,
official-source runtime provenance with unsigned development-package inspection plus installed Windows MSI and Linux
DEB containment, the approval-required native
proposal/review contract, a process-local one-use approve/deny lifecycle, provider-neutral async wait/resume,
append-only durable audit, explicit oMLX, Ollama, OpenAI-compatible, and Anthropic-compatible mappings, and
selected-lineage execution-result presentation are implemented. The native waiter also publishes bounded approval
lifecycle events to the existing WebView review state. An approved exact mapped-provider call can now cross the
provider-neutral Rust execution boundary into the helper's bounded private-pipe protocol through Linux containment, a
macOS XPC client, or a Windows AppContainer controller. Only an explicitly marked development bundle advertises the
tool, and only on a discovered tool-capable mapped-provider route. Bottie does not ship a Python tool. Default and
protected packages remain unchanged.

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

| Resource                  | Limit or policy                                         |
| ------------------------- | ------------------------------------------------------- |
| Request JSON              | 256 KiB                                                 |
| Python source             | 32 KiB UTF-8                                            |
| Execution purpose         | 512 Unicode scalar values                               |
| Wall time                 | 30 seconds, including interpreter startup               |
| WebAssembly linear memory | 256 MiB                                                 |
| stdout                    | 32 KiB after UTF-8 replacement and JSON string escaping |
| stderr                    | 32 KiB after UTF-8 replacement and JSON string escaping |
| Filesystem                | Two explicit read-only mounts; no ambient host paths    |
| Environment               | Only `PYTHONHOME=/runtime`                              |
| Network                   | TCP, UDP, name lookup, and address use denied           |
| Subprocesses              | Not provided by WASI                                    |
| Random-data request       | At most 1 MiB per host call                             |

The opt-in runtime suite proves ordinary execution and checks host-file denial, network denial, subprocess denial,
read-only mounts, environment isolation, timeout interruption, output ceilings, and memory-growth denial. The normal
unit suite checks the request and stable-result contracts without requiring a Python runtime.

## Runtime input and supply chain

`python-runner/runtime-manifest.json` pins the official CPython 3.14.7 source and SBOM from python.org, WASI SDK 24,
Wasmtime 45.0.3 as the build runner, the fixed build command and environment, the required runtime layout, and every
input's exact byte count and SHA-256 digest. The previous Brett Cannon `cpython-wasi-build` archive remains pinned only
as a compatibility-test input for the existing containment suites; the bundling workflow cannot select it as build
provenance.

The credential-free pull-request workflow downloads and verifies the official inputs, builds at one fixed path,
stages the reviewed 539-file runtime, cleans and rebuilds at that same path, then requires the two staged trees and
path-free evidence documents to be byte-identical. It passes that exact artifact to macOS, Windows, and Linux jobs,
which build the locked native helper plus the platform XPC client/service or AppContainer controller, create an opt-in
unsigned Tauri package, extract it, and compare the packaged helper/runtime against the original evidence while
recording each required native transport's package-relative path, byte count, and digest. Same-path repeatability is a
bounded hosted proof, not a claim that independent hosts produce identical bytes.

The Linux job additionally installs that one inspected development DEB, reinspects the fixed installed helper and
runtime against the package-owned evidence marker, and requires the installed result to match the extracted result
byte for byte. It then runs the same Landlock/seccomp/rlimit and process-lifecycle verifier directly against the
installed resources. The uploaded containment result contains only the closed path-free Boolean evidence.

The Windows job similarly installs its one inspected development MSI into a fresh fixed application directory,
reinspects the controller, helper, and runtime, and requires its path-free evidence to equal the extracted package
evidence. It then copies only those installed bytes into the transient AppContainer-owned proof tree and runs the
existing token, access, private-pipe, cancellation, and controller-close checks without a helper or controller rebuild.

The runtime, helper, evidence, and platform-native transport are selected only by the three
`src-tauri/tauri.python-development.*.conf.json` overlays; Bottie's base and protected distribution configurations
remain unchanged. macOS places the helper/runtime inside the private XPC service and requires macOS 14 for this
development bundle. Windows and Linux retain the resource directory plus sidecar layout. The official CPython licence
is checked by digest, included in `THIRD-PARTY-NOTICES.txt`, and represented alongside the complete runner Cargo graph
in `dependency-inventory.json`. No application runtime download occurs.

On the current Apple-silicon macOS host, the locally built official runtime is 40,864,108 bytes with tree digest
`293a02f7cc9bf01945c53a0fa68429cd7d7570b94da5bdde8502c857a2c97b2b`; the optimized unsigned helper is 14,273,328
bytes. An unsigned `.app` was built with its target-suffixed native XPC client and nested service; extracted-package
inspection matched all 539 runtime files and the helper, and the packaged Bottie executable started with the native
resolver active. These are development-host measurements, not cross-platform, signed, notarized, containment,
installed-package, or release evidence.

Wasmtime and `wasmtime-wasi` are exactly pinned to 45.0.3 for this slice. That patch contains the fix for Wasmtime's
June 2026 read-only-directory bypass advisory. Re-audit the current supported Wasmtime release and RustSec/GitHub
advisories before any distributed build rather than treating this feasibility pin as permanent.

Sources:

- [CPython WASI platform notes](https://github.com/python/cpython/blob/main/Platforms/WASI/README.md)
- [CPython 3.14.7 source release](https://www.python.org/downloads/release/python-3147/)
- [WASI SDK 24](https://github.com/WebAssembly/wasi-sdk/releases/tag/wasi-sdk-24)
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

The transient app, service, helper, copied checksum-verified runtime, and fixture are deleted after the proof. The
unsigned development `.app` now places the XPC client beside Bottie and the service under `Contents/XPCServices`; the
native resolver requires the complete fixed layout before retaining the client-backed runner. This proves bundle
placement and byte identity only. Distribution packages must still prove the exact shipping nested code, containment
launch, hardened runtime, notarization, Gatekeeper acceptance, and release-candidate hashes. Deprecated custom
sandbox profiles are not a release strategy.

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

Before launch, the proof wrapper and product bundle preparation use the same deterministic in-process ZIP32 writer to
derive an uncompressed `python314.zip` from the copied standard-library tree, with no shell or archive subprocess. The
product archive is included before its Windows-specific runtime evidence is calculated. CPython/WASI searches that
archive before the directory tree; storing rather than deflating it is required because this pinned WASI build does
not provide `zlib`. The copied source tree remains present so the contained probe separately proves exact
standard-library file reads and directory listing.

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

The credential-free pull-request workflow now proves exact extracted-to-installed controller, helper, and runtime byte
identity for the unsigned development MSI. It runs the same zero-capability, low-integrity, privilege-stripped,
host-fixture-denial, private-pipe, cancellation, and controller-close proof against those installed resources. Both
shipping routes still need evidence that the final signed bytes retain the intended containment policy and terminate
with Bottie.

The provider-neutral Windows product runner now starts an injected controller with the fixed `execute` mode, a
controller-safe `com.bottie.python.runner.<process-id>` profile, and native-only helper/runtime paths. Source and purpose
stay in bounded JSON on private stdin. The shared 45-second outer deadline and cancellation kill and reap the
controller; closing it activates the controller's existing kill-on-close Job Object and terminates the retained
AppContainer runner. The Tauri application now requires the complete marked development-bundle layout, provisions its
process-scoped profile before retaining the injected runner, and cleans up only that owned profile when native state is
dropped. Overlapping app processes therefore cannot delete one another's AppContainer registration or local storage.
Windows compilation, packaging, and lifecycle execution for this product path remain hosted evidence rather than local
macOS evidence.

References: [AppContainer isolation](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation),
[launching an AppContainer](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer),
[Create Process In Sandbox](https://learn.microsoft.com/en-us/windows/win32/secauthz/createprocessinsandbox), and
[Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects).

### Linux

Implemented as a development proof in the unchanged standalone runner: the Linux-only launch mode first requires a
single-threaded process and arms `PR_SET_PDEATHSIG` with a parent-race check. It then creates a Landlock ruleset that
handles all filesystem rights available through ABI 3 while granting only read-file and read-directory access to the
exact configured runtime and per-request staged workspace. Generated source still enters over stdin and the child
receives only its private `TMPDIR`; no source, output, runtime path, or fixture path enters the final evidence.

The runner creates its existing deadline thread only after Landlock is active, so that thread inherits the filesystem
domain. It then applies fixed address-space, data, CPU, file-size, and open-descriptor `rlimit` ceilings before
installing one architecture-checked seccomp BPF filter across all threads with `TSYNC`. The filter denies IPv4/IPv6 and
Unix socket operations, process-form `clone`, `clone3`, namespace creation, and `execve`/`execveat` while also closing
the `io_uring` network bypass; it retains thread-form `clone` for the bounded deadline worker. The proof directly
observes exact runtime/workspace reads, host-fixture denial, network/process/exec denial, ordinary private-pipe
execution, explicit caller cancellation, and kernel-enforced kill on parent exit.

This is the built-in baseline for the development DEB path; it does not depend on Bubblewrap or Flatpak. Those
containers may add namespaces, an empty home, and private temporary storage later, but must not replace or weaken these
controls. The credential-free pull-request workflow now proves exact installed development-DEB helper/runtime identity,
containment launch, denials, cancellation, and parent-exit cleanup on Ubuntu. It does not prove protected signing,
shipping-runtime identity, release-candidate binding, publication, or another Linux distribution/kernel baseline.

References: [Landlock](https://docs.kernel.org/userspace-api/landlock.html) and
[seccomp BPF](https://docs.kernel.org/userspace-api/seccomp_filter.html). The kernel explicitly describes seccomp as
one sandbox-building tool rather than a complete sandbox by itself.

## Local verification

Run the runtime-free contract suite with:

```sh
npm run python:bundle:test
cargo fmt --manifest-path python-runner/Cargo.toml -- --check
cargo clippy --manifest-path python-runner/Cargo.toml --offline --all-targets -- -D warnings
cargo test --manifest-path python-runner/Cargo.toml --offline
cargo build --manifest-path python-runner/Cargo.toml --release --locked --offline
```

The provenance workflow is the authoritative official-source build and package-inspection recipe. Locally, after
building official CPython with the exact manifest inputs, stage and inspect it with:

```sh
node scripts/python-runtime-bundle.mjs --stage-built /path/to/Python-3.14.7 /new/runtime/path
node scripts/python-runtime-bundle.mjs --inspect-runtime /new/runtime/path
```

After downloading and independently verifying the compatibility archive in `runtime-manifest.json`, extract it
outside the repository and run the native boundary suite with:

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

After installing the opt-in development MSI, the fixed installed-resource variant runs without rebuilding the helper
or controller:

```powershell
$env:BOTTIE_PYTHON_INSTALLED_ROOT = "C:\absolute\installed\bottie"
npm run python:appcontainer:prove-installed
```

On Linux, the credential-free native proof uses that same independently verified runtime:

```sh
BOTTIE_PYTHON_WASI_RUNTIME=/absolute/path/to/extracted/python npm run python:linux:prove
```

The command builds the locked runner and returns only path-free boolean evidence for its Landlock, seccomp, rlimit,
private-environment, runtime/workspace-read, host-fixture-denial, private-pipe, cancellation, and parent-close checks.
After installing the opt-in development DEB, the fixed installed-resource variant runs without rebuilding the helper:

```sh
npm run python:linux:prove-installed
```

## Approval-required product contract

`run_python` is now reserved as a provider-independent native contract with exactly two required fields: `source` and
`purpose`. The native validator rejects unknown fields, blank or NUL-containing values, source over 32 KiB UTF-8, and
purpose over 512 Unicode scalar values. These limits match the unchanged standalone runner request boundary. The
oMLX, Ollama, and OpenAI-compatible product mappings submit this request only after its exact native approval is
durably recorded.

The existing native tool policy classifies `run_python` as `ApprovalRequired`. Missing approval fails closed before
argument validation, and an approval grant is consumed and bound to the exact call identity, tool name, source, and
purpose. The definition is appended only to the oMLX, Ollama, OpenAI-compatible, or Anthropic-compatible native tool
set when the selected model explicitly reports tool capability and startup resolved the complete marked development
runtime.

One Rust-owned process-local slot can now retain a validated proposal for an explicit decision. The WebView receives a
random opaque request token plus the complete bounded source and purpose, never the provider call identity. A closed
Tauri command accepts only that token and `approve` or `deny`; it rejects unknown tokens, competing proposals, and a
second decision. Future native orchestration can consume the resolved decision only for the unchanged complete call,
and consumption clears it. An approval produces the existing exact one-use grant; a denial never does.

Provider-neutral native orchestration can now publish that exact proposal and wait asynchronously for its terminal
decision. Approval resumes with the exact one-use grant; denial resumes as a non-execution outcome. The waiter also
observes the same cancellation signal already shared by provider and native-tool work. Cancellation before publication
creates no review, while cancellation during a wait clears the exact slot and makes the old opaque token stale. A drop
guard applies the same exact-call cleanup if the provider task is aborted before it observes cancellation. No branch in
this orchestration launches code.

When a proposal becomes pending, the native controller emits the same bounded path-free status returned by
`get_python_approval`; it does not add provider call identity or native paths. Cancellation and waiter abortion emit a
`null` lifecycle update after clearing the exact slot. The WebView installs this listener before its startup read and
uses an event sequence to prevent an older `null` read from overwriting a newly published request. It removes the
listener on page disposal. If the native event cannot be published, the proposal fails closed and is removed rather
than leaving orchestration waiting on an invisible decision.

The Tool activity surface recognizes only the exact bounded argument shape and shows the proposed purpose followed by
the complete inert source. It explicitly states that Bottie has not run the code and suppresses the redundant raw
approval-error envelope. A separate modal shows the native pending proposal, traps keyboard focus, and offers one
Approve once or Deny action. Approved and denied acknowledgements both remain explicit that no code ran. Malformed or
future-shaped records retain the generic inert JSON disclosure instead. The development-only
`?python=approval-review` browser fixture makes the pending, approved, and denied presentation reproducible without
native inference or execution.

After the waiter consumes an approval, the new provider-neutral execution boundary applies the existing policy grant
to the unchanged complete call and validates the arguments again. Only then does it translate `source` to the helper's
stdin-only `code` field. The provider call identity never enters that JSON, process arguments, or the child environment.
The helper response must match the exact closed status/stdout/stderr/duration contract, fit the 96 KiB transport cap,
and retain the helper's 32 KiB per-stream limits. Malformed, future-shaped, oversized, non-zero-exit, launch, and
transport failures become fixed path-free native errors.

The concrete Linux launcher selects the already-proven `--linux-contained` runner mode. The macOS product transport
launches a native client whose sole fixed argument is `execute`; that client connects to Bottie's private App-Sandboxed
XPC service and never launches the runner directly. The Windows product transport launches the proven AppContainer
controller with its process-scoped profile plus native-only helper/runtime paths; the controller creates the
restricted-token, zero-capability child inside its one-process Job Object. All three transports clear the inherited
environment, use
bounded private stdin/stdout/stderr pipes, apply a 45-second outer deadline, and kill and reap their owned process on
cancellation. Killing the macOS client invalidates its XPC connection, while killing the Windows controller closes its
Job Object; both actions terminate the retained helper. Denial and cancellation while review is pending never touch a
transport; cancellation after approval is shared with the running transport.

## Durable audit boundary

Schema 22 extends the existing native tool audit with one immutable optional `tool_approvals` row. The storage boundary
accepts only `approved` or `denied` for an existing approval-required call while its provider run is active. It rejects
duplicates, decisions after a result, decisions for safe tools, successful results without approval, and successful
results after denial. Older records reopen with no invented decision.

The provider-neutral orchestration seam checkpoints the exact invocation before review, checkpoints an
explicit decision before any approved helper launch, then checkpoints one bounded terminal payload. Executed payloads
contain only the existing closed status, bounded stdout/stderr, and helper duration; denial and cancellation have fixed
payloads, and approval/helper failures retain only a stable error code. Reconstructed audit data omits provider call
identity, approval request tokens, and native paths. Native duration excludes user decision time. If approval storage
fails, no helper starts; if execution is interrupted after approval, the durable decision remains without a fabricated
result.

The selected response's typed Tool activity now parses only the exact closed Python audit shapes. It distinguishes
proposed from approved source and purpose, labels bounded stdout and stderr independently, shows the helper's stable
outcome and duration, and identifies Bottie's contained Python runtime as execution provenance. Denial, cancellation,
and native failure codes map to fixed path-free explanations. Malformed, contradictory, oversized, or future-shaped
payloads fail closed to one fixed unavailable message instead of reflecting their fields. Portable exports retain the
existing structured audit representation.

## Deferred product integration

The mapped-provider integration deliberately does not:

- decide automatically that Python is appropriate for a user question;
- expose Python in a default or protected package without the complete native runtime marker;
- select the development bundle config for normal or protected distribution, sign or publish the runtime/helper; or
- claim shipping-package containment, installed production behavior, or release identity on macOS, Windows, or Linux.

The OpenAI-compatible mapping uses the same asynchronous provider-neutral executor as the local providers. It adds the
definition only for an explicitly tool-capable selected model with the complete marked development runtime, retains the
provider's exact Chat Completions call ID through invocation, approval, result, and follow-up request, and reuses the
existing loop budgets and cumulative usage accounting. The configured remote provider receives the tool definition and
its own proposed source/purpose, but the helper remains local and cannot start without Bottie's exact one-use approval.

The Anthropic-compatible mapping uses that same asynchronous executor and runtime gate. It preserves complete thinking
and redacted-thinking blocks, returns each bounded success or error as a Messages `tool_result`, and retains the exact
opaque `tool_use` identity through invocation, approval, durable audit, and the follow-up request. Denial and shared
cancellation remain terminal non-execution paths, while usage and the existing loop budgets span the whole exchange.

The next bounded slice can add a credential-free macOS packaged-development-app XPC smoke for the exact inspected
client, service, helper, and runtime. It should prove exact package-byte identity, the existing App Sandbox and
host-fixture denial contract, ordinary private-pipe execution, caller cancellation, and client-exit cleanup without
rebuilding or substituting nested code. It must not change default or protected package configs, claim shipping
containment, sign for distribution, notarize, release, publish, or perform Microsoft Store work. Those actions remain
separately authorized and deferred.
