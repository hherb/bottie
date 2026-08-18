# bottie

Bottie is a local-first desktop chatbot built with Tauri 2, Rust, Svelte, and TypeScript. It is designed to connect to oMLX, Ollama, Anthropic-compatible, and OpenAI-compatible inference providers while keeping application secrets, files, tools, and persistent memory behind the Rust boundary.

The current developer preview pairs the interactive product shell with real, text-only local inference through oMLX. The Rust core discovers models from `127.0.0.1:8000`, streams normalized response events over a typed Tauri IPC channel, and owns end-to-end cancellation. Persistence, attachments, memory retrieval, tools, and remote providers are intentionally not implemented yet; those UI surfaces are disabled or labelled as preview-only fixtures.

## Development

Prerequisites:

- A current Rust toolchain
- Node.js and npm
- The platform prerequisites required by Tauri 2

Install and verify:

```sh
npm install
npm run check
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

Run the native desktop application:

```sh
npm run tauri dev
```

With oMLX running on its default loopback port, the native app discovers available models automatically. The endpoint is fixed inside Rust for this slice and is never supplied by the WebView.

Run the layout-only browser preview:

```sh
npm run dev
```

The browser preview deliberately disables sending and reports that native inference is unavailable.

Two ignored tests exercise the adapter against a live local oMLX instance:

```sh
cargo test --manifest-path src-tauri/Cargo.toml live_omlx_stream -- --ignored --test-threads=1
```

## Planned architecture

The WebView owns presentation state. The Rust application core will own:

- provider configuration, credentials, and streaming inference;
- conversation and attachment persistence;
- SQLite FTS5 and vector memory retrieval;
- file extraction and normalization;
- web search, fetch, and other model tools;
- privacy policy enforcement and audit data.

The provider layer will normalize OpenAI, Anthropic, Ollama, and oMLX responses into a capability-aware internal event stream rather than assuming every compatible endpoint behaves identically.
