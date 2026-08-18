# bottie handover

Last verified: 2026-08-18

## Start here

Bottie is a greenfield Tauri 2 desktop chatbot. The product shell, both local inference adapters, and provider configuration are complete: the native app validates and persists loopback endpoints, tests connections, discovers oMLX and Ollama models, remembers the last provider/model pair, and streams text through Rust-owned networking and cancellation. The next bounded implementation slice is Roadmap 1.4; do not reopen broad product or visual-design planning.

Read these files first:

1. `HANDOVER.md`
2. `ROADMAP.md`
3. `README.md`
4. `src/routes/+page.svelte`
5. `src-tauri/src/lib.rs`
6. `src-tauri/tauri.conf.json`

The repository tracks `origin/main` at `https://github.com/hherb/bottie.git`. Work currently begins on local branch `main`.

## Current implementation

### Stack

- Tauri 2 desktop shell
- Rust 2024 application core
- Svelte 5 and SvelteKit in static SPA mode
- TypeScript and Vite
- npm for frontend dependencies

### Working UI

`src/routes/+page.svelte` currently provides:

- desktop conversation navigation and responsive mobile navigation;
- separate provider and model selectors, provider-specific refresh, connection/offline state, retry action, loaded/on-demand state, and a local-only privacy indicator;
- user and assistant message presentation;
- a context inspector containing attachments, recalled memories, privacy routing, and a token meter;
- attachment selection and removal in presentation state;
- a composer with memory and web affordances;
- live normalized inference activity and token streaming;
- working stop-generation cancellation backed by a Rust abort handle;
- a local-provider settings dialog with endpoint editing, connection tests, timeout policy, and redacted session diagnostics;
- context-panel open/close behavior;
- reduced-motion and keyboard-focus support.

`src/lib/Icon.svelte` is the dependency-free local icon set used by the shell.

### Native boundary

`src-tauri/src/lib.rs` exposes typed `app_info`, provider-settings/test/diagnostic commands, `discover_models`, `start_chat`, and `cancel_chat`. Each generation receives an opaque Rust-owned run ID and one typed IPC channel. `src-tauri/src/inference/` contains the provider-neutral types/trait, local settings policy, and the oMLX and Ollama adapters; provider JSON, SSE, and NDJSON parsing do not reach Svelte.

The oMLX adapter:

- owns and validates a configurable loopback endpoint, defaulting to `http://127.0.0.1:8000/`;
- discovers models with `GET /v1/models`;
- streams `POST /v1/chat/completions` SSE responses;
- normalizes started, text delta, usage, completed, cancelled, and failed events;
- maps connection, timeout, HTTP, and malformed-response failures to structured user-readable errors;
- aborts the active HTTP stream when the UI cancels a run.

The Ollama adapter:

- owns and validates a configurable loopback endpoint, defaulting to `http://127.0.0.1:11434/`;
- discovers installed models with `GET /api/tags`, capabilities/context with `POST /api/show`, and loaded state with `GET /api/ps`;
- streams native `POST /api/chat` NDJSON responses;
- normalizes text, prompt/output usage, completion, provider errors, and malformed streams;
- shares the same Rust abort-handle and typed-channel cancellation path as oMLX.

Requests now include a provider ID because model names can collide across local providers. Discovery tolerates either provider being offline and reports a combined retryable error only when neither provides a streaming text model.

Local provider configuration now:

- persists only normalized oMLX and Ollama base URLs in the OS application-config directory;
- remembers the last successfully selected provider/model pair in the same Rust-owned settings file;
- accepts HTTP(S) loopback roots only, with no credentials, subpaths, queries, or fragments;
- disables redirects so a loopback service cannot redirect native traffic to a remote host;
- uses 3-second connect, 5-second discovery, and 120-second stream-idle timeouts;
- keeps the most recent 100 structured diagnostic events in memory and redacts credential-shaped values before returning them to Svelte.

The native application configuration has:

- a minimal `core:default` capability;
- no opener or filesystem plugin permission;
- a restrictive CSP allowing bundled assets and Tauri IPC;
- a 1320 x 820 default window with a 720 x 620 minimum.

## What is still simulated

Do not mistake visual fixtures for implemented backend behavior:

- memory cards and relevance scores are fixtures;
- context usage and tool sources are fixtures; response elapsed time is real, while token usage appears when the selected provider reports it;
- attachments retain only browser-side name, size, and type metadata;
- no attachment bytes are read, copied, extracted, or indexed;
- conversations disappear at restart;
- no SQLite database, migrations, FTS5, or vector extension exists yet;
- no API credentials or remote-provider profiles are stored; only local loopback endpoint settings persist;
- no web search or fetch tool exists;
- there are no automated UI tests yet.

The browser preview intentionally reports `Browser preview`; only the native Tauri runtime can invoke `app_info`.

## Architecture boundaries to preserve

