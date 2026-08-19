# bottie handover

Last verified: 2026-08-19

## Start here

Bottie is a greenfield Tauri 2 desktop chatbot. Milestone 1 is complete: the native app supports oMLX, Ollama,
OpenAI-compatible, and Anthropic-compatible text inference through Rust-owned networking, credentials, streaming, and
cancellation. Milestone 2 is in progress: a Rust-owned bundled SQLite store now persists local-profile conversations
and ordered text/reasoning messages across restart. Accepted provider runs now persist their request link, provider,
model, generation settings, terminal outcome, timing, provider-reported usage, and crash-safe partial text/reasoning
checkpoints. Runs left active by an earlier process reopen as visibly interrupted partial responses. Users can rename,
archive, soft-delete, restore, and browse real conversations in calendar-date groups. Provider selection remains
explicit, cloud routes are visible before sending, and credential-vault values are never returned to the WebView. The
next bounded implementation slice is restoring the exact last-open built-in local-profile conversation after restart; do
not reopen broad product or visual-design planning.

Read these files first:

1. `HANDOVER.md`
2. `ROADMAP.md`
3. `README.md`
4. `CONTRIBUTING.md`
5. `src/routes/+page.svelte`
6. `src/routes/page-state.svelte.ts`
7. `src-tauri/src/lib.rs`
8. `src-tauri/tauri.conf.json`

The repository tracks `origin/main` at `https://github.com/hherb/bottie.git`. Work currently begins on local branch `main`.

## Current implementation

### Stack

- Tauri 2 desktop shell
- Rust 2024 application core
- Svelte 5 and SvelteKit in static SPA mode
- TypeScript and Vite
- npm for frontend dependencies

### Working UI

`src/routes/+page.svelte` composes focused presentation components under `src/lib/`, while
`src/routes/page-state.svelte.ts` owns the shared reactive conversation state and actions. Together they provide:

- desktop conversation navigation and responsive mobile navigation;
- separate provider and model selectors, provider-specific refresh, connection/offline state, retry action,
  loaded/on-demand state, and an explicit local/cloud privacy indicator;
- user and assistant message presentation;
- a context inspector containing attachments, recalled memories, privacy routing, and a token meter;
- attachment selection and removal in presentation state;
- a composer with memory and web affordances;
- live normalized inference activity and token streaming;
- an off-by-default reasoning toggle with low effort when enabled;
- collapsed reasoning sections that can be expanded independently of answer text;
- working stop-generation cancellation backed by a Rust abort handle;
- durable conversation creation on first send, recent-conversation navigation, and restart-safe reopening;
- crash-safe partial answer/reasoning checkpoints and visibly interrupted-run recovery;
- real Today, Yesterday, Previous 7 days, Archived, and Trash navigation groups;
- inline conversation rename plus archive, unarchive, recoverable trash, and restore actions;
- a provider settings dialog with endpoint editing, OS-vault credential management, connection tests, timeout policy,
  and redacted session diagnostics;
- context-panel open/close behavior;
- reduced-motion and keyboard-focus support.

`src/lib/chat.ts` contains tested pure presentation helpers, `src/lib/presentation.ts` owns typed fixtures and named UI
constants, and `src/lib/styles/` keeps cohesive stylesheets below the project file-size limit. `src/lib/Icon.svelte` is
the dependency-free local icon set used by the shell.

### Native boundary

`src-tauri/src/lib.rs` exposes typed `app_info`, provider-settings/test/diagnostic commands, `discover_models`,
`start_chat`, and `cancel_chat`. Each generation receives an opaque Rust-owned run ID and one typed IPC channel.
`src-tauri/src/inference/` contains the provider-neutral types/trait, endpoint policy, and four concrete adapters.
`src-tauri/src/provider_registry.rs` resolves an explicit route, while `src-tauri/src/credentials.rs` is the only
credential-vault boundary. Provider JSON, SSE, and NDJSON parsing do not reach Svelte.

