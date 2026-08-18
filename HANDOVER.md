# bottie handover

Last verified: 2026-08-18

## Start here

Bottie is a greenfield Tauri 2 desktop chatbot. The first vertical slice is complete: a polished, responsive conversation shell backed by a minimal typed Rust command. The next session should continue with the bounded inference slice described below rather than reopening broad product or visual-design planning.

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
- an oMLX/model status header and local-only privacy indicator;
- user and assistant message presentation;
- a context inspector containing attachments, recalled memories, privacy routing, and a token meter;
- attachment selection and removal in presentation state;
- a composer with memory and web affordances;
- simulated memory/file activity stages;
- simulated token streaming;
- working stop-generation cancellation for the simulation;
- context-panel open/close behavior;
- reduced-motion and keyboard-focus support.

`src/lib/Icon.svelte` is the dependency-free local icon set used by the shell.

### Native boundary

`src-tauri/src/lib.rs` exposes a typed `app_info` Tauri command. It proves the frontend-to-Rust path and returns the package version and local storage mode.

The native configuration currently has:

- a minimal `core:default` capability;
- no opener or filesystem plugin permission;
- a restrictive CSP allowing bundled assets and Tauri IPC;
- a 1320 x 820 default window with a 720 x 620 minimum.

## What is still simulated

Do not mistake visual fixtures for implemented backend behavior:

- no inference provider is connected;
- the displayed oMLX model and localhost route are hard-coded examples;
- memory cards and relevance scores are fixtures;
- token counts, timing, context usage, and tool sources are fixtures;
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

## Next bounded slice: real oMLX streaming

### Goal

Replace the simulated response path with one real text-only oMLX conversation streamed through the Rust core, while keeping the existing UI behavior and privacy boundary intact.

This slice should introduce only enough provider abstraction to make the next Ollama adapter straightforward. Do not add SQLite, attachments, memory retrieval, remote-provider credentials, web search, or a general MCP runtime in the same change.

### Suggested implementation order

1. Create a small Rust inference module, for example:

   ```text
   src-tauri/src/inference/
   ├── mod.rs
   ├── types.rs
   ├── provider.rs
   └── omlx.rs
   ```

2. Define serializable internal types for:

   - provider/model identity;
   - provider capabilities;
   - text content blocks and chat turns;
   - chat request settings;
   - normalized stream events;
   - structured provider errors.

3. Start with a narrow event vocabulary:

   ```text
   Started
   TextDelta
   UsageUpdated
   Completed
   Cancelled
   Failed
   ```

   Tool, reasoning, citation, image, and audio events belong in later slices unless required to keep the type extensible.

4. Add Rust-owned provider state with a default loopback oMLX base URL. Validate the URL and do not allow arbitrary URL fetching through a generic WebView command.

5. Implement oMLX model discovery using `GET /v1/models` and text streaming using `POST /v1/chat/completions` with server-sent events.

6. Stream normalized events to the WebView with Tauri's typed IPC channel (`tauri::ipc::Channel`) rather than many global events. The installed Tauri 2.11 source includes this API.

7. Give every run an opaque ID owned by Rust. Add cancellation that aborts the corresponding Rust task and HTTP response stream.

8. Replace hard-coded frontend streaming logic with a small client/view-model module. Keep provider JSON and SSE parsing out of Svelte components.

9. Populate the model picker from discovery results and show a clear offline state when oMLX is unavailable.

10. Preserve the existing simulated path only as an explicit development fixture if it remains useful; it must not masquerade as a connected provider.

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

The following passed on 2026-08-18:

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

## Development commands

```sh
npm install
npm run check
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
npm run tauri dev
```

Use the explicit Cargo manifest because the repository root is not a Cargo workspace.

## Known housekeeping

- Tauri's default application icons and favicon remain; replace them in the branding/distribution phase.
- The repository has no remote configured.
- The first commit contains the full greenfield scaffold and first UI slice.
- Generated frontend output, `node_modules`, Rust targets, environment files, and generated Tauri capability schemas are ignored.
