# bottie handover

Last verified: 2026-08-18

## Start here

Bottie is a greenfield Tauri 2 desktop chatbot. The product shell and the first inference implementation are complete: the native app discovers local oMLX models and streams text through Rust-owned networking and cancellation. The next session should take the bounded Ollama adapter slice from Roadmap 1.2; do not reopen broad product or visual-design planning.

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
- a live oMLX model picker, connection/offline state, retry action, and local-only privacy indicator;
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

`src-tauri/src/lib.rs` exposes typed `app_info`, `discover_models`, `start_chat`, and `cancel_chat` commands. Each generation receives an opaque Rust-owned run ID and one typed IPC channel. `src-tauri/src/inference/` contains the provider-neutral types/trait plus the oMLX adapter; provider JSON and SSE parsing do not reach Svelte.

The oMLX adapter:

- owns and validates the fixed `http://127.0.0.1:8000/` loopback endpoint;
- discovers models with `GET /v1/models`;
- streams `POST /v1/chat/completions` SSE responses;
- normalizes started, text delta, usage, completed, cancelled, and failed events;
- maps connection, timeout, HTTP, and malformed-response failures to structured user-readable errors;
- aborts the active HTTP stream when the UI cancels a run.

The native configuration currently has:

- a minimal `core:default` capability;
- no opener or filesystem plugin permission;
- a restrictive CSP allowing bundled assets and Tauri IPC;
- a 1320 x 820 default window with a 720 x 620 minimum.

## What is still simulated

Do not mistake visual fixtures for implemented backend behavior:

- memory cards and relevance scores are fixtures;
- context usage and tool sources are fixtures; response elapsed time is real, while token usage appears only when oMLX reports it;
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

## Current bounded slice: real oMLX streaming

### Goal

Replace the simulated response path with one real text-only oMLX conversation streamed through the Rust core, while keeping the existing UI behavior and privacy boundary intact.

This slice should introduce only enough provider abstraction to make the next Ollama adapter straightforward. Do not add SQLite, attachments, memory retrieval, remote-provider credentials, web search, or a general MCP runtime in the same change.

### Implemented shape

1. The Rust inference module is:

   ```text
   src-tauri/src/inference/
   ├── mod.rs
   ├── types.rs
   ├── provider.rs
   └── omlx.rs
   ```

2. Serializable internal types cover:

   - provider/model identity;
   - provider capabilities;
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

4. Rust-owned provider state uses a validated loopback-only oMLX base URL. No generic URL-fetching command exists.

5. Model discovery and text streaming use the oMLX endpoints above.

6. Normalized events use `tauri::ipc::Channel` rather than global events.

7. Every run has a UUID and an abort handle stored only in Rust.

8. `src/lib/inference.ts` is the typed frontend client; Svelte consumes only normalized events.

9. The model picker uses discovery results and the UI has checking, available, offline/retry, and browser-preview states.

10. Simulated response generation was removed. The opening conversation remains explicitly labelled `Product shell fixture`.

### Acceptance criteria

- With oMLX running on loopback, bottie discovers at least one model and streams a real text reply.
- The UI remains responsive while streaming.
- Stop generation interrupts the Rust task promptly and leaves a valid partial assistant message.
- Network, malformed-event, server, and unavailable-provider failures become user-readable errors rather than panics.
- Provider-specific response data does not leak into UI state outside an optional diagnostic representation.
- No API key or unrestricted HTTP capability reaches the WebView.
- The browser preview presents a clear disconnected or fixture state and does not pretend native inference is available.
- Unit tests cover model-list decoding, SSE event decoding, completion, provider errors, and cancellation behavior where practical.
- `npm run check`, `npm run build`, `cargo fmt --check`, and `cargo test` pass.
- The native application is manually checked for send, streaming, cancel, offline recovery, and restart.

### Keep the change reviewable

`src/routes/+page.svelte` is intentionally still a large prototype component. Extract only the state or components required to connect real streaming cleanly; avoid a broad visual rewrite during the provider slice. Preserve the current layout and motion unless a functional state requires a small adjustment.

## Verification completed for the current slice

The following passed on 2026-08-18 before the inference change:

```sh
npm run check
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
npm run tauri dev
```

Manual interaction checks covered:

- desktop rendering;
- 760 px responsive rendering without horizontal overflow;
- context-panel close and reopen;
- simulated activity stages;
- streamed text rendering;
- stop-generation cancellation;
- browser console errors, with none observed.

The native development process was stopped after verification.

For the oMLX slice, the following have passed:

```sh
npm run check
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml live_omlx_stream -- --ignored --test-threads=1
```

The standard Rust suite currently has eleven tests: nine run by default and two opt-in live oMLX tests. The live tests used `mlx-community--LFM2-1.2B-8bit` from the running local catalogue and verified both complete streaming and abort-after-first-delta. The browser preview was checked for a visible disconnected message, disabled composer/send controls, and no console errors. The native app was launched twice and visually confirmed to rediscover the real oMLX model catalogue and connected loopback route after restart. The user manually confirmed model selection, incremental response streaming, and Stop preserving a valid partial assistant response. Unavailable-provider mapping and retry presentation are covered by automated tests and browser-state inspection; physically stopping and restarting oMLX remains an optional smoke check.

The next bounded implementation slice is Roadmap 1.2: an Ollama adapter with parity against the same normalized stream contract. Do not combine Ollama with persistence or provider-settings work.

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
