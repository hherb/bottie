<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="Bottie logo" width="112" height="112">
</p>

<h1 align="center">Bottie</h1>

<p align="center">
  <strong>A local-first desktop chatbot with durable memory and a native security boundary.</strong>
</p>

<p align="center">
  Bring your own local or cloud models. Keep secrets, files, tools, and conversation storage behind Rust.
</p>

> [!IMPORTANT]
> Bottie 0.9.0 is being prepared as a tester-facing beta. It is not yet distributed as a signed end-user release.

## Why Bottie?

Bottie treats local inference and storage as first-class. Cloud routes are explicit, and the Tauri WebView receives
only the typed information it needs to render the interface.

|                     | What Bottie provides                                                                 |
| ------------------- | ------------------------------------------------------------------------------------ |
| **Provider choice** | oMLX, Ollama, OpenAI-compatible, and Anthropic-compatible inference                  |
| **Durable work**    | Searchable conversations, branches, retries, ratings, exports, backups, and recovery |
| **Visible context** | Inspectable attachments, memories, web sources, email activity, and tool audit       |
| **Native trust**    | Rust-owned credentials, files, SQLite storage, provider traffic, and tool execution  |

## Highlights

### Talk to the models you choose

- Discover local oMLX and Ollama models and their advertised capabilities.
- Connect explicit OpenAI-compatible and Anthropic-compatible endpoints.
- Stream answers and separate reasoning with cancellation, bounded output, and retained usage metadata.
- Send normalized JPEG and PNG images only when the selected model advertises vision support.

### Keep conversations useful over time

- Reopen conversations after restart, including checkpointed partial output from interrupted runs.
- Edit or regenerate from earlier messages without overwriting the original branch.
- Search active and archived histories and reopen the exact branch containing a match.
- Export the visible lineage as readable Markdown or versioned JSON, with portable attachment bundles when needed.
- Create verified backups, recover from corruption, and use forward-only staged database migrations.

### Bring context under your control

- Attach text, Markdown, PDF, DOCX, JPEG, and PNG files through a native picker.
- Extract, normalize, index, and preview supported content without exposing native paths to the WebView.
- Search conversations and retained documents through an opt-in, local hybrid memory index.
- Inspect and remove context cards without erasing the durable tool audit that produced them.
- Exclude a conversation from memory, move it to Trash, or permanently forget it through native lifecycle controls.

### Use bounded native tools

- Let capable models use opt-in memory, web search, web fetch, and read-only Localmail tools.
- Search through Brave Search or Exa Search while keeping API keys in the operating-system credential vault.
- Fetch only validated public HTTP(S) pages through a proxy-free, redirect-limited native client.
- Search and open email, then read already extracted attachment text, through an explicitly pinned Localmail connection
  without exposing credentials, attachment hashes, raw bytes, or mail internals to the WebView.
- Review each tool call, stable outcome, duration, and retained result in the conversation audit.

## Keyboard commands

Open the local command palette with Command+K on macOS or Ctrl+K on Windows and Linux. It filters a small registry of
existing safe interface actions and keeps unavailable actions visible with their reason. Direct shortcuts are also
available for New conversation (Command/Ctrl+N), conversation search (Command/Ctrl+Shift+F), conversation navigation
(Command/Ctrl+Shift+B), the Context panel (Command/Ctrl+Shift+C), and Settings (Command/Ctrl+,). Escape closes the
palette and restores focus to the control that opened it.

## Appearance

Settings includes local System, Light, and Dark theme choices plus Comfortable and Compact density. Appearance changes
apply immediately without reconnecting a provider and stay on the current device. New and invalid preference state
falls back to Bottie's existing dark, comfortable presentation; System follows OS color changes only while selected.

## Local voice capture

The composer has an explicit **Record voice** action in the native desktop app. Bottie lists at most 64 currently
available inputs at startup without opening one or requesting permission; **Refresh** repeats that bounded discovery.
The WebView receives only sanitized display labels and process-local opaque tokens, including a distinct **System
default** choice; native device IDs, host APIs, paths, and hardware addresses remain in Rust. Rust derives a separate
stable opaque preference key for the most recent exact choice on this device. If it is unavailable during the next startup discovery, Bottie
returns to System default and remembers that fallback. At Record, System default resolves the operating system's
current default, while an exact choice that disappears later in the same session still fails with a path-free
unavailable state instead of silently switching inputs.

