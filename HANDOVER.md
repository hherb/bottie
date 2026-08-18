# bottie handover

Last verified: 2026-08-18

## Start here

Bottie is a greenfield Tauri 2 desktop chatbot. The product shell and both local inference adapters are complete: the native app discovers oMLX and Ollama models and streams text through Rust-owned networking and cancellation. The next session should take the bounded provider-configuration slice from Roadmap 1.3; do not reopen broad product or visual-design planning.

Read these files first:

1. `HANDOVER.md`
2. `ROADMAP.md`
3. `README.md`
4. `src/routes/+page.svelte`
5. `src-tauri/src/lib.rs`
6. `src-tauri/tauri.conf.json`

There is currently no remote repository. Work begins on local branch `main`.

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
- a combined oMLX/Ollama model picker, connection/offline state, retry action, loaded/on-demand state, and local-only privacy indicator;
- user and assistant message presentation;
- a context inspector containing attachments, recalled memories, privacy routing, and a token meter;
- attachment selection and removal in presentation state;
- a composer with memory and web affordances;
- live normalized inference activity and token streaming;
- working stop-generation cancellation backed by a Rust abort handle;
- context-panel open/close behavior;
- reduced-motion and keyboard-focus support.

`src/lib/Icon.svelte` is the dependency-free local icon set used by the shell.

### Native boundary

`src-tauri/src/lib.rs` exposes typed `app_info`, `discover_models`, `start_chat`, and `cancel_chat` commands. Each generation receives an opaque Rust-owned run ID and one typed IPC channel. `src-tauri/src/inference/` contains the provider-neutral types/trait plus the oMLX and Ollama adapters; provider JSON, SSE, and NDJSON parsing do not reach Svelte.

The oMLX adapter:

- owns and validates the fixed `http://127.0.0.1:8000/` loopback endpoint;
- discovers models with `GET /v1/models`;
- streams `POST /v1/chat/completions` SSE responses;
- normalizes started, text delta, usage, completed, cancelled, and failed events;
- maps connection, timeout, HTTP, and malformed-response failures to structured user-readable errors;
- aborts the active HTTP stream when the UI cancels a run.

The Ollama adapter:

- owns and validates the fixed `http://127.0.0.1:11434/` loopback endpoint;
- discovers installed models with `GET /api/tags`, capabilities/context with `POST /api/show`, and loaded state with `GET /api/ps`;
- streams native `POST /api/chat` NDJSON responses;
- normalizes text, prompt/output usage, completion, provider errors, and malformed streams;
- shares the same Rust abort-handle and typed-channel cancellation path as oMLX.

Requests now include a provider ID because model names can collide across local providers. Discovery tolerates either provider being offline and reports a combined retryable error only when neither provides a streaming text model.

The native configuration currently has:

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
- no API credentials or provider settings are stored;
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

## Current bounded slice: Ollama parity

### Goal

Add native Ollama discovery and text streaming behind the existing provider-neutral contract while preserving oMLX behavior, cancellation, and the Rust/WebView privacy boundary.

Do not add SQLite, attachments, memory retrieval, remote-provider credentials, web search, or a general MCP runtime in the same change.

### Implemented shape

1. The Rust inference module is:

   ```text
   src-tauri/src/inference/
   ├── mod.rs
   ├── types.rs
   ├── provider.rs
   ├── ollama.rs
   └── omlx.rs
   ```

2. Serializable internal types cover:

   - provider/model identity;
   - provider capabilities including embeddings;
   - loaded, unloaded, or unknown model state;
   - text content blocks and chat turns;
   - chat request settings;
   - normalized stream events;
   - structured provider errors.

3. The event vocabulary is:

   ```text
   Started
   TextDelta
   UsageUpdated
   Completed
   Cancelled
   Failed
   ```

   Tool, reasoning, citation, image, and audio events belong in later slices unless required to keep the type extensible.

4. Rust-owned provider state uses validated, fixed loopback-only oMLX and Ollama base URLs. No generic URL-fetching command exists.

5. Ollama discovery combines `/api/tags`, `/api/show`, and `/api/ps`; chat uses native `/api/chat` NDJSON rather than Ollama's OpenAI compatibility layer.

6. Normalized events use `tauri::ipc::Channel` rather than global events.

7. Every run has a UUID and an abort handle stored only in Rust.

8. `src/lib/inference.ts` is the typed frontend client; Svelte consumes only normalized events.

9. The model picker uses provider-qualified keys, shows the provider and Ollama loaded/on-demand state, and retains checking, available, offline/retry, and browser-preview states.

10. Discovery runs both providers concurrently, keeps either provider usable when the other is offline, and returns one combined error only when no streaming text model is available.

### Acceptance criteria

- With Ollama running on loopback, bottie discovers at least one chat model and streams a real text reply.
- The UI remains responsive while streaming.
- Stop generation interrupts the Rust task promptly and leaves a valid partial assistant message.
- Network, malformed-event, server, and unavailable-provider failures become user-readable errors rather than panics.
- Provider-specific response data does not leak into UI state outside an optional diagnostic representation.
- No API key or unrestricted HTTP capability reaches the WebView.
- The browser preview presents a clear disconnected or fixture state and does not pretend native inference is available.
- Unit tests cover model/capability/context/load-state decoding, fragmented NDJSON, completion usage, provider errors, loopback enforcement, and cancellation behavior.
- `npm run check`, `npm run build`, `cargo fmt --check`, and `cargo test` pass.
- The native application is manually checked for send, streaming, cancel, offline recovery, and restart.

### Keep the change reviewable

`src/routes/+page.svelte` is intentionally still a large prototype component. Extract only the state or components required to connect real streaming cleanly; avoid a broad visual rewrite during the provider slice. Preserve the current layout and motion unless a functional state requires a small adjustment.

## Verification completed for the current slice

The following passed on 2026-08-18:

```sh
npm run check
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml live_ollama_stream -- --ignored --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml live_omlx_stream -- --ignored --test-threads=1
```

The standard Rust suite has twenty-one tests: seventeen run by default and four are opt-in live-provider tests. Both live Ollama tests passed against the user's running local catalogue, verifying complete streaming and abort-after-first-delta. The earlier live oMLX completion/cancellation checks also passed.

The browser preview was checked at the desktop default and 760 px. It shows the provider-neutral disconnected message, disables composer/send controls, has no horizontal overflow, and produced no console warnings or errors.

The native Tauri app was then launched against the running Ollama catalogue. The user reviewed the resulting provider/model experience and confirmed that it works, completing the native acceptance check for Roadmap 1.2.

The next bounded implementation slice is Roadmap 1.3: provider configuration. Keep FastEmbed/EmbeddingGemma implementation with the first memory-search storage slice, where download progress, cache location, dimensions, and reindex metadata can be implemented coherently.

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
- The repository has no remote configured.
- The first commit contains the full greenfield scaffold and first UI slice.
- Generated frontend output, `node_modules`, Rust targets, environment files, and generated Tauri capability schemas are ignored.