`src-tauri/src/storage.rs` owns short-lived configured SQLite connections, migrations, integrity policy, and
transactional conversation/message operations. `src-tauri/src/storage/runs.rs` owns provider-run and usage records,
partial-response checkpoints, and interrupted-run recovery, while `src-tauri/src/generation.rs` closes each native run
before its terminal stream event reaches the WebView. `src-tauri/src/storage_commands.rs` exposes only list, create,
load, user-message append, and explicit lifecycle commands. The
database lives in the OS application-data directory; the WebView never receives a path, SQL, or generic database
capability. One built-in `local` profile represents the current OS account. Every conversation has
a main branch, and every message stores a branch-local append sequence plus independently ordered text/reasoning
blocks. User prompts commit before inference starts, and terminal assistant responses commit before another prompt can
append. Rust creates each assistant response with its run, checkpoints every provider text/reasoning delta before IPC,
and marks leftover running records interrupted during the next startup. Assistant responses reference opaque native
provider runs, and reopened conversations reconstruct real elapsed time plus provider-reported token/cost usage without
estimating missing values.

The oMLX adapter:

- owns and validates a configurable loopback endpoint, defaulting to `http://127.0.0.1:8000/`;
- discovers models with `GET /v1/models`;
- streams `POST /v1/chat/completions` SSE responses;
- normalizes started, text delta, reasoning delta, usage, completed, cancelled, and failed events;
- maps connection, timeout, HTTP, and malformed-response failures to structured user-readable errors;
- aborts the active HTTP stream when the UI cancels a run.

The Ollama adapter:

- owns and validates a configurable loopback endpoint, defaulting to `http://127.0.0.1:11434/`;
- discovers installed models with `GET /api/tags`, capabilities/context with `POST /api/show`, and loaded state with `GET /api/ps`;
- streams native `POST /api/chat` NDJSON responses;
- normalizes answer text, separate thinking text, prompt/output usage, completion, provider errors, and malformed
  streams;
- shares the same Rust abort-handle and typed-channel cancellation path as oMLX.

The remote adapters:

- use separate native OpenAI Chat Completions and Anthropic Messages request and stream shapes;
- validate configurable HTTPS roots, reject embedded credentials/query/fragment values, and disable redirects;
- retrieve API keys just in time from the operating-system credential vault without returning them to Svelte;
- require Touch ID for the first read of each saved credential per macOS app session, then retain the unlocked value only
  in process memory;
- discover remote models and normalize answer, reasoning, usage, cancellation, and provider errors;
- preserve provider-reported USD cost metadata when compatible endpoints include it.

Requests include a provider ID because model names can collide across providers. Initial discovery tolerates either
local provider being offline and reports a combined retryable error only when neither reports a streaming text model.

Provider configuration now:

- persists normalized endpoint roots and the remembered provider/model pair in the OS application-config directory;
- keeps API keys out of that file and stores only remote credential availability in UI state;
- remembers the last successfully selected provider/model pair in the same Rust-owned settings file;
- accepts HTTP(S) loopback roots for local providers and HTTPS roots for remote profiles, with no embedded credentials,
  queries, or fragments;
- disables redirects for every provider client;
- uses 3-second connect, 5-second discovery, and 120-second stream-idle timeouts;
- keeps the most recent 100 structured diagnostic events in memory and redacts credential-shaped values before returning them to Svelte.

Generation settings now default to reasoning off and a 4,096-token completion ceiling. The toolbar can enable low-effort
reasoning for the next request. Rust maps that provider-neutral setting to oMLX `enable_thinking`/`reasoning_effort` and
Ollama `think`, while normalized reasoning deltas remain separate from assistant answer text throughout IPC and UI
state.

The native application configuration has:

- a minimal `core:default` capability;
- no opener or filesystem plugin permission;
- a restrictive CSP allowing bundled assets and Tauri IPC;
- a 1320 x 820 default window with a 720 x 620 minimum.

## What is still simulated

Do not mistake visual fixtures for implemented backend behavior:

