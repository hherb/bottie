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
> Bottie is an active developer preview. It is not yet distributed as a signed end-user release.

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
- Search and open email through an explicitly pinned Localmail connection without exposing credentials or raw mail
  internals to the WebView.
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

## The trust boundary

Bottie is local-first, not local-only. The interface makes the selected provider and enabled context routes visible
before a request leaves the application.

| The WebView receives              | The Rust core owns                                              |
| --------------------------------- | --------------------------------------------------------------- |
| UI-safe typed state               | Conversations, indexes, backups, and recovery                   |
| Path-free file metadata           | Paths, bytes, extraction, image normalization, and hashes       |
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

## Provider support

| Provider             | Route                           | Capability-aware vision | Native tool loop |
| -------------------- | ------------------------------- | ----------------------- | ---------------- |
| oMLX                 | Local loopback                  | Yes                     | Yes              |
| Ollama               | Local loopback                  | Yes                     | Yes              |
| OpenAI-compatible    | Explicit user-configured origin | Yes                     | Yes              |
| Anthropic-compatible | Explicit user-configured origin | Yes                     | Yes              |

Features are enabled from discovered or mapped model capabilities rather than model-name guesses. Memory, Web, and
Email are session-only controls and remain off by default. Email additionally requires a saved, trusted Localmail
connection and is read-only.

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
identity-free JSON evidence under the ignored `package` directory. It does not publish or upload Bottie as a release.

The manual `macOS distribution validation` GitHub workflow provides the same contract through the protected
`macos-distribution` environment. It expects environment secrets for the base64 PKCS #12 Developer ID certificate,
its password, a temporary-keychain password, and the notary team API key, key ID, and issuer ID. The job imports those
values only into runner-temporary storage, uploads only the bounded evidence JSON for seven days, and deletes the
temporary keychain and credential files even after failure.

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

Live-provider tests are ignored by default because they require explicitly configured local services and, in some
cases, credentials. See [`HANDOVER.md`](HANDOVER.md) for the current test inventory and the evidence recorded for the
latest completed slice.

## Project documentation

- [`ROADMAP.md`](ROADMAP.md) — product principles, completed milestones, and upcoming work
- [`HANDOVER.md`](HANDOVER.md) — current implementation state, validation evidence, limitations, and next slice
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — documentation, style, TDD, and slice-completion requirements
- [`DEPENDENCY-LICENCES.md`](DEPENDENCY-LICENCES.md) — reproducible package inventory, licence review, and release
  gaps
- [`MIGRATION-ROLLBACK.md`](MIGRATION-ROLLBACK.md) — forward-only migration and recovery contract

## Current boundaries

Bottie deliberately does not offer arbitrary MCP execution, approval-required tools, outbound email, attachment access
through Localmail, automatic memory injection, or office-document extraction beyond DOCX. These are product boundaries,
not hidden configuration switches. Follow the [roadmap](ROADMAP.md) for planned work and its security gates.
