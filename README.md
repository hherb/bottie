# bottie

Bottie is a local-first desktop chatbot built with Tauri 2, Rust, Svelte, and TypeScript. It is designed to connect to oMLX, Ollama, Anthropic-compatible, and OpenAI-compatible inference providers while keeping application secrets, files, tools, and persistent memory behind the Rust boundary.

The current developer preview pairs the interactive product shell with real, text-only inference through oMLX,
Ollama, OpenAI-compatible, and Anthropic-compatible providers. The Rust core validates provider endpoints, discovers
models, tests connections, streams normalized answer and reasoning events over a typed Tauri IPC channel, and owns
end-to-end cancellation. Conversations and their ordered text/reasoning messages persist in a Rust-owned bundled
SQLite database and reopen after restart. Accepted provider runs retain their model, generation settings, terminal
state, elapsed time, provider-reported token/cost usage, and checkpointed partial output. If Bottie exits during a
generation, its next launch marks that run interrupted and reopens the response with the text and reasoning already
saved. Remote API keys stay in the operating-system credential vault and are never returned to the WebView. On macOS,
Touch ID gates the first read of each saved cloud credential
per Bottie session; successful unlocks are cached only in process memory. Attachments, memory retrieval, and tools are
not implemented yet; those UI surfaces are disabled or labelled as preview-only fixtures.

## Development

Prerequisites:

- A current Rust toolchain
- Node.js and npm
- The platform prerequisites required by Tauri 2

Install and verify:

```sh
npm install
npm run format:check
npm run check
npm test
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

See `CONTRIBUTING.md` for the mandatory documentation, file-size, line-length, pure-function, named-constant, TDD, and
slice-documentation rules.

Run the native desktop application:

```sh
npm run tauri dev
```

With oMLX or Ollama running on its default loopback port, the native app discovers available models automatically. Provider and model use separate selectors; changing providers refreshes that provider's models, and the last successful pair is restored after restart. Settings can change either endpoint and test it before saving. Rust rejects non-loopback hosts, embedded credentials, paths, query strings, and fragments; redirects are disabled, and no HTTP capability is exposed to the WebView. Ollama discovery also normalizes model capabilities, context size, and loaded/on-demand state.

Settings also support HTTPS OpenAI-compatible and Anthropic-compatible profiles. API keys are written and removed
through narrow Rust commands backed by the OS credential vault. Cloud routes are visibly labelled before sending,
redirects stay disabled, and remote response usage and provider-reported cost metadata are preserved when available.

Thinking/reasoning defaults to off and can be toggled to low effort for each request. Reasoning-capable providers stream
that material into a collapsed, user-expandable section rather than mixing it into the answer. Native generation also
applies a 4,096-token default ceiling when the interface does not provide a tighter limit.

The first submitted prompt creates a durable conversation for the built-in local profile. User messages commit before
provider inference begins. Rust creates an empty assistant checkpoint with each accepted run, appends streamed text,
reasoning, and usage before forwarding those events to the interface, and commits terminal state before the next
prompt can be sent. On startup, any run left active by a prior process becomes an interrupted partial response. The
sidebar groups real conversation activity by local calendar date. Conversations can be renamed inline, archived, moved
to recoverable trash, and restored without losing messages. The initial SQLite schema models conversations, main
branches, ordered messages, separate text/reasoning blocks, provider runs, and append-only usage snapshots. Reopened
assistant responses recover provider-reported token/cost metadata without estimating missing values. Branching, exact
last-open-conversation restoration, search, export, and backup/restore remain planned Milestone 2 work.

Run the layout-only browser preview:

```sh
npm run dev
```

The browser preview deliberately disables sending and reports that native inference is unavailable.

Four ignored tests exercise streaming and cancellation against live local providers:

```sh
cargo test --manifest-path src-tauri/Cargo.toml live_omlx_stream -- --ignored --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml live_ollama_stream -- --ignored --test-threads=1
```

## Planned architecture

The WebView owns presentation state. The Rust application core will own:

- provider configuration, credentials, and streaming inference;
- conversation and attachment persistence;
- SQLite FTS5 and vector memory retrieval;
- file extraction and normalization;
- web search, fetch, and other model tools;
- privacy policy enforcement and audit data.

The provider layer normalizes OpenAI, Anthropic, Ollama, and oMLX responses into a capability-aware internal event
stream rather than assuming every compatible endpoint behaves identically.

For the planned memory subsystem, bottie will use FastEmbed inside Rust with the quantized EmbeddingGemma 300M model as its single built-in embedding default. The application will own model download/cache UX and embedding-version metadata; users will not need to configure a second inference provider merely to enable local memory search.