- memory cards and relevance scores are fixtures;
- context-panel usage and tool sources are fixtures; response elapsed time and provider-reported token/cost usage are
  real and survive conversation reopen;
- attachments retain only browser-side name, size, and type metadata;
- no attachment bytes are read, copied, extracted, or indexed;
- branching, exact last-open-conversation restoration, search, export, and backup/restore are not yet implemented;
- tool-invocation persistence is not yet implemented;
- reasoning-toggle state is session-only and resets to off when the app restarts;
- SQLite conversation storage exists, but no FTS5 or vector extension exists yet;
- no web search or fetch tool exists;
- there are no automated component or end-to-end UI tests yet; pure presentation helpers have frontend unit coverage.

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

## Engineering rules and housekeeping

`CONTRIBUTING.md` is the canonical repository guidance. Docstrings are mandatory; files should remain under 500 lines
and lines at or below 120 characters where practical; pure reusable functions and named constants are preferred; TDD
is required for testable functionality; and each completed slice must update relevant documentation.

The 2026-08-18 housekeeping slice applied those rules to the existing code without changing product scope:

- the 1,360-line page prototype was split into a 101-line composition root, a reactive state module, focused Svelte
  components, tested pure helpers, and cohesive stylesheets;
- the Ollama adapter was split into provider I/O, pure protocol normalization, and colocated tests;
- oMLX tests, command transfer types, and bounded diagnostics were separated from their orchestration files;
- the diagnostic history capacity and frontend conversion/layout limits now use named constants;
- Rust crate documentation is enforced with `#![deny(missing_docs)]`, and TypeScript exports and functions carry JSDoc;
- Vitest and Prettier checks are part of the standard frontend workflow.

All handwritten Rust, TypeScript, Svelte, and CSS files are now below 500 lines. The remaining lines over 120 characters
are four indivisible SVG path values in `src/lib/Icon.svelte`.

## Most recently completed product slice: Crash-safe partial responses and interrupted-run recovery

### Goal

Retain the latest provider output across an application or process interruption without allowing the WebView to author
assistant history or reconstruct provider-run state.

### Implemented shape

1. Starting a native provider run atomically creates its empty, provider-linked assistant checkpoint immediately after
   the persisted user request.
2. Rust appends every non-empty text/reasoning delta and cumulative usage update transactionally before forwarding the
   matching normalized event to Svelte.
3. Completed, cancelled, and failed outcomes update the native-owned assistant state and provider run before their
   terminal stream event reaches the WebView.
4. Startup converts leftover `running` records into failed runs with the stable `interrupted` category while retaining
   their assistant message as a visible partial response. Older version-three running records without a response gain
   an empty recovery checkpoint.
5. The WebView storage command can append only final user messages. Reopened interrupted, cancelled, and failed
   responses use tested stable labels and safe empty-response fallbacks.

### Acceptance criteria

- A provider run can start only from the latest persisted user request on a non-deleted local-profile branch.
- Text/reasoning content visible through IPC has already committed to SQLite with exact fragment whitespace.
- Terminal provider-run, message, and final usage state commit before the terminal IPC event.
- Restart recovery preserves partial blocks, marks the run interrupted, and allows another user prompt to append.
- Cancellation keeps the composer blocked until native orchestration has saved the terminal checkpoint.
- Existing version-three stores require no schema rewrite and recover pre-checkpoint running records safely.

## Prior completed product slice: Provider-run provenance and usage persistence

### Goal

Retain auditable generation provenance and provider-reported usage across restart without allowing the WebView to
invent usage records or exposing generic database access.

### Implemented shape

1. Schema version 3 adds profile-owned provider runs linked to their conversation, main branch, and persisted user
   request, plus append-only cumulative usage snapshots and an optional response-message link.
2. Native generation records the provider, model, reasoning effort, temperature, output ceiling, and start time before
   network work begins.
3. Rust closes completed, cancelled, and failed runs before sending their terminal stream event. Stable provider error
   categories are retained without storing raw provider responses or credential-shaped diagnostics.
