# bottie handover

Last verified: 2026-08-20

## Start here

Bottie is a greenfield Tauri 2 desktop chatbot. Milestone 1 is complete: the native app supports oMLX, Ollama,
OpenAI-compatible, and Anthropic-compatible text inference through Rust-owned networking, credentials, streaming, and
cancellation. Milestone 2 is complete: a Rust-owned bundled SQLite store now persists local-profile conversations
and ordered text/reasoning messages across restart. Accepted provider runs now persist their request link, provider,
model, generation settings, terminal outcome, timing, provider-reported usage, and crash-safe partial text/reasoning
checkpoints. Runs left active by an earlier process reopen as visibly interrupted partial responses. Users can rename,
archive, soft-delete, restore, and browse real conversations in calendar-date groups. The exact last-open conversation,
including an intentional blank new-chat view, now survives restart. Editing a user prompt or regenerating an assistant
response creates a selected alternative branch while preserving every prior lineage for switching. Provider selection
remains explicit, cloud routes are visible before sending, and credential-vault values are never returned to the
WebView. Native conversation search now finds titles and visible message text across active and archived histories and
opens the preserved branch containing each result. Assistant answers now render parser-owned Markdown while raw HTML,
unsafe destinations, and remote image fetches stay inert. Non-empty assistant answers can now be copied as their exact
Markdown source when reasoning is absent. When separate reasoning exists, the copied Markdown contains labelled
Reasoning and Response sections without parser-generated HTML or response metadata. Interrupted, cancelled, and
transiently failed responses now expose a labelled retry action that forks the unchanged request while preserving the
original attempt. Assistant responses can now retain a local Good or Poor rating across restart and branch switching;
selecting the active choice clears it without changing response content. The selected visible conversation lineage can
now be exported as either human-readable Markdown or versioned, machine-readable JSON through Rust-owned native Save
dialogs without revealing the chosen path to the WebView. A separate global action exports every active and archived
conversation's selected lineage as one versioned JSON document while excluding Trash and hidden branch siblings. Users
can also create a complete verified SQLite snapshot
through a separate Rust-owned Save dialog; SQLite's online backup API includes committed WAL content without pausing
the live store, and the destination path remains native-only. A separate native Open-and-confirm flow now restores
validated Bottie backups only after
creating an application-private snapshot of the current store; selected directories and database paths never reach the
WebView. After a successful startup, Bottie now creates a verified application-private snapshot when no automatic
backup is newer than 24 hours and retains the seven newest automatic snapshots. Rotation runs in the background, never
prunes manual backups or pre-restore safety copies, and reports a path-redacted outcome in session diagnostics. If
SQLite reports corruption at startup, Bottie now opens in a restricted recovery state instead of aborting launch. The
guided screen can restore the newest verified automatic snapshot or a manually selected Bottie backup after preserving
the damaged database bundle in app-private storage. Native provider runs now also retain ordered structured tool calls
and one append-only result per call; reopened tool activity is inspectable and portable without exposing native or
provider call identities. Provider tool loops and execution remain absent. The next bounded implementation slice is
native content-addressed attachment ingestion with MIME sniffing, hashes, size limits, duplicate detection, and safe
display names; do not bundle extraction, indexing, provider delivery, memory search, or broad visual-design work with
it.

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
- durable conversation creation on first send, recent-conversation navigation, and exact last-open restoration;
- crash-safe partial answer/reasoning checkpoints and visibly interrupted-run recovery;
- real Today, Yesterday, Previous 7 days, Archived, and Trash navigation groups;
- inline conversation rename plus archive, unarchive, recoverable trash, and restore actions;
- inline user-message editing, assistant-response regeneration, and preserved branch switching;
- native conversation search with snippets, archived-result labels, matching-branch selection, and keyboard focus and
  clear behavior;