Bottie does not open the selected microphone or request operating-system permission until Record is chosen. Rust
then downmixes native PCM into one bounded, session-only in-memory capture for at most 60 seconds and 32 MiB. The
WebView receives only capture phase,
permission state, duration, input level, sample rate, channel count, retained byte size, and bounded speech/silence
timing—never audio samples, detector thresholds, or native input-device identity. A native energy detector analyzes
20 ms frames with separate onset and release confirmation so brief peaks and pauses do not flicker the state. The
composer calmly reports **Listening for speech** or **Speech detected**. A dedicated Rust worker recognizes bounded
speech-containing snapshots with the pinned multilingual Whisper tiny Q5 model, replaces partial results while capture
continues, and makes the stopped result final. The WebView receives at most 32 path-free transcript ranges and 4,000
UTF-8 bytes of transcript text with visible start/end timing; model paths, hashes, runtime details, and PCM stay native.
After Stop makes the transcript final, every range is shown as a numbered voice turn. Each turn has an explicit
correction field; Rust accepts only non-blank replacements of at most 512 UTF-8 bytes while retaining the turn timing
and 4,000-byte transcript ceiling. Corrected turns are visibly marked. Partial turns cannot be edited, and corrections
remain only in the same native session slot.

For a completed non-empty transcript, **Use transcript as text** explicitly copies the visible final turns into the
ordinary editable composer draft. Turns keep their displayed order and corrected text. An existing draft is preserved
with one blank-line append boundary; choosing the action again appends the transcript again. The combined draft must
fit a 32 KiB UTF-8 ceiling or Bottie leaves it unchanged and reports the limit. Copying focuses the composer but never
submits, persists, retains, discards, or selects audio delivery. The native capture and transcript remain available for
further correction, provider delivery, local retention, or Discard. The local draft remains editable when no provider
or model is ready, while Send stays unavailable until the existing provider and model requirements are satisfied.

The same session status shows four bounded monotonic intervals that Rust can observe directly: native receipt of
**Record** until the input stream reports a successful start, the same Record anchor until the first non-empty local
transcript is applied, native capture-stop handling until the final transcript is applied, and **Play response aloud**
handling until the local speech engine accepts the playback command. These are not first-sample, first-token, or
audible-output measurements. Missing work is shown as waiting, processing, unavailable, or no transcript rather than
zero. Starting a replacement action clears earlier measurements; Discard, accepted audio consumption, playback stop,
natural playback completion, and failures clear the relevant session timing. Timing is never persisted or sent to a
provider.

**Stop** retains the capture and final transcript only until they are discarded, replaced, explicitly consumed, or
Bottie exits;
**Discard** clears the retained capture, pending snapshot, and visible transcript immediately. If one native inference
pass is already executing, its bounded PCM copy is released when that pass returns and its stale result is ignored.
**Record again** deterministically replaces the stopped session capture on the same selected input: Rust first joins
an already-inactive capture owner, including its short final unwind, while continuing to reject overlapping starting
or recording owners.

The speech model is downloaded only after explicit capture first produces speech, from the immutable repository
revision and file identity in `runtime-assets.json`, into Bottie's app-owned cache. Native code verifies its exact
32,152,673-byte SHA-256 contract before loading it. Transcript text remains session-only and is not inserted into a
conversation or sent to a provider.

After capture stops, provider delivery and local retention remain two separate, off-by-default choices. **Send
recording** is available only when the selected oMLX or OpenAI-compatible model explicitly advertises audio input.
Rust snapshots the stopped mono PCM into one bounded PCM16 WAV, inserts it as a native-only provider-neutral audio
content block on the current user turn, and maps it to the OpenAI-shaped `input_audio` field. Audio bytes, encoded WAV,
hashes, and paths never cross WebView IPC. **Retain locally** can be selected independently, including with a text-only
model; it stores that WAV as an app-private `voice-recording.wav` message attachment under the existing backup,
export, branch, removal, forget, and attachment-garbage-collection policy. After the native request accepts delivery or
retention, the session capture and transcript are discarded. Without either choice, captured audio remains native-only
until Discard, replacement, or process exit.