1. The WebView owns rendering and ephemeral presentation state.
2. Rust owns provider calls, credentials, files, persistence, tools, cancellation, and policy enforcement.
3. Never expose API keys, credential-vault values, unrestricted filesystem paths, or raw database access to the WebView.
4. Normalize providers into internal capabilities and stream events. Do not assume every OpenAI-compatible server implements the same features.
5. Keep local-versus-cloud routing explicit and visible before data leaves the device.
6. Represent future messages as content blocks rather than assuming all content is one text string. Planned blocks include text, image, document, audio, tool calls, tool results, citations, and reasoning summaries.
7. Bottie should own its tool loop so memory and web tools behave consistently across providers.
8. Preserve cancellation throughout the stack: UI control, Tauri command, Rust task, and HTTP stream.
9. The memory milestone has settled on Rust-owned FastEmbed with quantized EmbeddingGemma 300M as one built-in default. Do not add a user-facing embedding-provider picker. Model download/cache UX and versioned index metadata must land with the first real embedding consumer, not as a dormant dependency in inference-provider work.

## Current bounded slice: Provider configuration

### Goal

Let users configure and test local providers without weakening the Rust/WebView privacy boundary or adding remote-provider credentials.

Do not add SQLite, attachments, memory retrieval, remote-provider credentials, web search, or a general MCP runtime in the same change.

### Implemented shape

1. `src-tauri/src/inference/settings.rs` owns defaults, normalization, persistence, timeouts, and diagnostic redaction.
2. Provider instances live behind an async Rust lock and are replaced only after both submitted endpoints validate and the settings file saves successfully. An in-flight run retains its original provider instance.
3. `get_provider_settings`, `update_provider_settings`, and `test_provider_connection` are narrow typed commands; the WebView receives no generic URL-fetching capability.
4. The settings UI tests draft endpoints without saving, saves and rediscovers on success, and disables changes during generation.
5. Separate provider and model selectors keep models scoped to the active provider. Changing providers clears stale models, refreshes only the selected provider, chooses its remembered or first available model, and persists the resulting pair through Rust.
6. The active conversation's provider/model pair is snapshotted into every chat request and assistant response label. Durable conversation-specific selection follows with real conversations in Milestone 2.
7. Diagnostics cover discovery, connection tests, settings updates, and generation lifecycle events. They are session-only, bounded to 100 records, and redacted before crossing IPC.

### Acceptance criteria

- Draft loopback endpoints can be tested without changing the active providers.
- Invalid, remote, credential-bearing, or path/query/fragment endpoints are rejected in Rust.
- Saving settings persists normalized URLs, replaces providers, and triggers discovery/reconnect.
- Provider changes refresh only that provider's model list, and the last successful provider/model pair survives restart.
- Provider/model choice remains explicit and provider-qualified for the active conversation.
- Network and unavailable-provider failures become user-readable errors rather than panics.
- Redirects cannot turn a configured loopback endpoint into unrestricted native HTTP access.
- Diagnostic values are structured, bounded, and secret-redacted.
- The browser preview keeps settings read-only and does not pretend native inference is available.
- Unit tests cover endpoint policy, persistence round trips, and redaction in addition to the provider protocol/cancellation suite.
- `npm run check`, `npm run build`, `cargo fmt --check`, and `cargo test` pass.
- The native application is manually checked for invalid endpoint rejection, offline test feedback, successful connection testing, save/reconnect, and settings persistence after restart.

### Keep the change reviewable

`src/routes/+page.svelte` is intentionally still a large prototype component. Extract only the state or components required to connect real streaming cleanly; avoid a broad visual rewrite during the provider slice. Preserve the current layout and motion unless a functional state requires a small adjustment.

## Verification completed for the current slice

The following passed on 2026-08-18 for the provider-configuration implementation:

```sh
npm run check
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

The standard Rust suite now has twenty-seven tests: twenty-three run by default and four are opt-in live-provider tests. The opt-in Ollama tests were attempted during implementation but Ollama was not running on `127.0.0.1:11434`; Roadmap 1.2 retains its earlier completed live streaming and cancellation evidence.

The browser preview was checked at the desktop default and 760 px. It shows the provider-neutral disconnected message, disables composer/send controls, has no horizontal overflow, and produced no console warnings or errors.

The native provider/model experience was manually reviewed after implementation. The user confirmed that the separate selectors, provider-specific model refresh, and remembered selection work as intended, completing Roadmap 1.3 acceptance.

The next bounded implementation slice is Roadmap 1.4: native remote OpenAI and Anthropic adapters, compatible endpoint profiles, operating-system credential-vault storage, and an explicit local/cloud routing indicator. Keep FastEmbed/EmbeddingGemma implementation with the first memory-search storage slice, where download progress, cache location, dimensions, and reindex metadata can be implemented coherently.

## Development commands

```sh
npm install
npm run check
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri dev
```

Use the explicit Cargo manifest because the repository root is not a Cargo workspace.

## Known housekeeping

- Tauri's default application icons and favicon remain; replace them in the branding/distribution phase.
- The repository tracks GitHub remote `origin`.
- The first commit contains the full greenfield scaffold and first UI slice.
- Generated frontend output, `node_modules`, Rust targets, environment files, and generated Tauri capability schemas are ignored.