- sanitized assistant Markdown with headings, lists, tables, quotes, safe external links, and code presentation;
- assistant-response and reasoning copying as labelled Markdown with visible and screen-reader-readable feedback;
- response retry for interrupted, cancelled, and retryable failed attempts, preserving the original branch;
- durable Good/Poor response ratings with accessible pressed state, replacement, and clearing;
- expandable persisted tool-call arguments and pending/success/error results on reopened assistant responses;
- selected-lineage Markdown/JSON and non-trashed batch JSON export through native Save dialogs with compact feedback;
- complete manual SQLite backup creation through a native Save dialog with compact saved/error feedback;
- confirmed manual restore from validated Bottie backups with a named pre-restore safety copy;
- verified daily automatic SQLite snapshots with a seven-snapshot app-private retention policy;
- corruption-aware startup with guided automatic/manual restore and app-private damaged-data preservation;
- a provider settings dialog with endpoint editing, OS-vault credential management, connection tests, timeout policy,
  and secret/path-redacted session diagnostics including automatic-backup outcomes;
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
`src-tauri/src/storage/selection.rs` owns profile-scoped last-open state, `src-tauri/src/storage/branching.rs` owns
branch creation and selection, `src-tauri/src/storage/search.rs` owns bounded conversation search,
`src-tauri/src/storage/ratings.rs` owns response-rating validation and mutation, `src-tauri/src/storage/tools.rs` owns
bounded append-only tool-call/result records, `src-tauri/src/storage/export.rs` owns
deterministic selected-lineage Markdown plus selected and batch JSON rendering and safe suggested filenames, and
`src-tauri/src/storage/backup.rs` owns
consistent online SQLite snapshots, strict automatic-backup discovery and rotation, restore validation, isolated
migration, pre-restore
safety copies, and post-copy integrity checks, `src-tauri/src/storage/recovery.rs` owns read-only startup corruption
classification, verified automatic recovery-point discovery, restricted store state, damaged-bundle preservation, and
staged replacement, and
`src-tauri/src/generation.rs` closes each native run before its terminal stream event reaches the WebView.
`src-tauri/src/storage_commands.rs` exposes only list/search, create, selected-load/clear, user-message append,
explicit branch, response-rating, selected-lineage Markdown/JSON and non-trashed batch JSON export, whole-store
backup/restore,
recovery-status/latest-snapshot restore, and lifecycle commands. The database lives in the OS application-data
directory; the WebView never receives a path, SQL, or generic database
capability. One built-in `local` profile represents the current OS account. Every conversation has
a selected branch, and every message stores a branch-local append sequence plus independently ordered text/reasoning
blocks. User prompts commit before inference starts, and terminal assistant responses commit before another prompt can
append. Rust creates each assistant response with its run, checkpoints every provider text/reasoning delta before IPC,
and marks leftover running records interrupted during the next startup. Assistant responses reference opaque native
provider runs, and reopened conversations reconstruct real elapsed time plus provider-reported token/cost usage without
estimating missing values. Creating or opening a conversation records it as the local profile's exact selection;
starting a blank chat clears that selection, and archiving or deleting the selected conversation clears it in the same
transaction as the lifecycle change. Editing, regenerating, or retrying creates one new branch whose first request
points to the visible predecessor from the selected lineage; switching branches reconstructs ancestry through
native-owned parent message links without copying or deleting the original history.

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
- provider adapters and orchestration do not yet emit or execute the persisted tool records; browser-preview tool
  activity remains a fixture;
- reasoning-toggle state is session-only and resets to off when the app restarts;
- SQLite conversation storage exists, but no FTS5 or vector extension exists yet;
- no web search or fetch tool exists;
- there are no automated component or end-to-end UI tests yet; pure presentation and Markdown-policy helpers have
  frontend unit coverage.

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

## Most recently completed product slice: Append-oriented tool activity persistence

### Goal

Retain provider-emitted tool calls and their outcomes as durable, structured, inspectable conversation provenance
without implementing a tool loop, executing a tool, exposing mutation commands to the WebView, or leaking opaque call
identities.

### Implemented shape

1. Schema version 7 adds immutable ordered `tool_invocations` under native provider runs and at most one append-only
   `tool_results` row per call. Foreign keys cascade with the owning run while unique constraints prevent duplicate
   provider call identities, ordinals, or outcomes.
2. Rust accepts calls/results only while the owning run is active, trims and bounds provider-controlled identities,
   requires argument objects, caps each serialized JSON payload at 1 MiB, and reconstructs resolved or pending calls
   in provider order after restart and branch switching.
3. Reopened assistant responses expose an expandable read-only Tool activity panel. It renders arguments and
   pending/success/error outcomes as inert text with no execution, approval, retry, web, provider, or filesystem
   capability in JavaScript.
4. Selected and batch JSON export contracts advance to version 2 and include structured tool activity without native
   run or provider call IDs. Markdown exports add explicit tool sections using dynamic safe fences for embedded
   backticks.

### Acceptance criteria

- Existing version-6 stores migrate transactionally to version 7 without rewriting messages, branches, runs, ratings,
  backups, or selection.
- Ordered calls, unresolved calls, results, and error results survive restart; duplicates, malformed linkage,
  non-object arguments, oversized payloads, and writes after terminal run state are rejected natively.
- Tool records follow their provider response through selected-branch reconstruction and portable export while opaque
  database/run/provider-call identities remain absent from the WebView and files.
- No provider protocol, streaming event, tool loop, execution, approval, web access, attachment, import, or credential
  behavior is added.

### Verification completed

The standard frontend and Rust checks passed on 2026-08-20. The frontend suite has thirty passing tests,
`svelte-check` reports no errors or warnings, and the production build succeeds. The Rust suite has ninety-two tests:
eighty-eight pass by default and four live-provider tests remain opt-in. Four focused tool tests cover schema-6
migration, ordered call/result restart reconstruction, unresolved calls, validation, linkage, duplicates, and terminal
run rejection; nine export tests cover version-2 JSON and Markdown tool records without opaque identities.

The expanded read-only panel was visually checked in the browser preview at 1320 x 820 and 420 x 780. Both viewports
kept the document and JSON payloads within their containers with no horizontal overflow, and the browser console was
clean. A fresh native process migrated the existing application store from schema 6 to 7 and remained open without
console errors; read-only inspection confirmed both new tables were empty, the migration record was present, and
`quick_check` returned `ok`.

## Prior completed product slice: Batch conversation JSON export

### Goal

Let users save every active and archived conversation's selected lineage as one portable machine-readable document
without exposing filesystem paths, exporting recoverable Trash, or leaking hidden branch siblings and opaque storage
identifiers.

### Implemented shape

