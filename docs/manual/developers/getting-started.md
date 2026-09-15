# Getting started

[Back to the manual index](index.md)

## Prerequisites

Install a current Node.js/npm, current Rust via `rustup` (including Cargo and rustfmt), Git, and the operating-system
packages listed by Tauri 2. A real chat additionally needs oMLX, Ollama, or an explicitly configured OpenAI- or
Anthropic-compatible endpoint. Linux native builds need the WebKit/audio development libraries used by Tauri and the
native dependencies; packaged speech uses Speech Dispatcher.

```sh
node --version
npm --version
rustc --version
cargo --version
npm install
cargo check --manifest-path src-tauri/Cargo.toml
```

Use the explicit Cargo manifest: the repository root is **not** a Cargo workspace. `npm install` reproduces the lockfile;
do not casually upgrade dependencies while setting up.

## Run modes

### Full desktop

```sh
npm run tauri dev
```

This starts Vite, compiles Rust, and opens a Tauri window. Use it for IPC, SQLite, vault, provider, file, audio, and
other native behavior. The first compilation can take several minutes.

### Browser-only UI

```sh
npm run dev
```

Use the printed Vite URL for layout and deterministic preview fixtures. Tauri commands do not work in a browser.
Browser preview cannot prove native behavior and must never fake production success.

### Production-shaped frontend

```sh
npm run build
npm run preview
```

The static adapter builds a single-page app. SSR is disabled because packaged Tauri has no Node server.

## Provider setup

Configure providers in Settings. Local providers must be loopback. Cloud routes must be explicit, and secrets go to
the OS credential vault—not TypeScript, browser storage, fixtures, `.env`, logs, or Git. Default tests use fakes/local
servers. Live-provider tests are opt-in and documented in `README.md`.

## Daily loop

1. Read `HANDOVER.md`, `ROADMAP.md`, `CONTRIBUTING.md`, and the relevant implementation.
2. Start from a failing focused test when behavior is testable.
3. Implement the smallest complete vertical slice.
4. Run focused tests, then all relevant required checks.
5. Exercise the actual Tauri flow when native behavior changed.
6. Inspect the complete diff and explicitly staged files before committing.

```sh
npx vitest run src/lib/storage.test.ts
cargo test --manifest-path src-tauri/Cargo.toml storage::
npx prettier --write src/lib/storage.ts
cargo fmt --manifest-path src-tauri/Cargo.toml
```

Recommended editor services are rust-analyzer, Svelte, and TypeScript. Use repository Prettier and rustfmt.

## Common mistakes

- Running bare Cargo commands at the root instead of supplying the manifest.
- Treating browser preview as a native integration test.
- Putting an API key or durable private state in the WebView.
- Returning paths, bytes, native IDs, or raw upstream errors for convenience.
- Adding a Rust public item without documentation; the crate denies missing docs.
- Editing generated inventory/notices rather than using their scripts.
- Updating only one side of a command DTO.
- Claiming another operating system works based on the current host.
