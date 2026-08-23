# Migration rollback plan

Status: accepted implementation plan; no rollback code is included in this planning slice.

## Purpose

Bottie must not leave the only live conversation store partly upgraded when a schema migration, validation step, or
process fails. The implementation will migrate an isolated candidate, preserve a verified pre-migration database, and
promote the candidate only after every pending migration passes.

This plan protects Bottie's Rust-owned SQLite store while preserving the existing WebView boundary. It covers failed
forward migrations and interrupted candidate promotion. It does not promise that an older Bottie binary can read a
newer schema, and it does not introduce reverse SQL migrations.

## Current baseline

- `ConversationStore::initialize` opens the live database and applies each pending migration in its own immediate
  transaction. One failed migration rolls back its transaction, but earlier migrations from the same startup may
  already be committed.
- `user_version` and `schema_migrations` record the current schema. Version 17 also runs a Rust-owned deterministic
  backfill inside its migration transaction.
- Manual restore and corruption recovery already copy a source through SQLite's online backup API, migrate an isolated
  staging database, validate it, and retain a safety copy or damaged bundle before replacement.
- Attachment processing, semantic indexing, automatic backup rotation, retention, and attachment garbage collection
  begin only after healthy store initialization. They must remain outside the migration transaction.
- The live schema is version 21. SQLite, attachment paths, hashes, SQL, migration detail, and backup filenames remain
  native-only.

## Decisions

### 1. Forward-only schema contract

Migration source remains an ordered forward-only list. Each entry has a stable version and name plus one application
function that applies the SQL or bounded Rust backfill. Production code will not contain `down` SQL.

An older binary that encounters a newer `user_version` must stop without changing the database. Recovery from a bad
release means running a compatible Bottie build or restoring a pre-migration recovery point; it does not mean asking an
older binary to interpret or destructively rewrite a newer schema.

### 2. Classify data before writing a migration

Every migration must identify the data it touches:

- user-owned source data includes profiles, conversations, branches, messages, content blocks, provider runs, usage,
  tool audit records, ratings, attachment catalog metadata, associations, preferences, and retained extraction state;
- derived data includes FTS rows, deterministic chunks, vector mappings, and rebuildable progress metadata;
- application-private files include original attachment blobs, normalized derivatives, and the embedding-model cache.

A schema migration may transactionally transform SQLite source data. Derived data may be invalidated and rebuilt.
Schema migration code must not mutate application-private files; any required file conversion must be a separately
versioned, resumable post-start job. This keeps the unchanged attachment tree valid if the database is restored.

### 3. Preflight without mutation

Before opening a writable live connection, startup reads the existing database and classifies it:

- absent or empty: create the current schema directly;
- current: continue normal startup without a migration safety copy;
- older but supported: enter staged migration;
- newer: fail closed without creating files or changing SQLite state;
- corrupt or not a database: retain the existing restricted recovery path.

For an older supported store, preflight requires `quick_check = ok`, a `user_version` in the supported range, the
built-in profile and foundation tables, and a coherent migration ledger whose versions are unique, contiguous, and do
not exceed `user_version`.

### 4. Build and validate an isolated candidate

Startup creates a strictly named same-volume staging database through SQLite's online backup API so committed WAL
content is included. Only the staging database is opened for pending migrations.

Candidate initialization is narrower than normal application initialization. It applies schema migrations and
migration-owned backfills, then validates:

- `quick_check = ok` and no `foreign_key_check` rows;
- exact current `user_version`;
- one coherent `schema_migrations` row for every version through the current version;
- required foundation tables and the built-in local profile;
- current semantic schema/model metadata contract; and
- migration-specific source-data invariants declared by the migrations crossed in this run.

Interrupted provider-run recovery, Trash retention, attachment garbage collection, attachment processing, semantic
embedding work, automatic backup rotation, provider discovery, and WebView setup do not run against the candidate.

If copying, migration, or candidate validation fails, Bottie deletes only the strict staging database and its SQLite
sidecars. The live database and attachment tree remain unchanged, and startup stops with a stable path- and SQL-redacted
error.

### 5. Preserve a durable promotion recovery point

After the candidate validates, Bottie creates a second verified SQLite online copy of the still-unchanged live store in
an application-private `migration-backups` directory. This recovery point deliberately remains at the source schema
version and includes committed WAL content.

Migration recovery points do not embed or duplicate attachment blobs. Migrations are forbidden from mutating the
attachment tree, so the exact existing files remain paired with either database version. The two newest completed
recovery points are retained outside the seven-file automatic-backup rotation. Strict filename parsing prevents
unrecognized files from being pruned, and a recovery point named by an unfinished promotion marker is never pruned.

The copy must reopen read-only with `quick_check = ok`, its original `user_version`, coherent ledger, and source-data
row invariants before promotion can begin. Failure to create or verify it leaves the live database untouched.