1. A read transaction reconstructs all non-deleted local-profile conversations in deterministic active-then-archived,
   recent-first order while preserving each conversation's selected lineage.
2. A pure Rust renderer emits pretty UTF-8 JSON with the stable `bottie-conversation-batch` discriminator and version
   1. Each item retains its title, active/archived lifecycle, activity time, and the same portable message and provider
   metadata as single-conversation JSON.
3. Trash, hidden branch siblings, conversation/branch/message/run IDs, directories, and SQL remain excluded. Preparing
   the batch does not change the last-open conversation, lifecycle, activity, ratings, branches, messages, or schema.
4. One global labelled toolbar action opens a JSON-filtered native Save dialog for `bottie-conversations.json` even
   when no conversation is open. Rust prepares the potentially larger document on a blocking worker and writes it;
   the WebView receives only `saved` or `cancelled` plus a leaf filename.

### Acceptance criteria

- The batch parses as version 1 of the Bottie batch contract and includes active and archived selected lineages in a
  deterministic order.
- Trash, hidden siblings, and opaque native identifiers are absent while exact content, reasoning, state, rating,
  timestamps, provenance, and provider-reported usage remain machine-readable.
- No eligible conversations disables the action and is also rejected at the native boundary.
- Cancellation remains neutral and filesystem/serialization failures use the existing path-redacted export error.
- No migration, import, provider, credential, backup, attachment, tool persistence/execution, or branch behavior is
  added.

### Verification completed

The standard frontend and Rust checks passed on 2026-08-20. The frontend suite has twenty-nine passing tests,
`svelte-check` reports no errors or warnings, and the production build succeeds. The Rust suite has eighty-seven tests:
eighty-three pass by default and four live-provider tests remain opt-in. Eight focused export tests now cover selected
Markdown/JSON compatibility plus batch ordering, lifecycle, selected-lineage isolation, Trash/opaque-ID omission,
empty-batch rejection, selection stability, safe filenames, and native UTF-8 writing.

The browser preview was visually checked at 1320 x 820 and 420 x 780; the global batch action remained labelled,
visible, and contained without horizontal toolbar overflow. A fresh native process compiled, registered the command,
opened against the existing healthy store, and remained running. macOS denied synthetic accessibility inspection, so
the native Save dialog was not clicked; real SQLite-backed batch preparation and native-path writing remain covered by
the path-backed Rust tests.

## Prior completed product slice: Selected-conversation JSON export

### Goal

Let users save the selected visible conversation lineage as a portable, machine-readable document without exposing
filesystem paths, leaking opaque storage identifiers, or expanding into multi-conversation export or import.

### Implemented shape

1. A pure Rust renderer emits deterministic pretty UTF-8 JSON with the stable `bottie-conversation` discriminator and
   an export-contract version independent from SQLite schema versions.
2. Ordered selected-lineage messages retain exact text and separate reasoning, durable state, provider/model
   provenance, local rating, creation time, and provider-run outcome, reasoning setting, timing, error category, and
   provider-reported usage when available. Conversation, branch, message, and provider-run IDs are omitted.
3. JSON export uses the same reconstructed selected-branch policy and bounded cross-platform filename logic as
   Markdown export. It does not change selection, activity timestamps, branches, messages, ratings, or schema state.
4. A narrow native command owns the JSON-filtered Save dialog and UTF-8 write. The WebView receives only `saved` or
   `cancelled` plus a leaf filename, while a distinct labelled toolbar action shares the existing compact feedback.

### Acceptance criteria

- The exported JSON parses as version 1 of the Bottie conversation contract and contains only the selected lineage.
- Text, separate reasoning, incomplete state, provenance, rating, timestamps, and provider-reported metadata remain
  machine-readable without opaque database or run identifiers.
- User-controlled titles yield bounded `bottie-*.json` suggestions and cancelling the dialog remains neutral.
- Filesystem and serialization failures use the existing path-redacted export error; no directory reaches JavaScript.
- No migration, batch export, import, provider, credential, backup, attachment, or tool behavior is added.

### Verification completed

The standard frontend and Rust checks passed on 2026-08-20. The frontend suite has twenty-eight passing tests,
`svelte-check` reports no errors or warnings, and the production build succeeds. The Rust suite has eighty-five tests:
eighty-one pass by default and four live-provider tests remain opt-in. Six focused export tests cover deterministic
Markdown compatibility, the versioned JSON contract and metadata, opaque-ID omission, selected-lineage isolation,
safe filenames, selection stability, and native UTF-8 writing.

A fresh native process compiled and remained open against the existing healthy store. The browser preview was visually
checked at 1320 x 820 and 420 x 780; the separate Markdown and JSON actions stayed labelled and contained in both
toolbars. macOS did not expose the debug window to System Events in this launch context, so the native Save dialog was
not synthetically clicked. The real selected-lineage preparation and native-path UTF-8 write remain covered by the
path-backed Rust tests.

## Prior completed product slice: Corruption detection and guided recovery

### Goal

Keep Bottie usable when SQLite identifies a corrupt live conversation store, while preventing further conversation
mutation and providing a path-redacted route back to verified data.

### Implemented shape

1. Startup opens an existing database read-only and runs SQLite's bounded `quick_check` before migration. Explicit
   `SQLITE_CORRUPT`, `SQLITE_NOTADB`, or non-`ok` integrity results create a restricted native store instead of aborting
   Tauri setup. Other initialization failures remain fatal rather than being mislabeled as corruption.