Every completed assistant response also has an explicit **Play response aloud** action. Rust lazily enumerates the
device's local voices and exposes at most 128 bounded names, language tags, and process-local opaque selection tokens;
platform identifiers remain native. **Settings** owns the voice selector, while Rust stores a separate stable opaque
preference key for the most recent choice on this device. If that voice is unavailable after restart, Bottie chooses and remembers the default
available local voice. Playback accepts at most 32 KiB of visible text derived from the response's safe Markdown
tokens, so link destinations and formatting syntax are not spoken. The WebView receives only `idle`, `speaking`, or
fixed error state and never receives generated audio, engine details, output-device identity, or the retained
utterance. **Stop local playback** ends only Bottie's current utterance. While Bottie is speaking or generating, the
Record action becomes **Interrupt & record**. Choosing it
requests the existing provider cancellation, stops Bottie's local playback, and only then starts native capture. Rust
also serializes capture against provider-run registration, cancels every registered provider/tool run, and rejects a
generation that races with active capture. A failed local-playback stop keeps capture fail-closed. Playback remains
rejected while native capture is active and also stops when the user changes conversation or branch, starts a new
generation, or closes the page. Linux packages depend on Speech Dispatcher and recommend its eSpeak NG voices; macOS
and Windows use their installed system voices. The browser preview can render deterministic voice controls for layout
review but cannot capture, recognize, or play audio.

## Application updates

Settings provides one explicit **Check for updates** action. Bottie contacts only its fixed HTTPS GitHub release
manifest from native Rust, returns bounded version and release-note text, and downloads nothing until the user chooses
**Install update** for that exact reviewed version. Rust rechecks the candidate before installation, rejects equal or
older versions, verifies the downloaded artifact with Bottie's embedded production public key, and keeps manifest and
artifact URLs, signatures, bytes, filesystem paths, and private-key material out of WebView IPC. No update manifest or
signed updater artifact is published yet, so this source capability is not evidence of a working release channel.
Protected platform builds now independently verify each generated minisign signature against the exact final artifact
and committed public key, then retain only path-free hashes, byte size, target, format, and verified state. Current
protected evidence exists for Linux x64 only; macOS and direct-download Windows remain credential-gated. The manual
`Updater publication` workflow now composes those three protected builds, rechecks current `main`, creates a complete
draft, verifies every uploaded digest, and only then publishes the beta-labelled release as GitHub's full latest
release. That full-release state is deliberate because GitHub excludes prereleases from the fixed `/releases/latest/`
endpoint used by Bottie.

## The trust boundary

Bottie is local-first, not local-only. The interface makes the selected provider and enabled context routes visible
before a request leaves the application.

| The WebView receives              | The Rust core owns                                              |
| --------------------------------- | --------------------------------------------------------------- |
| UI-safe typed state               | Conversations, indexes, backups, and recovery                   |
| Path-free file metadata           | Paths, bytes, extraction, image normalization, and hashes       |
| Path-free voice/transcript state  | Microphone, PCM, speech engines, and local speech inference     |
| Credential status and diagnostics | Vault access, provider authentication, and sensitive logs       |
| Context cards and audit summaries | Tool policy, network clients, deadlines, and durable audit data |

Additional safeguards include:

- an explicit main-window Tauri allowlist limited to native event listen/unlisten plus a CSP limited to bundled UI,
  Tauri IPC, and opaque attachment previews;
- adversarial IPC contract tests for secret-free credential/settings/diagnostic state and path-free native file
  outcomes, with unknown fields rejected on secret-bearing command inputs;
- no general HTTP client exposed to the WebView;
- loopback-only validation for local inference providers;
- explicit cloud routing and first-run privacy disclosure;
- one macOS Touch ID prompt at app start, followed by process-memory caching of all configured credentials for the
  session;
- public-network and destination policy checks for web tools;
- certificate pinning, disabled redirects, and disabled ambient proxies for Localmail;
- fixed call, round, response-size, aggregate-output, and deadline ceilings for native tools.
- no updater plugin permissions or JavaScript updater binding in the WebView; only Bottie's narrow native
  check/install/cancel commands are registered.
- no WebView media-capture capability; microphone access and sample retention remain behind narrow Rust commands.
- local speech recognition downloads one hash-pinned model only after explicit capture; no audio reaches its source.
- captured audio reaches a provider only after the separate explicit send choice and an advertised audio capability;
  optional app-private WAV retention remains independently off by default.
