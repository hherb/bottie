# Architecture and request flow

[Back to the manual index](index.md)

## Process model

```text
User
  v
Svelte components + PageState (untrusted WebView)
  | typed invoke() / bounded event channels
  v
Tauri commands (Rust authority boundary)
  |-- provider registry/adapters --> local or explicit cloud provider
  |-- ConversationStore ---------> app-private SQLite and files
  |-- credential store -----------> operating-system vault
  |-- tool policy/dispatch -------> web, Localmail, protected Python
  `-- controllers ---------------> mic, speech, image worker, updater
```

TypeScript owns presentation and transient UI coordination. Rust owns authority: secrets, validation, durable state,
native handles/paths, outbound clients, policy, and limits. A disabled button is usability—not authorization.

## Startup and composition

`src-tauri/src/main.rs` calls `bottie_lib::run()`. In `src-tauri/src/lib.rs`, `run` constructs `AppState`, initializes
storage/controllers, registers the `generate_handler!` command list, and starts background work. `AppState` is similar
to a C# dependency-injection container; Tauri injects `State<'_, AppState>` into commands.

`src/routes/+layout.ts` disables SSR. `+page.svelte` constructs `PageState`, installs appearance/development preview
helpers in `onMount`, calls `initialize`, and disposes lifecycle resources on teardown.

## Frontend layers

- `src/lib/*.svelte`: focused components and callbacks.
- `src/lib/*.ts`: native DTO/adapters and reusable pure presentation helpers.
- `src/routes/*-state.svelte.ts`: feature async/reactive controllers.
- `src/routes/page-state.svelte.ts`: cross-feature composition.
- `src/routes/+page.svelte`: visual tree and app-shell interactions.
- `src/lib/styles/`: feature CSS imported by the shell.

Put state at the narrowest owner: visual state in a component, operation state in a controller, cross-feature state in
`PageState`, durable state in Rust, and secrets/native session resources only in Rust.

## Native layers

- `lib.rs`: composition and narrow commands.
- `storage_commands/` and command modules: translate IPC to domain calls.
- `generation*`, `tool_*`, `microphone/`, `speech/`, `image_generation/`, `local_image_worker/`, `localmail/`: domain
  state machines and policy.
- `inference/`: provider discovery and protocol adapters.
- `provider_registry.rs`: selects a normalized provider route.
- `storage/`: SQLite, app-private content, migrations, recovery, backup, and exports.

Shared mutation uses Rust synchronization types. Blocking SQLite/file/model work must not block an async/WebView task.

## Chat flow

1. The composer changes transient state and derives availability from provider/model/capture/run state.
2. Page state persists a user turn and starts generation with a typed request and event channel.
3. Rust validates IDs, capabilities, attachments, tool choices, and concurrency.
4. The provider registry chooses an adapter; Rust builds context from storage without exposing paths.
5. The adapter streams normalized text, reasoning, usage, and tool events. Rust bounds and checkpoints durable state;
   the WebView receives display-safe events.
6. Tool requests pass through contract parsing, policy, dispatch, bounded rounds, and durable audit. Approved Python is
   exact and one-use.
7. Cancellation follows the native run identity through provider/tool work.
8. Completion/failure writes a terminal state; startup repairs interrupted runs.

Trace success, failure, cancellation, and restart. A stream that renders correctly but leaves durable `running` state
is defective.

## Providers and tools

`InferenceProvider` normalizes oMLX, Ollama, OpenAI-compatible, and Anthropic-compatible protocols. Capabilities—not
model-name guesses—control vision, audio, reasoning, and tools. Keep wire DTOs, pure normalization, and authenticated
network I/O separate so fixture tests do not need live services.

Tool schemas/validation live in `tool_contract/`, policy in `tool_policy.rs`, dispatch in `tool_dispatch.rs`, bounded
orchestration in `tool_loop.rs`, and persistence in storage tool/audit modules. Every tool needs a stable name, strict
arguments, output/deadline/round bounds, fixed errors, audit representation, and malformed-input tests.

## External runtimes

`python-runner/` and platform helpers implement a closed approved Python contract, not shell access.
`local-image-worker/` is reached through a versioned native protocol. Microphone/speech/image controllers keep opaque
session resources native. Extend these existing controllers rather than adding frontend shortcuts.