2. Every normal conversation connection is guarded by the shared restricted-state flag, and automatic rotation does not
   run against damaged data. The WebView receives only `ready` or `recovery_required`, the count of verified managed
   snapshots, and the newest snapshot timestamp—never a database path or automatic-backup filename.
3. A dedicated recovery screen pauses the normal shell and offers the newest verified automatic snapshot plus a manual
   Bottie-backup picker. Both flows retain native confirmation; manual candidates still pass the existing schema,
   profile, integrity, and isolated-migration checks.
4. Before replacement, Rust moves the exact damaged database, WAL, and shared-memory files that exist into one unique
   app-private preservation directory. A migrated verified replacement is prepared independently before that move and
   installed by same-directory rename; failed installation rolls the preserved bundle back.
5. Successful recovery clears the shared restriction, reloads navigation and exact selection, and resumes model
   discovery. The WebView receives only a human-readable source label and preservation-directory leaf name.

### Acceptance criteria

- Corrupt or non-database live bytes no longer prevent the native app from opening to a recovery action.
- Conversation, generation, backup, export, and lifecycle access remains paused until a verified restore succeeds.
- Only strictly named automatic snapshots that independently pass Bottie restore validation are counted or selected.
- The newest verified automatic snapshot and a valid manual backup both use the same staged replacement policy.
- Damaged main/WAL/shared-memory files are preserved before replacement, and no path or SQL detail crosses IPC.
- Healthy startup, existing manual restore, provider networking, credentials, migrations, and automatic retention keep
  their existing behavior.

### Verification completed

The standard frontend and Rust checks passed on 2026-08-20. The frontend suite has twenty-seven passing tests,
`svelte-check` reports no errors or warnings, and the production build succeeds. The Rust suite has eighty-three tests:
seventy-nine pass by default and four live-provider tests remain opt-in. Three path-backed recovery tests cover
restricted corrupt startup, filtering corrupt managed-looking snapshots, newest verified automatic recovery, resumed
conversation access, and byte-exact damaged-main preservation.

The native app compiled and remained open against the existing healthy store. The recovery screen was visually checked
at 1320 x 820 and 420 x 780 through a temporary browser-preview fixture; both recovery actions remained visible, the
responsive card stayed contained, and no console warnings or errors appeared. The temporary fixture was removed. A
destructive native corruption exercise was intentionally not run against the user's live application store; the real
filesystem replacement flow is covered by the path-backed Rust tests.

## Prior completed product slice: Automatic SQLite backup rotation

### Goal

Maintain recent verified local recovery points without requiring users to remember a manual backup routine or granting
the WebView any filesystem authority.

### Implemented shape

1. After the live store initializes and passes its integrity policy, Bottie schedules rotation on a blocking native
   worker so startup presentation is not held behind the database copy.
2. When no managed automatic snapshot is newer than 24 hours, SQLite's online backup API writes a unique staging file,
   reopens it with `quick_check=ok`, and atomically renames it into the app-private `automatic-backups` directory.
3. Only filenames matching Bottie's timestamp-and-UUID contract participate in rotation. After a successful new
   snapshot, the seven newest managed files remain and older managed files are removed.
4. Manual backups, pre-restore safety copies, unrecognized files, and the live database remain outside the rotation set.
   A copy or prune failure leaves the app usable and adds a stable path-redacted error to Recent diagnostics.
5. Successful creation and already-current outcomes are also visible in Recent diagnostics without returning directory
   names, filenames, or database paths to the WebView.

### Acceptance criteria

- The first successful startup rotation creates an independently readable snapshot containing committed WAL content.
- Another startup inside 24 hours reuses the current set; reaching the 24-hour boundary creates a new snapshot.
- Rotation keeps seven successful managed snapshots and creates/verifies the new snapshot before removing an old one.
- Manual backups, restore safety copies, and unrecognized files are never removed by automatic retention.
- Rotation failures do not block Bottie startup and do not expose a native path or SQL detail.
- No corruption recovery, migration rollback, backup settings, batch/JSON export, provider, credential, or tool behavior
  is added.

### Verification completed

The standard frontend and Rust checks passed on 2026-08-20. The frontend suite has twenty-seven tests and
`svelte-check` reports no errors or warnings. The Rust suite has eighty tests: seventy-six pass by default and four
live-provider tests remain opt-in. Path-backed rotation tests cover the 24-hour boundary, independent snapshot
readability, seven-file retention, and preservation of unrecognized and pre-restore files.

The native app compiled and opened twice against the existing application store. The first fresh process created one
automatic snapshot; immutable read-only inspection confirmed `quick_check=ok`, schema version 6, the built-in local
profile, and the same five-conversation/26-message counts as the live store. A second fresh process inside the retention
interval kept exactly that one snapshot, confirming the real startup skip path. Provider live tests were not required
because this slice does not change provider networking, streaming, cancellation, or credentials.

## Prior completed product slice: Manual SQLite restore

### Goal

Let users replace the live conversation store with a validated Bottie backup without exposing filesystem paths to the
WebView or risking the only current copy of their data.

### Implemented shape