4. Provider-reported input tokens, output tokens, and compatible-endpoint USD cost are written only by native
   orchestration. Missing values remain absent rather than estimated.
5. Assistant messages reference the opaque native run, and reopening a conversation reconstructs elapsed time and
   usage/cost metadata below the response.

### Acceptance criteria

- A version 2 database migrates transactionally to version 3 without rewriting existing messages.
- A provider run can start only from a persisted user request in the same non-deleted local-profile conversation.
- Terminal state and final cumulative usage survive database reopen and remain linked to the assistant response.
- Response links must match the run's conversation, branch, provider, and model.
- Provider-run persistence completes before the matching terminal stream event reaches the WebView.

## Prior completed product slice: Conversation lifecycle management

### Goal

Make durable conversations manageable without permanently deleting user content or broadening the native storage
boundary.

### Implemented shape

1. Conversation summaries expose one Rust-derived `active`, `archived`, or `deleted` lifecycle state without exposing
   raw archive/delete timestamps or SQL to the WebView.
2. Narrow native commands rename, archive/unarchive, move to recoverable trash, and restore profile-owned
   conversations. Empty titles and invalid lifecycle transitions return stable secret-free errors.
3. Archived conversations remain readable; appending to one reactivates it. Trashed conversations cannot load or
   append until restored, and restore preserves every message and content block.
4. The sidebar groups active conversations by local calendar date and provides separate Archived and Trash groups.
5. Each conversation has a keyboard-focusable action menu and inline bounded rename editor. Lifecycle actions are
   disabled during generation, and archiving or deleting the open thread starts a clean conversation view.

### Acceptance criteria

- Rename collapses whitespace and applies the same 80-character native title bound used at creation.
- Archive/unarchive and trash/restore survive database reopen without losing messages.
- Deleted conversations reject load and append through the narrow storage API.
- Active conversations appear under Today, Yesterday, Previous 7 days, or Older based on real update timestamps.
- Archived and trashed conversations remain visibly recoverable in dedicated sidebar groups.
- Desktop and narrow navigation remain accessible and free of console errors.

## Prior completed product slice: Durable conversation storage foundation

### Goal

Resolve Milestone 2's ownership design gate and make the existing text conversation flow survive application restart
without exposing a generic database capability to the WebView.

### Implemented shape

1. Bottie uses one built-in local profile scoped to the current OS account; explicit profile ownership remains in the
   schema for future optional local profiles.
2. Bundled SQLite runs in WAL mode with foreign keys, a busy timeout, startup quick-check, and ordered transactional
   migrations recorded both in `user_version` and `schema_migrations`.
3. Conversations, main branches, messages, and text/reasoning blocks use UUID identities. Messages also use a unique
   branch-local append sequence so ordering does not depend on wall-clock resolution.
4. Narrow Tauri commands list, create, load, and append. SQL, database paths, migration details, and connections never
   cross IPC.
5. The first prompt creates a conversation and commits the user message before inference. Final, cancelled, and failed
   assistant responses commit before the composer can send the next prompt.
6. The sidebar lists real recent conversations, reopens them, and restores the most recent thread at native startup.

### Acceptance criteria

- A fresh native launch creates and migrates the application-data database without exposing its path to the WebView.
- Reopening a store preserves conversation title, ordered roles, answer text, separate reasoning, provider, and model.
- Equal message timestamps cannot reorder a branch.
- Empty messages and unknown conversations fail through a stable secret-free storage error.
- Browser preview retains fixtures and does not pretend to persist conversations.
- Desktop and narrow sidebar states remain accessible and free of overflow or console errors.

## Prior completed product slice: Remote OpenAI and Anthropic APIs

### Goal

Complete real text inference for explicit cloud routes without exposing credentials or provider traffic to the
WebView.

### Implemented shape

1. Dedicated OpenAI-compatible Chat Completions and Anthropic-compatible Messages adapters preserve their distinct
   authentication, request, reasoning, streaming, and usage shapes.