### 6. Journal candidate promotion

Promotion uses a small native-only marker written atomically beside the live database. The marker contains only the
operation identifier, source and target schema versions, strict managed leaf filenames, and promotion phase. It never
contains user content, SQL, credentials, or an unrestricted path.

With all candidate and safety-copy connections closed, Bottie restores the validated candidate into the live database
through SQLite's backup API and validates the live destination again. The marker is removed only after live validation
passes and the database is safely closed.

On the next launch, marker reconciliation runs before ordinary corruption classification:

1. If the live database validates at the target version, promotion is complete; remove stale candidate files and the
   marker, then continue.
2. Otherwise, restore the marker's verified pre-migration recovery point, validate the source version, remove only the
   managed candidate files, retain the recovery point, and stop startup with a redacted migration failure.
3. If neither database validates, do not guess or delete either copy. Enter a migration-recovery failure that preserves
   all managed files for a compatible build or a later guided recovery flow.

This journal makes crashes before candidate creation, during migration, after safety-copy creation, during live
restore, and after successful restore but before cleanup deterministic.

### 7. Resume ordinary startup only after promotion

Once a migrated live store validates, ordinary initialization performs interrupted-run recovery and then retains the
established startup order: retention, attachment garbage collection, worker start/wake, and automatic backup rotation.
A path-free diagnostic may report source and target schema versions and whether interrupted promotion reconciliation
completed; it must not report filenames, paths, SQL, row values, or migration error detail.

No Tauri command or WebView migration API is required for the first implementation. A failed migration blocks normal
startup because the current binary cannot safely use the older schema, but the original data remains recoverable.

## Implementation slices

### Slice A — staged migration and promotion rollback

This is the next bounded implementation slice.

1. Separate migration-only candidate initialization from normal store startup.
2. Add read-only preflight and exact candidate/live validation helpers.
3. Add strict staging, safety-copy, and promotion-marker contracts with cleanup limited to those managed names.
4. Migrate the isolated candidate, verify the safety copy, journal promotion, reconcile interrupted promotion, and
   retain two completed migration recovery points.
5. Return only a stable startup error or path-free migration outcome; add no WebView command or screen.

No schema version bump is required to add this framework.

### Later work

- A guided migration-recovery screen may expose path-free recovery-point availability after the native failure model is
  proven. It must reuse the existing storage restriction and worker-pause boundaries.
- Release tooling may document compatible binary/schema ranges and offer an explicit user-selected recovery point.
- Any future file-format conversion must be designed as its own resumable job and may not weaken database rollback.

## Test and fault matrix

The implementation is not complete until path-backed tests prove:

- every retained historical schema fixture migrates to the current schema with `quick_check`, `foreign_key_check`,
  ledger, profile, and migration-specific invariants passing;
- a committed WAL-only row appears in the candidate, promoted store, and preserved recovery point as appropriate;
- injected failure while copying, in an early pending migration, in a later pending migration, and in candidate
  validation leaves the live database byte/logical state and attachment tree unchanged;
- safety-copy failure prevents promotion;
- failure during live restore returns the verified source schema and source-data rows;
- restart reconciliation handles every marker phase, including a valid promoted live store and a damaged live store;
- a newer schema, corrupt source, malformed ledger, invalid safety copy, and unmanaged lookalike files are never
  mutated or removed;
- promotion cleanup removes only exact managed staging files, retains the recovery point named by an active marker, and
  keeps the two newest completed points;
- source-data counts and identities remain stable unless the crossed migration explicitly declares a transformation;
- derived indexes may be rebuilt without deleting source content; and
- no path, SQL, attachment identity, content, credential, or native filename enters a serializable frontend type or
  diagnostic.

Use deterministic fault hooks at the storage layer in tests rather than malformed production migrations. Tests must
reopen the relevant databases in independent connections and inspect immutable copies where the live application store
would otherwise be changed.

## Acceptance gate for implementation

The rollback framework is accepted only when:

- no pending migration writes the live database before the candidate and safety copy validate;
- every promotion crash point either validates the target or restores the verified source without touching attachment
  files;
- unsupported newer schemas and corrupt stores preserve their current fail-closed/recovery behavior;
- all standard frontend and Rust checks pass, with no migration-specific capability added to Tauri; and
- a disposable copy of the real store passes schema, integrity, foreign-key, ledger, and source-row parity checks before
  any native launch is allowed to migrate the live store.

## Explicit exclusions

This plan does not implement rollback, reverse migrations, cross-version downgrade automation, release packaging,
update delivery, migration-recovery UI, backup settings, attachment file conversion, model-cache deletion, provider or
tool behavior, oMLX Web mapping, automatic memory injection, document opening, attachment retry controls, or a general
MCP runtime.