1. A global toolbar action opens a SQLite-filtered native Open dialog, then requires an explicit native warning-dialog
   confirmation. Restore is unavailable during provider generation or other storage management.
2. Rust opens the candidate read-only and requires `quick_check=ok`, a supported Bottie schema version, the expected
   foundation tables, and the built-in local profile before any live data changes.
3. The candidate is copied to a same-directory staging database through SQLite's online backup API. Bottie applies any
   supported pending migrations and repeats validation on that isolated copy.
4. Rust creates a verified application-private snapshot of the current live store before restoring the staged database
   through SQLite's backup API. A restore failure attempts to roll back from that safety copy, which remains available.
5. The WebView receives only `restored` or `cancelled` plus the selected and safety-copy leaf filenames. It reloads the
   restored navigation and exact last-open selection without receiving a path, SQL, or generic filesystem capability.

### Acceptance criteria

- A valid Bottie backup replaces the visible live conversations and preserves its selected branch/profile state.
- The pre-restore safety copy independently reopens with the conversations that existed immediately before restore.
- Corrupt, unrelated, empty, newer-schema, and live-database candidates are rejected before the live store changes.
- Cancelling either native dialog is neutral; active provider work blocks restore at the native command boundary.
- No scheduled rotation, corruption recovery, batch/JSON export, provider, credential, or tool behavior is added.

## Prior completed product slice: Manual SQLite backup

### Goal

Let users create a portable, consistent snapshot of all local conversation data without granting the WebView
filesystem access or combining backup creation with restore and recovery policy.

### Implemented shape

1. A global toolbar action opens a SQLite-filtered native Save dialog even when no conversation is selected.
2. Rust copies the live store through SQLite's online backup API on a blocking worker, so the snapshot includes all
   committed content visible through the WAL-aware source connection.
3. Rust reopens the completed snapshot and requires `quick_check=ok` before reporting success. The command maps SQL and
   filesystem details to a path-redacted error.
4. The WebView receives only `saved` or `cancelled` plus the chosen leaf filename. It never receives the source path,
   destination directory, SQL access, or a generic filesystem capability.

### Acceptance criteria

- A completed backup opens as an independent SQLite database and contains committed conversations and messages.
- Backup creation does not mutate the live database, selected conversation, branches, messages, or ratings.
- The live database cannot be selected as its own backup destination.
- Cancelling is neutral, while copy or integrity failures return a stable path-redacted message.
- No automatic rotation, corruption recovery, migration, batch/JSON export, provider, or tool behavior is added.

## Prior completed product slice: Conversation Markdown export

### Goal

Let users save the selected visible conversation lineage as portable Markdown without granting the WebView filesystem
access, exporting hidden branch siblings, or turning conversation export into database backup/restore.

### Implemented shape

1. A pure Rust renderer produces a deterministic UTF-8 Markdown document with the conversation title, user and
   assistant turns, separate reasoning/response sections, provider/model provenance, local rating, and explicit labels
   for interrupted, cancelled, or failed assistant output.
2. Export reconstructs only the selected branch lineage through the existing profile-scoped storage policy. It does
   not change the selected conversation, activity timestamps, schema, messages, branch history, or ratings.
3. One async native command opens a Markdown-filtered Save dialog and writes the prepared document in Rust. The
   WebView receives only `saved` or `cancelled` plus the chosen leaf filename; local directories never cross IPC.
4. The toolbar exposes a labelled file action only when a durable conversation is open, disables it during generation
   or storage work, treats cancellation as neutral, and shows compact saved/error feedback.

### Acceptance criteria

- Export contains only the current selected lineage and retains exact user/assistant Markdown plus separate reasoning.
- Provider, model, rating, and non-final response status remain human-readable without database or run identifiers.
- User-controlled titles yield bounded cross-platform `bottie-*.md` suggestions.
- The native command owns dialog and file writing; no dialog/filesystem capability or chosen directory is exposed to
  JavaScript.
- Cancelling does not create an error, while file failures return a path-redacted message.
- No migration, backup/restore, batch/JSON export, provider, credential, attachment, or tool behavior is added.

## Prior completed product slice: Response rating

### Goal

Let users retain a simple local quality signal on an exact assistant response without sending feedback to a provider,
changing generated content, or collapsing preserved branch alternatives.

### Implemented shape

1. Schema version 6 adds one optional Good/Poor rating row per immutable message. Ratings are local application data;
   no provider adapter, credential flow, or outbound request receives them.
2. One narrow native command sets, replaces, or clears a rating only for an assistant response in the selected visible
   lineage. User, foreign-conversation, hidden-sibling, and deleted-conversation targets are rejected.
3. Reopened conversations reconstruct the current rating beside each preserved assistant response. Ratings remain on
   their exact response when users switch branches and survive a fresh process.
4. Good and Poor controls expose `aria-pressed`, disable during generation or storage mutation, and use the active
   control as the clear action. Selected Good uses a dark green treatment and selected Poor uses dark red so state does
   not depend on a subtle white-brightness difference. A tested pure helper owns the toggle decision.

### Acceptance criteria

- A visible durable assistant response can be rated Good or Poor, changed between those choices, and cleared by
  selecting the active choice.