- local playback uses narrow Rust commands and opaque voice tokens; no WebView speech or audio API is authorized.

The standalone CPython/WASI runner now has development-only macOS, Windows, and Linux containment proofs plus a pinned
official-source runtime and exact unsigned package inspection. An explicitly marked development bundle can advertise
the closed `run_python` contract to a discovered tool-capable oMLX or Ollama model. Its 32 KiB source and 512-character
purpose require exact one-use approval before the platform-contained helper can run. The selected response's Tool
activity labels approved source, bounded stdout/stderr, stable outcomes, and contained-runtime provenance from the
durable path-free native audit. Remote providers, default and protected package configs, and shipping claims remain
unchanged.
See [`docs/python-sandbox.md`](docs/python-sandbox.md).

## Provider support

| Provider             | Route                           | Vision | Audio input   | Native tool loop |
| -------------------- | ------------------------------- | ------ | ------------- | ---------------- |
| oMLX                 | Local loopback                  | Yes    | If advertised | Yes              |
| Ollama               | Local loopback                  | Yes    | No            | Yes              |
| OpenAI-compatible    | Explicit user-configured origin | Yes    | If advertised | Yes              |
| Anthropic-compatible | Explicit user-configured origin | Yes    | No            | Yes              |

Features are enabled from discovered or mapped model capabilities rather than model-name guesses. Memory, Web, and
Email start off for a new installation, then remember the user's last choices across app sessions. A remembered choice
is effective only while the selected model remains tool-capable; Email additionally requires a saved, trusted
Localmail connection and remains read-only.

## Get started

### Prerequisites

