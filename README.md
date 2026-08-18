# bottie

Bottie is a local-first desktop chatbot built with Tauri 2, Rust, Svelte, and TypeScript. It is designed to connect to oMLX, Ollama, Anthropic-compatible, and OpenAI-compatible inference providers while keeping application secrets, files, tools, and persistent memory behind the Rust boundary.

The current first slice is an interactive product shell. It includes the conversation layout, context inspector, attachment selection, memory and privacy affordances, responsive navigation, and a simulated tool-and-token stream. Provider calls and persistence are intentionally not implemented yet.

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
```

Run the native desktop application:

```sh
npm run tauri dev
```

Run the layout-only browser preview:

```sh
npm run dev
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