- Ratings survive restart and branch switching without changing conversation activity order or generated content.
- The Rust boundary rejects non-assistant, foreign, hidden, and deleted targets and exposes no generic database access.
- Live responses reload their durable message identity after terminal persistence before rating becomes available.
- No export, backup/restore, provider, credential, attachment, tool, or general feedback-comment behavior is added.

## Prior completed product slice: Response retry

### Goal

Let users try an interrupted, cancelled, or transiently failed request again without overwriting the failed attempt or
silently sending provider-authored failure text back as conversation context.

### Implemented shape

1. Live provider failures retain the normalized native `retryable` decision. Reopened interruptions, cancellations,
   timeouts, unavailable-provider failures, provider-server failures, and malformed responses reconstruct the same
   presentation state from durable message state and stable error codes.
2. Retry forks the unchanged visible user request through the existing native branch command, then starts generation
   with the provider, model, and reasoning route currently visible in the toolbar. The failed attempt remains on its
   original selectable branch.
3. Failed assistant text stays excluded from the next provider context. A tested pure helper now owns the conversion
   from visible successful messages to provider-neutral chat turns.
4. The message row uses a labelled Retry action for retryable terminal states and keeps Regenerate for completed or
   non-retryable responses. Both actions disable while generation/storage is busy or no route is available.

### Acceptance criteria

- Interrupted and cancelled responses can be retried after reopen; provider failures expose retry only when native
  normalization says the operation may succeed if attempted again.
- Retrying creates a new selected branch from the unchanged user request while preserving the original partial or
  failed response for later branch switching.
- Provider failure copy never enters the retried provider context, and no response content is overwritten or deleted.
- The retry action remains labelled, keyboard reachable, and contained at the native minimum viewport.
- No schema migration, native capability, provider adapter, credential flow, or general rating/export behavior is
  added.

## Prior completed product slice: Response copying

### Goal

Let users copy an assistant answer and its optional separate reasoning predictably without mixing in generated HTML or
response metadata.

### Implemented shape

1. Every non-empty assistant answer has a working copy action that writes Markdown through the WebView clipboard API.
   Answers without reasoning remain byte-for-byte unchanged so pasted Markdown stays portable and editable.
2. When separate provider reasoning exists, the clipboard document contains `## Reasoning` and `## Response` sections
   in generation order. Model and usage metadata and parser-generated HTML remain excluded.
3. The action reports `Copied`, `Copied with reasoning`, or `Copy failed` beside the response and exposes the result as
   a polite status update. Feedback clears after a bounded interval and repeat writes replace the prior timer cleanly.
4. Clipboard access stays in ephemeral WebView presentation code. No native command, Tauri permission, database write,
   or persisted-message transformation was added.

### Acceptance criteria

- Copying a streamed, reopened, interrupted, or alternative-branch answer includes the answer and any separate
  reasoning available at the moment the action runs.
- Answers without reasoning copy byte-for-byte; answers with reasoning use explicit Markdown section headings rather
  than parser-generated HTML.
- Successful writes show a checkmark and accessible confirmation; unavailable or rejected clipboard access produces
  an accessible failure state without an unhandled error.
- Copying does not mutate durable conversation content or require a broader native capability.

## Prior completed product slice: Sanitized Markdown rendering

### Goal

Render structured assistant answers clearly without trusting provider-authored HTML, allowing unsafe navigation, or
silently fetching remote image resources.

### Implemented shape

1. `src/lib/markdown.ts` owns one tested parser configuration; raw HTML stays escaped and only parser-generated markup
   reaches Svelte's HTML rendering boundary.
2. Explicit absolute HTTP, HTTPS, and email links open in an isolated browsing context with `noopener noreferrer`.
   Relative destinations and other protocols do not become anchors.
3. Markdown images become inert text labels so an answer cannot trigger a remote tracking request. User messages and
   provider reasoning remain plain text and do not pass through the Markdown renderer.
4. Assistant typography covers headings, paragraphs, emphasis, lists, quotes, inline and fenced code, tables, rules,
   and links without adding syntax-highlighting execution or changing persisted message content.
5. Rendering remains derived presentation state, so streamed, interrupted, reopened, and alternative-branch answers
   share the same policy without a storage migration.

### Acceptance criteria

- Common answer structure renders consistently for both streamed and reopened assistant messages.
- Provider-supplied HTML is displayed as text rather than interpreted as DOM.
- Script-like, relative, and unsupported link destinations cannot navigate the WebView.
- Remote Markdown images cannot initiate a network request; their alt text remains visible.
- User prompts and hidden reasoning retain their existing plain-text presentation.
- The WebView receives no new native capability and durable message content remains unchanged.

## Prior completed product slice: Conversation search

### Goal

Find a durable conversation from its title or visible message text without exposing SQL, generic database access, or
hidden reasoning content to the WebView, and reveal the exact preserved branch containing the match.

### Implemented shape

1. A narrow native command normalizes literal case-insensitive queries, rejects input over 200 characters, searches
   active and archived conversation titles plus text blocks, and returns at most 50 results in activity order.
2. Deleted conversations and separate reasoning blocks remain outside normal search. No schema migration, FTS5 index,
   or embedding dependency was added ahead of the later memory-search milestone.
3. Each result includes a bounded Unicode-safe excerpt and the opaque branch identity needed to reveal the matched
   lineage. Matches already visible on the selected lineage keep that branch selected.
