# Bottie programmer's manual

This manual is the starting point for developers who maintain Bottie. It assumes basic C# knowledge, but no prior
Rust, TypeScript, Svelte, Tauri, SQLite, or AI-provider experience. Bottie contains no C#: C# comparisons below are
only a bridge to unfamiliar concepts.

## What Bottie is

Bottie is a local-first desktop chatbot. A Svelte/TypeScript interface runs in a Tauri WebView, while Rust owns
credentials, network access, files, audio, tools, and SQLite state. Supporting Rust, Python, and Node programs build
protected Python and image-worker runtimes and package the app.

The main rule is that the WebView is **not trusted with secrets or native resources**. Treat every Tauri command like a
public web API: validate in Rust, return a small typed and path-free result, and test failures.

## Reading order

1. [Getting started](getting-started.md) — prerequisites, running the app, and daily commands.
2. [Architecture and request flow](architecture.md) — processes, responsibilities, and feature flows.
3. [Frontend development](frontend.md) — TypeScript, Svelte 5, UI state, IPC, and accessibility.
4. [Native Rust development](native-rust.md) — commands, providers, tools, workers, and cancellation.
5. [Storage and migrations](storage.md) — durable data, files, backups, and recovery.
6. [Security and privacy](security.md) — invariants and a review checklist.
7. [Testing and change workflow](testing-and-workflow.md) — building and reviewing a vertical slice.
8. [Operations and troubleshooting](operations.md) — common failures, dependencies, packaging, and releases.

Also read the narrower authoritative references when relevant:

- [`CONTRIBUTING.md`](../../../CONTRIBUTING.md): coding rules and required checks.
- [`HANDOVER.md`](../../../HANDOVER.md): current implementation state and next work.
- [`ROADMAP.md`](../../../ROADMAP.md): principles, milestones, and definition of done.
- [`README.md`](../../../README.md): user-visible behavior and provider support.
- [`docs/python-sandbox.md`](../../python-sandbox.md): protected Python execution.
- [`docs/local-image-model-package.md`](../../local-image-model-package.md): local image packaging.
- [`MIGRATION-ROLLBACK.md`](../../../MIGRATION-ROLLBACK.md): migration incident runbook.

## C# translation guide

| C# idea | Bottie equivalent |
| --- | --- |
| `.sln` / `.csproj` | `package.json` for UI; `src-tauri/Cargo.toml` for native code |
| NuGet | npm for frontend; Cargo/crates.io for Rust |
| `dotnet build` | `npm run build` and Cargo `check` |
| xUnit/NUnit | Vitest and Rust `#[test]` / `#[tokio::test]` |
| ASP.NET controller action | A registered `#[tauri::command]` |
| DTO with JSON attributes | TypeScript interface plus Serde Rust struct |
| `Task<T>` | TypeScript `Promise<T>`; Rust `async fn` returning `Result<T, E>` |
| nullable reference | TypeScript `T | null`; Rust `Option<T>` |
| exception | rejected promise; Rust `Result<T, E>` |
| `lock` / `SemaphoreSlim` | Rust `Mutex`, `RwLock`, and async mutexes |
| EF migration | Ordered SQL constants applied by Rust |

## Repository map

| Path | Responsibility |
| --- | --- |
| `src/routes/` | App shell and page-level reactive controllers |
| `src/lib/` | Svelte components, native adapters, pure helpers, tests, and styles |
| `src-tauri/src/` | Native app, providers, storage, tools, audio, and workers |
| `src-tauri/tests/` | Native integration tests |
| `scripts/` | Build, evidence, package, dependency, icon, and release tooling |
| `python-runner/` | Standalone protected Python runner (Rust) |
| `local-image-worker/` | Python local image worker and package metadata |
| `macos-python-xpc/`, `windows-python-appcontainer/` | Platform containment helpers |
| `static/` | Files copied into the frontend bundle |
| `distribution/` | Packaging inputs |
| `.github/workflows/` | Platform validation and publication workflows |
| `runtime-assets.json` | Pinned runtime asset identities and hashes |

Before editing, find a neighboring implementation with a test. Extend an existing pattern rather than inventing a
parallel abstraction. A command contract change normally requires Rust, TypeScript, tests, and docs in the same slice.
