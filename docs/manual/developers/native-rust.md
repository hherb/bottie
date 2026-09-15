# Native Rust development

[Back to the manual index](index.md)

## Rust essentials for C# developers

Rust ownership decides who releases values; borrowing (`&T`, `&mut T`) grants temporary access. The compiler prevents
data races and use-after-free. `struct` resembles a data record/class, `trait` an interface, and data-carrying `enum`
a discriminated union. `Option<T>` is an explicit maybe-value. `Result<T, E>` is explicit success/failure, and `?`
returns errors after conversion. `Arc<T>` gives thread-safe shared ownership; locks control mutation.

The crate uses Rust edition 2024 and `#![deny(missing_docs)]`. Document modules/items with `//!`/`///`, explaining
policy, units, bounds, ownership, and failure—not repeating syntax. Prefer the smallest visibility (`pub(crate)` rather
than `pub`) and exhaustive `match` handling.

## Adding a Tauri command

```rust
/// Path-free state returned to the WebView.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WidgetStatus {
    /// Stable presentation state.
    state: &'static str,
}

#[tauri::command]
/// Reads bounded state without exposing its native handle.
async fn get_widget_status(state: tauri::State<'_, AppState>) -> Result<WidgetStatus, WidgetError> {
    state.widget.status().await
}
```

Register the function in `tauri::generate_handler!` in `lib.rs`, add to `AppState` only for a long-lived dependency,
add the TypeScript adapter, and test both sides.

Review every command:

1. Deserialize a narrow type; reject unknown authority/secret-bearing fields where appropriate.
2. Validate lengths, counts, IDs, state, origins, and capabilities before side effects.
3. Return stable typed errors, sanitizing internal OS/database/provider details.
4. Exclude secrets, paths, raw bytes, handles, native IDs, and unbounded text.
5. Move blocking SQLite/filesystem/CPU work to `spawn_blocking` or an existing worker.
6. Specify concurrency, cancellation, terminal cleanup, and stale-result behavior.
7. Verify JavaScript input/output casing and command registration.
8. Test success, exact bounds, malformed input, failures, and races.

Never expose generic file, HTTP, SQL, shell, process, audio, or updater access.

## Errors, bounds, and durable side effects

Validate before side effects. Use domain error variants and convert library errors at the boundary. Avoid
`unwrap`/`expect` in production unless a local invariant truly proves safety. Give behavioral numbers named constants
with units (`MAX_*_BYTES`, `*_TIMEOUT`) and apply bounds before collecting external data.

Use transactions or staged promotion for multi-step durability. Failures must leave a state understood by startup
recovery. User-visible errors and diagnostics may not leak paths, URLs with credentials, headers, SQL, or raw upstream
messages.

## Async and cancellation

Provider/channel work is async; `rusqlite`, many filesystem/platform calls, and model work block. Do not hold a
synchronous lock guard across `.await`. Keep lock scope small and document ordering when multiple locks are unavoidable.

Dropping a frontend promise is not cancellation. Long operations need a native identity, overlap policy, cancellation
checks, idempotent terminal cleanup, stale-session protection, and durable terminal outcome where applicable. Test
cancellation before start, during work, and racing completion.

## Providers, networks, and workers

Provider adapters are under `inference/`. Keep provider-neutral types and separate wire DTO mapping from network I/O.
Local endpoints remain loopback-only; cloud origins, credentials, redirects/proxies, deadlines, and response bounds are
native policy. Streaming parsers must tolerate valid chunking, reject malformed/oversized events, assemble incremental
tool calls, and preserve terminal/usage state.

Use `#[cfg]` only for genuinely platform-specific code and keep a common contract. Worker protocols require version,
package/executable identity, frame size, deadline, ordering, and exit validation. Child stdout is untrusted. Read the
Python/image specialist docs before those changes.

## Tests

Unit/domain tests sit near modules; larger test modules are included under `#[cfg(test)]` in `lib.rs`; black-box tests
live in `src-tauri/tests/`. Prefer fake providers/clocks/transports, temporary stores, and local servers. Default tests
must be offline, deterministic, and credential-free.

```sh
cargo test --manifest-path src-tauri/Cargo.toml test_name_fragment -- --nocapture
```