- A current Rust toolchain
- Node.js and npm
- The [platform prerequisites for Tauri 2](https://v2.tauri.app/start/prerequisites/)
- At least one supported inference provider

### Install and run

```sh
npm install
npm run tauri dev
```

On first use, Bottie asks you to confirm a working provider and model after explaining which data stays local and which
data follows the selected route.

On macOS, the package script development-signs each newly linked executable with an available Apple Development
identity before Cargo runs it. If more than one identity is usable, set `BOTTIE_APPLE_SIGNING_IDENTITY` to the exact
certificate label or SHA-1 fingerprint you intend to use. This affects development signing only.

### Browser-only preview

```sh
npm run dev
```

The browser preview is useful for layout work. Native inference and other Tauri-owned functionality are deliberately
unavailable, so sending is disabled.

For reproducible long-history rendering and scrolling checks during development, open
`http://127.0.0.1:1420/?performance=long-history` after starting the preview. This explicit development-only fixture
renders 2,000 active/Archived navigation rows and a 600-turn selected lineage; it is absent from production behavior.

## Development

Run the standard validation suite before submitting a change:

```sh
npm run format:check
npm run dependencies:check
npm run icons:check
npm run check
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

The unbundled Python runner has its own manifest so its large sandbox dependency graph does not affect ordinary
Bottie builds. Its contract tests do not require the external runtime:

```sh
cargo fmt --manifest-path python-runner/Cargo.toml -- --check
cargo clippy --manifest-path python-runner/Cargo.toml --offline --all-targets -- -D warnings
cargo test --manifest-path python-runner/Cargo.toml --offline
```

On macOS, the separate native containment proof requires an already downloaded and independently checksum-verified
runtime plus one usable Apple Development identity. It builds and signs a transient app/XPC/runner bundle, runs the
denial and lifecycle checks, and deletes the bundle without notarizing or publishing it:

```sh
BOTTIE_PYTHON_WASI_RUNTIME=/absolute/path/to/extracted/python npm run python:xpc:prove
```

On Windows, the equivalent credential-free development proof creates a transient zero-capability AppContainer profile,
uses a restricted token and limited Job Object, and removes the copied runtime/profile after the denial and lifecycle
checks:

```powershell
$env:BOTTIE_PYTHON_WASI_RUNTIME = "C:\absolute\path\to\extracted\python"
npm run python:appcontainer:prove
```

Run the opt-in large-history budgets separately so their deterministic 2,000-conversation/50,000-message fixture does
not add to the default test duration:

```sh
npm run performance:test
```

### macOS package verification

On a macOS host with dependencies installed from the checked-in lockfiles, build and inspect the arm64 application
bundle with:

```sh
npm run icons:check
npm run dependencies:check
npm run package:macos
npm run package:macos:sign-development
npm run package:macos:inspect
```

The build is app-only, non-interactive, skips distribution signing, and passes `--locked` to Cargo. The optional
development-signing step uses the same identity-selection policy as `npm run tauri dev`, adds no timestamp or
notarization, and prints no certificate identity. Inspection requires the generated Bottie ICNS at its application
bundle path and reports only bundle-relative paths and hashes, public plist metadata, architecture, signing class, and
packaged native runtime files.

Run the bounded launch/storage/provider-offline check separately:

```sh
npm run package:macos:smoke
```

The smoke command compiles the same locked code under the distinct `com.bottie.packaging-smoke` identity, refuses to
replace a pre-existing support directory, points both local providers at one rejecting loopback endpoint, and removes
only that test identity's Application Support data after termination. Fresh release executables can remain in macOS
execution-policy evaluation for up to the command's two-minute startup allowance. This workflow does not produce a
notarized or end-user-distributable release.

To exercise Bottie's separate distribution-validation contract, first make a Developer ID Application identity
available in the active keychains. Set `BOTTIE_APPLE_DISTRIBUTION_IDENTITY` to its exact label or SHA-1 fingerprint
when more than one such identity is usable. Then provide exactly one Apple notary authentication mode:

- `BOTTIE_APPLE_NOTARY_PROFILE` for a profile already saved by `xcrun notarytool store-credentials`; or
- `BOTTIE_APPLE_NOTARY_KEY_PATH`, `BOTTIE_APPLE_NOTARY_KEY_ID`, and
  `BOTTIE_APPLE_NOTARY_ISSUER_ID` for one protected App Store Connect team API key whose private-key path remains
  outside the repository.

Run the credential-dependent validation explicitly:

```sh
npm run package:macos:distribution
```

The command rebuilds the same locked app-only bundle, signs it with the checked-in minimal hardened-runtime
entitlements and a secure timestamp, submits one temporary ZIP through `notarytool`, staples and validates the ticket,
and requires Gatekeeper to accept the final bundle. It removes the submission ZIP and writes only path-safe,
identity-free JSON evidence under the ignored `package` directory. The protected build also requires Bottie's updater
private key and password through Tauri's signing environment, recreates the updater archive only after notarization
and ticket stapling, signs those exact final bytes, removes all platform-signing variables before native minisign
verification, and records only bounded path-free evidence. It does not publish or upload Bottie as a release.

The manual `macOS distribution validation` GitHub workflow provides the same contract through the protected
`macos-distribution` environment. It expects environment secrets for the base64 PKCS #12 Developer ID certificate,
its password, a temporary-keychain password, the notary team API key, key ID, and issuer ID, plus
`BOTTIE_UPDATER_SIGNING_PRIVATE_KEY` and `BOTTIE_UPDATER_SIGNING_PRIVATE_KEY_PASSWORD`. The job imports protected
values only into runner-temporary storage, uploads bounded evidence for seven days and the exact updater archive plus
signature for one day, and deletes runner copies and temporary credentials even after failure. It does not create a
tag, release, or `latest.json` by itself.

The protected environment and updater secrets are configured, but its Developer ID PKCS #12, temporary-keychain
password, and notarization API-key secrets are not. The workflow therefore remains undispatched until those existing
platform credentials are explicitly configured; source tests are not macOS distribution evidence.

### Windows package verification

On a Windows host with dependencies installed from the checked-in lockfiles, build and inspect the unsigned x64 MSI:

```powershell
npm run icons:check
npm run dependencies:check
npm run package:windows:test
npm run package:windows
npm run package:windows:inspect
```

The build is MSI-only, non-interactive, skips code signing, and passes `--locked` to Cargo. Inspection uses an
administrative `msiexec` extraction instead of installing Bottie, reports the MSI separately, and inventories only the
app directory containing `bottie.exe`. Evidence includes relative paths and hashes, PE architecture, public dimensions
from the executable's associated Bottie icon, Authenticode classification, and loose native runtime files without
certificate identities or host paths.

Run the bounded launch/storage/provider-offline check separately:

```powershell
npm run package:windows:smoke
```

The smoke command builds under the distinct `com.bottie.packaging-smoke` identity, refuses to replace an existing
smoke profile, targets both local providers at one rejecting process-owned loopback endpoint, verifies the fresh store
read-only after termination, and removes only temporary build/extraction data plus that test identity's roaming data.
The checked-in Windows Server 2025 PR workflow uploads the unsigned MSI and path-free JSON evidence for seven days.
Neither command produces a signed or end-user-distributable release.

### Microsoft Store MSIX verification

Bottie's Microsoft Store route remains the exact-identity MSIX below. Store certification and publication are still
deferred. The separately authorized outside-Store updater release uses the Authenticode MSI in the next section and
does not certify, submit, replace, or publish this Store package.
The release owner has created the Individual Partner Center account and reserved Bottie. Partner Center assigned the
following exact case-sensitive identity. These values are public package metadata rather than credentials and must not
be replaced with the account login, a guessed publisher, the calculated package family name, or the package SID.

On a Windows host with the Windows SDK installed, supply those exact values and build the unsigned Store package:

```powershell
$env:BOTTIE_WINDOWS_STORE_IDENTITY_NAME = "ThoughtAgency.bottie"
$env:BOTTIE_WINDOWS_STORE_PUBLISHER = "CN=728BB523-5388-44C6-BEEE-EC334B12A1D6"
$env:BOTTIE_WINDOWS_STORE_PUBLISHER_DISPLAY_NAME = "ThoughtAgency"
$env:BOTTIE_WINDOWS_MAKEAPPX_PATH = "<absolute path to makeappx.exe>"
npm run package:windows:store:test
npm run package:windows:store
```

The repository-owned wrapper performs one locked executable-only Tauri build, constructs the reviewed full-trust x64
layout, invokes Microsoft `MakeAppx.exe`, independently unpacks the result, and checks the exact manifest, executable,
Store artwork, project licence, model notice, third-party notices, SHA2-256 block map, architecture, and unsigned state.
Bottie `0.9.0` maps monotonically to Store package version `1.9.0.0` because Microsoft requires a non-zero first
component and reserves the fourth component.

The manual `Windows Store MSIX validation` workflow accepts the same three public identity inputs, runs the Windows App
Certification Kit, and retains only the unsigned MSIX, path-free JSON evidence, and certification report for seven
days. It uses no certificate, signing secret, Store API, or publication action. Until Microsoft certifies and signs the
package, the workflow artifact is validation material and cannot be installed as a normal public Store application.

Current Windows-native evidence is workflow run
[`32821812167`](https://github.com/hherb/bottie/actions/runs/32821812167) at package-code commit `347b050`: the locked x64
build, MakeAppx pack/unpack inspection, exact public identity and reviewed payload checks, and complete Windows App
Certification Kit run all passed. The 18,114,084-byte unsigned MSIX has SHA-256
`145eb83446aa58d97b8eaf3babcd8cd3f673a03626ff6b36c5a075db5b32d0e6`; the downloaded bounded evidence and WACK report
match their retained hashes. This is local certification-kit evidence, not Microsoft Store certification, signing, or
publication. Microsoft rejected the exact reviewed submission because its screenshots showed macOS rather than
Windows. Store certification and publication remain deferred until further notice from the release owner. No account,
submission,
package-SID, or government-ID material is retained in this repository; the release gate remains closed until a future
matching package is certified and published.

### Protected direct-download MSI signing

The manual `Windows distribution validation` workflow is the selected Windows path for the outside-Store updater
release. It is the only checked-in path that consumes Windows signing credentials. Its protected
`windows-distribution` environment must provide
`BOTTIE_WINDOWS_SIGNING_PFX_BASE64` and `BOTTIE_WINDOWS_SIGNING_CERTIFICATE_PASSWORD`; the certificate is written only
under runner-temporary storage, and the password is exposed only to the signing step. It also requires the two
protected updater-signing secrets named above. The job performs one locked
no-bundle product build, applies SHA-256 Authenticode plus an RFC 3161 timestamp to `bottie.exe`, bundles that exact
signed executable into an otherwise unsigned MSI, then signs and independently verifies the MSI before creating its
Tauri updater signature over those exact final bytes. It then verifies the updater signature natively with every
platform-signing environment variable removed and requires the updater hash to equal the inspected MSI hash. A separate
`com.bottie.packaging-smoke` build supplies the isolated launch/storage/provider-offline result.

The workflow uploads `package/windows-package-evidence.json` for seven days and the exact MSI plus `.sig` for one day,
then always removes the runner PFX and release-byte copies. It retains no certificate labels, subjects, serials,
thumbprints, passwords, host paths, or raw SignTool output. The release-candidate gate now requires both the installer
and extracted executable signatures to be identified, securely timestamped, and independently valid, plus an updater
signature bound to the exact MSI hash. The required-reviewer environment and updater secrets are configured, but no
Authenticode PFX or password is configured. No current Windows distribution or updater evidence is therefore claimed.

### Linux package verification

On an Ubuntu host with the Tauri Linux desktop prerequisites and dependencies installed from the checked-in
lockfiles, build and inspect the unsigned DEB:

```sh
npm run icons:check
npm run dependencies:check
npm run package:linux:test
npm run package:linux
npm run package:linux:inspect
```

The build is DEB-only, non-interactive, skips package signing, and passes `--locked` to Cargo. Inspection extracts the
DEB without installing Bottie and requires the generated 32, 64, 128, and high-density 256 pixel Bottie marks at their
exact hicolor application paths, including Tauri's `256x256@2` directory. It reports only archive metadata,
extraction-relative payload paths and hashes, ELF architecture and direct shared-library requirements, and packaged
native runtime files. Host paths and maintainer scripts are excluded from the JSON evidence.

Run the bounded launch/storage/provider-offline check separately under an available display and D-Bus session:

```sh
dbus-run-session -- xvfb-run --auto-servernum npm run package:linux:smoke
```

The smoke command builds under the distinct `com.bottie.packaging-smoke` identity, confines config, data, cache, and
runtime files to process-owned XDG directories, targets both local providers at one rejecting loopback endpoint,
verifies the fresh store read-only after termination, and removes the complete temporary tree. The checked-in Ubuntu
24.04 PR workflow matches the locked ONNX Runtime archive's glibc/libstdc++ ABI requirements and uploads the unsigned
DEB plus path-free JSON evidence for seven days. It does not install, sign, publish, or claim an end-user-distributable
release.

The separate manual `Linux distribution validation` workflow is the only checked-in path that consumes Linux signing
credentials. Its protected `linux-distribution` environment requires a base64-encoded OpenPGP private key and its
passphrase in `BOTTIE_LINUX_SIGNING_PRIVATE_KEY_BASE64` and `BOTTIE_LINUX_SIGNING_KEY_PASSPHRASE`. The job first builds,
inspects, and smoke-tests the unsigned product bytes, then imports the key only into runner-temporary storage. The
same protected environment also requires the two updater-signing secrets. It
requires that private key to match Bottie's [published Linux public key](distribution/linux/README.md), signs the
canonical `debian-binary`, control-archive, and data-archive bytes, verifies the detached signature with `gpgv`, embeds
exactly one `origin` signature with `debsigs`, requires `debsig-verify` to accept the signed DEB through the published
policy and public key, and only then signs those final DEB bytes for Tauri update delivery. Merely finding an `_gpg*`
archive member is classified as identified but unverified and cannot pass the release gate.

Protected workflow run [`33279780950`](https://github.com/hherb/bottie/actions/runs/33279780950) passed from exact
source `2bb1ead`. Its identity-free evidence binds the 23,442,642-byte final DEB and updater artifact to the same
SHA-256 `e24788260e1688b373ead30fdde985c3974a3eba921b8d3675c16f2e40862eec`, independently verifies the minisign
signature against public-key SHA-256 `fd4adf69a4bea10958a0f63f0658083fa29bfad10c48c792877dcdcdb8c6355c`, and records no
signature content, credential, identity, or host path. A current run uploads evidence for seven days and exact DEB plus
`.sig` release bytes for one day, then removes runner package and protected material. The historical run removed its
DEB after evidence upload and is not release publication.

Run the same protected contract on an already prepared Ubuntu host only when the signing key, temporary policy, and
public keyring have been configured outside the repository:

```sh
npm run package:linux:distribution
```

The command replaces only the ignored Linux evidence file's installer size, SHA-256, and normalized
`identified`/`verifies` state after independent verification. It retains no key ID, fingerprint, user identity,
passphrase, private-key bytes, trust-root path, or raw signing output. The manual workflow uploads bounded evidence for
seven days and exact updater release bytes for one day, then removes both signing material and runner package bytes
even after failure. Its presence is a protected contract, not proof that a current Linux distribution has been signed.

## 0.9.0 beta release candidate

[`RELEASES/0.9.0.md`](RELEASES/0.9.0.md) is the versioned tester-facing source for Bottie's first desktop beta. After
the platform workflows have produced their evidence files, verify the deterministic licence/runtime sources and run
the offline release gate:

```sh
npm run dependencies:check
npm run notices:check
npm run release:assets:check
npm run release:candidate
```

The command always writes an ignored, path-free `package/release-candidate-manifest.json`, then exits non-zero unless
every version, dependency, artwork, licence/notice, runtime-asset, model-terms, package-smoke, Authenticode Windows,
OpenPGP Linux, updater-signature, macOS notarization, and Gatekeeper gate passes. It does not sign, upload, tag, or
publish anything. An unsigned Store workflow artifact, unsigned smoke package, or source test suite is intentionally
insufficient for `ready: true`; Store certification remains a separate deferred route.

The outside-Store release boundary has a credential-free signed-update contract:

```sh
npm run update:contract:test
```

It validates deterministic Tauri static manifests, canonical base64 Tauri signature/public-key file content,
immutable version-tagged GitHub asset URLs, and path-free evidence bound to exact manifest, public-key, and artifact
hashes. The authorized production-key ceremony and Rust-owned runtime boundary are complete. The manual protected
workflow requires an exact full-release confirmation and fresh Gemma-terms acknowledgement, calls all three
required-reviewer platform workflows, refuses stale `main`, passes the release-candidate gate, uploads a complete draft,
verifies GitHub's asset digests, publishes it as the latest full release, and verifies live latest resolution. macOS
still needs its six Apple environment secrets, Windows still needs its PFX and password, and the separate
`updater-publication` environment must remain required-reviewer protected. See
[`distribution/update/README.md`](distribution/update/README.md).

The release owner must read the [Gemma Terms of Use](https://ai.google.dev/gemma/terms) before creating model-terms
evidence. If and only if they accept the reviewed 1 April 2026 terms for this release, the exact acknowledgement is:

```sh
node scripts/release-assets.mjs \
  '--accept-gemma-terms=I have read and accept the Gemma Terms of Use dated 2026-04-01 for Bottie 0.9.0 release review'
```

That deliberate command writes ignored `package/model-terms-evidence.json`, containing only the accepted boolean,
schema version, model revision, and model-notice hash. It includes no operator identity, timestamp, host path, or terms
body. Codex and ordinary build/check commands do not run it or accept legal terms on the release owner's behalf.

Live-provider tests are ignored by default because they require explicitly configured local services and, in some
cases, credentials. See [`HANDOVER.md`](HANDOVER.md) for the current test inventory and the evidence recorded for the
latest completed slice.

## Project documentation

- [`ROADMAP.md`](ROADMAP.md) — product principles, completed milestones, and upcoming work
- [`HANDOVER.md`](HANDOVER.md) — current implementation state, validation evidence, limitations, and next slice
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — documentation, style, TDD, and slice-completion requirements
- [`DEPENDENCY-LICENCES.md`](DEPENDENCY-LICENCES.md) — reproducible package inventory, licence review, and release
  obligations
- [`LICENSE`](LICENSE) and [`THIRD-PARTY-NOTICES.txt`](THIRD-PARTY-NOTICES.txt) — distributable project and locked
  dependency terms
- [`MODEL-NOTICE.txt`](MODEL-NOTICE.txt) and [`runtime-assets.json`](runtime-assets.json) — reviewed Gemma terms notice
  plus immutable EmbeddingGemma, Whisper, and ONNX Runtime identities
- [`RELEASES/0.9.0.md`](RELEASES/0.9.0.md) — tester-facing 0.9.0 beta notes and distribution cautions
- [`MIGRATION-ROLLBACK.md`](MIGRATION-ROLLBACK.md) — forward-only migration and recovery contract

## Current boundaries

Bottie deliberately does not offer arbitrary MCP execution, provider-advertised or executable approval-required
tools, outbound email, original attachment download or opening through Localmail, automatic memory injection, or
office-document extraction beyond DOCX. These are product boundaries, not hidden configuration switches. Follow the
[roadmap](ROADMAP.md) for planned work and its security gates.