2. Remote endpoint profiles require HTTPS and share the existing redirect-disabled timeout and diagnostic policy.
3. Narrow native commands write or remove fixed-provider API-key entries in the OS credential vault; the WebView can
   query only whether a credential exists.
4. Provider and model selectors include the remote profiles, while the toolbar and context inspector identify cloud
   routing before a prompt can be sent.
5. Normalized usage carries provider-reported prompt/output token counts and optional USD cost without estimating a
   price in the interface.

### Acceptance criteria

- OpenAI-compatible and Anthropic-compatible profiles discover models and stream through their native protocol shapes.
- API keys are absent from application settings, IPC responses, diagnostics, and frontend state after saving.
- Saved macOS credentials require Touch ID before their first use in a new Bottie process.
- Remote profiles reject non-HTTPS and credential-bearing endpoints; all provider clients reject redirects.
- The selected route is visibly local or cloud before generation, including responsive layouts.
- Reasoning remains explicit and bounded, and answer/reasoning deltas stay separate.
- Usage and cost render only when returned by the provider.

## Prior completed product slice: Bounded reasoning controls

### Goal

Prevent reasoning-capable models from appearing stalled while retaining explicit user control and keeping answer text
separate from optional model reasoning.

### Implemented shape

1. `ChatSettings` has a provider-neutral `off`/`low` reasoning effort, defaults to off, and applies a named 4,096-token
   completion ceiling when callers omit a limit.
2. oMLX receives explicit chat-template thinking control and low reasoning effort; Ollama receives `think: false` or
   `think: "low"`.
3. Both adapters decode provider-specific reasoning fields into a dedicated normalized stream event.
4. Assistant state retains reasoning separately and renders it in a collapsed, keyboard-operable disclosure section.
5. The toolbar switch is disabled during generation so each request snapshots one stable reasoning level.

### Acceptance criteria

- Reasoning is off on launch and ordinary requests do not inherit a model's potentially expensive thinking default.
- Enabling reasoning requests the provider's low effort level.
- Reasoning activity becomes visible without exposing it in the answer body by default.
- Answer and reasoning deltas are never conflated in the normalized stream.
- Requests cannot silently inherit oMLX's 32,768-token default completion allowance.

## Earlier completed product slice: Provider configuration

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

The original page prototype is now split by responsibility. Continue extracting only cohesive behavior or presentation
units required by a slice, and preserve the current layout and motion unless functional state requires an adjustment.

## Verification completed for the current slice

The following passed on 2026-08-19 for crash-safe partial-response persistence and interrupted-run recovery:

```sh
npm run check
npm run format:check
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

The standard Rust suite now has fifty-seven tests: fifty-three run by default and four are opt-in live-provider tests.
All four live oMLX/Ollama streaming and cancellation tests passed when run separately with unrestricted loopback access.
Thirteen storage tests cover migration and profile policy, version 2-to-3 upgrade, WAL/foreign-key/integrity state,
transactional conversation/message round trips, branch-local ordering under equal timestamps, provider/model/reasoning
retention, provider-run/usage reconstruction, native partial checkpoints, legacy running-record recovery, lifecycle
transitions, recoverable deletion, and invalid input. The frontend suite has thirteen pure-helper tests, including
durable completion metadata, recovered-message labels, and local-calendar date grouping.

This slice did not change layout or interaction structure, so the prior browser-preview layout checks remain applicable.

The native application compiled and launched cleanly with the current schema version 3 store. Unrestricted direct
probes confirmed both oMLX and Ollama online on their configured default loopback ports. The storage suite exercises
process-reopen recovery against a real path-backed SQLite database without modifying the user's application data.

The next bounded implementation slice is restoring the exact last-open built-in local-profile conversation after
restart. Keep edit/regenerate branching, tool-invocation persistence, export, and backups as later reviewable slices.
Keep FastEmbed/EmbeddingGemma implementation with the first memory-search consumer, where download progress, cache
location, dimensions, and reindex metadata can be implemented coherently.

## Development commands

```sh
npm install
npm run format:check
npm run check
npm test
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