4. Opening a result first records the conversation as the local profile's last-open thread, then selects the matching
   branch when needed.
5. The responsive sidebar provides a persistent search field, `Command/Ctrl+K` focus, Escape clearing, loading and
   empty states, archived labels, and disabled navigation while generation owns the conversation.

### Acceptance criteria

- Search is case-insensitive and treats `%`, `_`, and other query characters literally rather than as SQL patterns.
- Title and visible text matches include active and archived conversations but exclude recoverable Trash.
- Provider reasoning content is not returned as an ordinary conversation-search hit.
- A match on a preserved alternative opens the matching branch and survives restart as the exact last-open selection.
- Results are bounded to 50 and queries to 200 characters without adding an index or broad memory-retrieval contract.
- The WebView receives only typed result metadata and opaque conversation/branch identities, never SQL or database
  paths.

## Prior completed product slice: Edit-and-regenerate branches

### Goal

Let users revise an earlier prompt or request another answer without overwriting durable history, copying ancestor
messages, or allowing the WebView to choose arbitrary storage relationships.

### Implemented shape

1. Schema version 5 adds one selected branch reference to each conversation and deterministically selects the existing
   main branch during migration.
2. Editing a visible final user message creates a new native-owned branch and user request whose parent is the edited
   message's predecessor; regenerating uses the same operation with unchanged request text.
3. Provider generation starts only from the new request on the selected branch. Checkpoints, terminal state,
   provenance, reasoning, and usage retain the existing native persistence guarantees.
4. Conversation loading follows parent-message ancestry across branch boundaries, so shared history is not duplicated.
5. A compact branch selector reopens every preserved lineage; branch changes and edits are rejected while generation
   is active.

### Acceptance criteria

- Editing any visible user prompt preserves the original lineage and excludes its superseded descendants from the new
  provider request.
- Regenerating an assistant response creates a sibling branch from its preceding durable user request.
- Switching branches restores the matching messages, response metadata, and provider-run content after restart.
- Hidden sibling messages, assistant messages, deleted conversations, and active-run conversations cannot become fork
  targets through the native command boundary.
- Version-four stores migrate transactionally to a selected main branch without rewriting messages or provider runs.
- The WebView receives opaque message/branch identities and narrow edit/select commands, never raw parent links or SQL.

## Prior completed product slice: Exact last-open conversation restoration

### Goal

Restore the built-in local profile's actual selected conversation after restart without making the WebView own durable
navigation preferences or silently reopening an older thread after the user chose a blank new chat.

### Implemented shape

1. Schema version 4 adds one nullable profile-owned last-open conversation reference. Existing version-three stores
   seed it once from their newest active conversation to preserve prior startup behavior during upgrade.
2. Creating a conversation records it in the same transaction, while opening an existing non-deleted conversation
   atomically records and returns it through the narrow native command.
3. Startup loads the exact stored selection instead of inferring it from recent-conversation ordering.
4. New chat clears the durable selection, so an intentional blank composer remains blank after restart.
5. Archiving or deleting the selected conversation clears the reference in the same lifecycle transaction; changing
   another conversation does not disturb the current selection.

### Acceptance criteria

- Opening an older conversation and restarting restores that exact thread even when a newer thread exists.
- Creating a first-send conversation immediately makes it the durable profile selection.
- A blank new-chat view survives restart without deleting or altering prior conversations.
- Selected archived/deleted conversations do not become stale startup targets.
- Version-three databases migrate transactionally and receive a deterministic initial selection.
- The WebView receives neither profile-table access nor a generic preference or database capability.

## Prior completed product slice: Crash-safe partial responses and interrupted-run recovery

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

The following passed on 2026-08-20 for manual SQLite restore:

```sh
npm run format:check
npm run check
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

The frontend suite remains at twenty-seven passing tests and `svelte-check` reports no errors or warnings. The Rust
suite has seventy-eight tests: seventy-four pass by default and four remain opt-in live-provider checks. Four focused
backup/restore tests cover a path-backed WAL-aware snapshot, independent reopen and integrity checking, committed
message content, rejection of the live database as its own destination, successful replacement, independent reopening
of the pre-restore safety copy, and rejection of an unrelated SQLite database without changing live state.

The browser preview was visually checked at 1320 x 820 and the 720 x 620 native minimum. The backup and conversation
export actions and the new restore action remain labelled and contained in both desktop and compact toolbars, while all
three correctly stay disabled without their native prerequisites; there were no console warnings or errors.
The native app compiled and launched against the existing schema-version-6 store. A read-only inspection returned
`quick_check=ok` with the unchanged five conversations, twenty-six messages, seven provider runs, and three ratings.
macOS denied assistive access for automated native clicking, so the destructive restore confirmation was not exercised
against the user's live store; path-backed integration tests cover the actual replacement and safety-copy contract.
Live-provider tests were not required because this slice does not change provider networking, streaming, cancellation,
or credentials.

## Verification completed for the previous manual-backup slice

The native app compiled and launched against the existing schema-version-6 store. The user manually confirmed the real
Save-panel flow, which created `bottie-backup.sqlite3`; an independent SQLite inspection returned `quick_check=ok`,
schema version 6, and the same five conversations, twenty-six messages, seven provider runs, and three ratings as the
live store.

## Verification completed for the previous conversation-Markdown-export slice

The export slice passed the same standard frontend and Rust commands. Four focused tests cover deterministic Markdown
and metadata, bounded filename normalization, selected-lineage-only reconstruction without last-open mutation, and
exact path-backed UTF-8 file output. The real native Save-panel flow retained expected conversation content, including
the separate reasoning trace, without returning its destination directory to the WebView.

## Verification completed for the previous response-rating slice

The following passed on 2026-08-20 for response rating:

```sh
npm run format:check
npm run check
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

