# Storage and migrations

[Back to the manual index](index.md)

## Storage boundary

`ConversationStore` in `src-tauri/src/storage.rs` owns an app-private SQLite path, short-lived configured connections,
migrations, integrity verification, and interrupted-work recovery. App-private files contain attachments, generated
images, indexes, backups, and caches. The WebView receives application IDs and safe metadata—not paths or private bytes.
Validated custom protocols serve supported previews.

The schema begins with profiles, conversations, branches, messages, and ordered content blocks. Later migrations add
provider runs/usage, tool audit/approval, attachments and extraction/indexing, ratings/retention/memory, and generated
image lineage/recovery. History is a branch lineage, not a mutable list; edits/regeneration do not overwrite originals.
Read `storage/types.rs`, the relevant focused module, and tests before changing a table.

## Transactions and ordering

Use transactions for multi-row invariants and immediate behavior where writers must serialize. Use sequence/ordinal
columns for deterministic order; timestamps alone are insufficient. SQLite is synchronous, so large work must follow
existing `spawn_blocking`/background coordinator patterns. Return path-free domain errors.

Never edit an already released migration: installed databases have already applied it, and altered old SQL makes schema
shape depend on installation history.

## Adding a migration

1. Read `CURRENT_SCHEMA_VERSION`, the final migration, and migration dispatcher.
2. Add the next ordered constant with deterministic SQL, constraints, indexes, defaults, and transformations.
3. Increment the version and add it to the ordered application list.
4. Update migration validation/rollback and portable export/backup contracts as applicable.
5. Test fresh initialization and upgrade from the previous/relevant older schema.
6. Add deterministic injected-fault coverage for risky staging/file boundaries.
7. Verify data, indexes, foreign keys, integrity, and app behavior.
8. Review incident handling against `MIGRATION-ROLLBACK.md`.

If a table must change substantially, create a replacement, copy/validate data, drop the old table, and rename within
the transaction. Preserve user semantics explicitly rather than resetting preferences for convenience.

## Files and lifecycle

Attachment ingest is native and bounded: identify, copy/normalize/extract, associate, deliver/export, remove, then
garbage-collect. Adding a format requires MIME detection, parser safety, count/byte bounds, preview CSP/protocol,
provider delivery, backup/export, branch/removal/forget, and garbage-collection review.

Generated files similarly require durable run/source/lineage state and interruption-safe file promotion/database order.
Orphan rows or files are privacy and disk-use defects.

Distinguish:

- **backup**: verified restorable store snapshot;
- **portable export**: versioned user interchange bundle;
- **Markdown/JSON export**: selected conversation representation;
- **retention**: lifecycle policy;
- **Trash/restore/forget**: explicit transitions, with permanent cleanup on forget;
- **recovery**: corruption/interruption handling and backup restoration.

A durable field is incomplete until backup, restore, exports, branch behavior, retention, recovery, and deletion are
explicitly decided and tested.

## Test matrix

Cover applicable fresh/upgrade initialization, invalid constraints, injected rollback, busy/concurrent access,
deterministic branch order, lifecycle/forget, export or backup round trip, interrupted promotion/run recovery, garbage
collection, and path/secret-free command serialization. Performance budgets are separate ignored tests run by
`npm run performance:native`.