The frontend suite has twenty-seven tests, including the pure Good/Poor replacement-and-clear decision, and
`svelte-check` reports no errors or warnings. The Rust suite has seventy tests: sixty-six pass by default and four are
opt-in live-provider tests. Twenty-six path-backed storage tests include schema-version-6 migration, rating persistence
across reopen, replacement, clearing, and rejection of user, foreign, hidden-sibling, and deleted targets.

The browser preview was visually checked at its desktop size and at 800 x 700. The response action row remains
contained, and its unrated buttons expose false `aria-pressed` state while correctly remaining disabled without native
storage. The native app compiled and launched twice against the existing store; a read-only host check confirmed
schema version 6, the `assistant response ratings` migration record, and `quick_check=ok`. The default-size native
window reopened the selected branch without layout or launch errors. macOS denied assistive access for automated native
clicking, but the real rating mutation and persistence flow was manually confirmed on 2026-08-20. A follow-up contrast
review found the original violet/white active treatment too subtle, so selected Good and Poor controls now use distinct
dark green and dark red treatments. Live-provider tests were not required because this slice does not change provider
networking, streaming, cancellation, or credentials.

## Verification completed for the previous response-retry slice

The following passed on 2026-08-20 for response retry:

```sh
npm run format:check
npm run check
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

The frontend suite now has twenty-six tests. Retry-focused coverage verifies durable retryability for interruption,
cancellation, transient provider failures, and non-retryable invalid/internal failures, plus exclusion of failed and
empty assistant text from provider context. `svelte-check` reports no errors or warnings.

The standard Rust suite now has sixty-eight tests: sixty-four pass by default and four are opt-in live-provider tests.
A path-backed SQLite regression confirms that retry branching preserves the failed partial response on its original
selectable branch. The browser preview was visually checked at its desktop size and the 720 x 620 native minimum using
a temporary failed-response fixture; the labelled Retry action remained contained and no console errors appeared. The
temporary fixture was removed after inspection. The native app compiled, launched against the existing schema-version-5
store, and displayed the current conversation without console errors. macOS denied assistive access for automated
native clicking, but the native retry flow was manually confirmed on 2026-08-20. Live-provider tests were not required
because this slice does not change provider networking, streaming, cancellation, or credentials.

## Verification completed for the previous sanitized-Markdown slice

The sanitized-Markdown checks included the same standard frontend and Rust commands plus the production dependency
audit. Six focused Markdown-policy tests cover answer structure, tables, raw HTML escaping, external-link isolation,
unsafe destinations, remote-image neutralization, and empty streaming state. The browser preview was visually checked
at 1320 x 820 and 800 x 700 with representative headings, an ordered list, inline code, and emphasis.

## Verification completed for the previous conversation-search slice

The conversation-search checks included the same standard frontend and Rust commands plus a native app launch. The
path-backed native search tests exercise the real SQLite schema without a new migration or index. The sidebar search
field, empty state, focus treatment, and navigation layout were visually checked in the browser preview at the desktop
default and an 800 x 700 responsive viewport; the browser preview intentionally cannot invoke the native search
command. The Tauri app compiled and launched against the existing application store without console errors.

## Verification completed for the earlier edit-and-regenerate slice

The following passed on 2026-08-20 for edit-and-regenerate branching:

```sh
npm run check
npm run format:check
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

The standard Rust suite now has sixty-four tests: sixty run by default and four are opt-in live-provider tests. Twenty
storage tests cover migration and profile policy, version 2-to-5, 3-to-5, and 4-to-5 upgrades,
WAL/foreign-key/integrity state, transactional conversation/message round trips, parent-linked branch ancestry and
selection, branch-local ordering under equal timestamps, exact/blank last-open restoration,
provider/model/reasoning retention, provider-run/usage reconstruction, native partial checkpoints, legacy
running-record recovery, lifecycle transitions, recoverable deletion, and invalid input. The frontend suite has
fourteen pure-helper tests, including durable completion metadata, recovered-message labels, response-to-request
resolution, and local-calendar date grouping. Live-provider tests were not required because this slice does not change
provider networking or streaming.

The storage suite exercises schema version 5 branch creation, provider response persistence, selection, and process
reopen migration against real path-backed SQLite databases. The native app also compiled and launched against the
existing local store without console errors; a read-only host query confirmed schema version 5 and selected branches
for all three existing conversations. The desktop WebView was visually checked at its default size with the new edit
affordance present and no overflow. End-to-end branch mutation is covered by the path-backed native integration test;
macOS denied assistive access for automated clicking in the native window during this run.

That earlier branching handover led to the completed export, recovery, and tool-persistence slices recorded above.
Keep
FastEmbed/EmbeddingGemma implementation with the first memory-search consumer, where download progress, cache location,
dimensions, and reindex metadata can be implemented coherently.

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
