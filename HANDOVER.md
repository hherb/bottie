# bottie handover

Last verified: 2026-08-22

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
dialogs without revealing the chosen path to the WebView. Referenced conversation- and message-scoped files turn the
export into a ZIP containing the document plus hash-deduplicated originals; attachment-free exports stay plain. A
separate global action applies the same behavior to every active and archived conversation's selected lineage while
excluding Trash and hidden branch siblings. Users can also create a complete verified SQLite snapshot through a
separate Rust-owned Save dialog; SQLite's online backup API includes committed WAL content plus verified original and
ready-normalized attachment bytes in backup-only tables without pausing the live store, and the destination path
remains native-only. A separate native Open-and-confirm flow now restores
validated Bottie backups only after
creating an application-private snapshot of the current store; selected directories and database paths never reach the
WebView. After a successful startup, Bottie now creates a verified application-private snapshot when no automatic
backup is newer than 24 hours and retains the seven newest automatic snapshots. Rotation runs in the background, never
prunes manual backups or pre-restore safety copies, and reports a path-redacted outcome in session diagnostics. If
SQLite reports corruption at startup, Bottie now opens in a restricted recovery state instead of aborting launch. The
guided screen can restore the newest verified automatic snapshot or a manually selected Bottie backup after preserving
the damaged database bundle and prior attachment tree in app-private storage. Native provider runs now also retain
ordered structured tool calls
and one append-only result per call; reopened tool activity is inspectable and portable without exposing native or
provider call identities. Provider tool loops and execution remain absent. Native attachment selection now streams
chosen local files into application-private content-addressed storage with SHA-256 identities, content-based MIME
sniffing, safe display names, a 25 MiB per-file limit, an eight-file selection limit, and cross-session duplicate
detection. Source and storage paths never reach the WebView, and the interface explicitly labels retained attachment
delivery state for the selected model. A selected draft can now commit atomically with its user message, reopen as
ordered path-free metadata
on the selected branch, and remain attached when that request is edited or regenerated. Association removal is limited
to visible user messages while generation is idle and retains the content-addressed catalog row and blob. Retained
UTF-8 plain-text, Markdown, PDF, and DOCX attachments now receive bounded native
extraction with durable ready, unsupported, or failed state. PDF work is limited to 500 pages, 8 MiB of decompressed
content per page, and 2 MiB of retained extracted text. DOCX work validates the package manifest, bounds archive
entries and total declared expansion, reads only bounded in-memory XML, caps XML events/depth, and shares the 2 MiB
retained-text ceiling. JPEG and PNG attachments now receive bounded native normalization into content-addressed,
application-private derivatives. Rust applies EXIF orientation, caps dimensions, pixels, decoder allocation, and
encoded output, and re-encodes without forwarding source metadata. The WebView receives only path-free processing
state, format, dimensions, counts, and sizes. Ingestion now returns after committing durable pending work; one native
worker resumes extraction and normalization after startup, selection, or restore and streams path-free state updates to
visible draft and message attachments. Native generation now reconstructs exact durable text context and reads
normalized JPEG/PNG bytes only inside Rust. A current image requires explicit vision capability from native model
discovery; text-only models block that draft while omitting older images from later text requests. Vision routes receive
at most eight images and 50 MiB of normalized derivatives through Ollama, OpenAI-shaped, or Anthropic-shaped request
blocks. Documents remain local-only. Extracted text now also moves through durable waiting-for-extraction, indexable,
unsupported, or blocked readiness in the same resumable native worker. Indexable is eligibility for native derived
memory construction, not provider delivery. Up to eight retained
files can now be kept in durable conversation scope independently of any branch or message. The interface distinguishes
next-message, conversation, and message associations and supports narrow removal without deleting retained content.
Conversation-scoped normalized images apply to every current request, require explicit vision capability, and are
deduplicated when the same file is also linked to a message; documents remain local-only. Portable backups now retain
every original blob and ready normalized derivative, while selected and batch exports bundle only referenced originals
with versioned path-free manifests. Each successful non-recovery startup now removes attachment catalog rows older
than a 24-hour safety window with no message or conversation reference, then sweeps equally old strict untracked
original/derivative files and interrupted temporary work without touching recent cross-process drafts, recoverable
Trash references, or shared derivatives. Milestone 3 is now complete: ready normalized images render as bounded,
metadata-free thumbnails in draft, context, and retained-message surfaces through an opaque attachment-ID-only native
protocol. Original and derivative hashes do not serialize through attachment IPC. Failed extraction and normalization
remain explicitly local and show a specific accessible consequence without adding retry controls. Persistent-memory
work now includes a native derived SQLite FTS5 index and bounded BM25 query layer over complete final message answers
and ready extracted documents. Schema version 17 also derives a versioned deterministic chunk catalog from those same
eligible sources. Unicode-safe, whitespace-aware chunks retain exact source offsets, stable SHA-256 identities, and a
1,200-character ceiling with approximately 200 characters of overlap. Schema version 18 statically links sqlite-vec
and maps those chunks to durable 768-dimensional cosine vectors through a resumable native worker. Rust owns the Q4
EmbeddingGemma 300M FastEmbed runtime, application-data cache, bounded batches, model/index versions, progress, and
stable failure categories. Reasoning and non-final responses never enter any derived layer. No memory query, vector,
chunk identity, cache path, or extracted content crosses IPC or enters provider context. A bounded Rust-only semantic
KNN contract now embeds normalized queries with EmbeddingGemma's versioned retrieval prompt, validates vectors, and
returns exact deterministic chunks from the current generation. sqlite-vec prefilters eligible row identities before
ranking, so profile, lifecycle, durable attachment association, source, conversation, and inclusive date policy match
lexical retrieval without consuming the result limit on ineligible nearer vectors. The next bounded implementation
slice is Rust-only reciprocal-rank fusion of lexical and semantic results under one shared filter contract; do not
bundle retrieval injection, memory tools/UI, reindex controls, provider changes, or attachment retry controls.

Read these files first:

1. `HANDOVER.md`
2. `ROADMAP.md`
3. `README.md`
4. `CONTRIBUTING.md`
5. `src/routes/+page.svelte`
6. `src/routes/page-state.svelte.ts`
7. `src-tauri/src/lib.rs`
8. `src-tauri/tauri.conf.json`

The repository tracks `origin/main` at `https://github.com/hherb/bottie.git`. The current product slice is on local
branch `codex/semantic-memory-knn`.

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
- native attachment selection, application-private ingestion, durable selected-lineage message and branch-independent
  conversation associations, explicit scope labels, narrow removal, and path-redacted outcome feedback;
- durable background plain-text, Markdown, page-aware PDF, DOCX, JPEG, and PNG processing with live path-free labels;
- bounded ready-image thumbnails plus explicit local-only extraction and normalization failure presentation;
- durable extracted-text indexing readiness with honest indexable, unsupported, and blocked presentation;
- capability-aware normalized JPEG/PNG delivery labels and current-draft blocking for text-only models;
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
bounded append-only tool-call/result records, `src-tauri/src/storage/attachments.rs` owns bounded streaming ingestion,
SHA-256 content identities, MIME sniffing, safe display names, deduplicated metadata, app-private blob placement,
ordered message associations, selected-lineage validation, and association removal,
`src-tauri/src/storage/conversation_attachments.rs` owns bounded ordered conversation associations, local-profile
validation, active-run exclusion, and association removal without content deletion,
`src-tauri/src/storage/attachment_processing.rs` selects one oldest pending item without introducing an in-progress
lease state,
`src-tauri/src/storage/attachment_indexing.rs` derives durable readiness for future text indexing without retaining or
claiming any index content,
`src-tauri/src/storage/extraction.rs` owns extraction persistence and bounded UTF-8/PDF parsing,
`src-tauri/src/storage/docx.rs` owns bounded package validation and WordprocessingML text extraction,
`src-tauri/src/storage/image_codec.rs` owns bounded JPEG/PNG decode, orientation, and metadata-free encoding,
`src-tauri/src/storage/image_normalization.rs` owns derivative persistence and path-free normalization state,
`src-tauri/src/storage/portable_backup.rs` owns verified backup-only byte tables and staged rehydration,
`src-tauri/src/storage/portable_export.rs` owns deduplicated ZIP members and portable attachment references,
`src/lib/storage-transfer.ts` owns the path-redacted frontend backup, restore, and export contracts,
`src-tauri/src/storage/attachment_delivery.rs` owns selected-lineage reconstruction, current-image readiness, bounded
derivative loading, and path-free delivery errors,
`src-tauri/src/storage/attachment_preview.rs` owns bounded metadata-free thumbnail generation from ready derivatives,
and `src-tauri/src/attachment_preview_protocol.rs` serves those pixels only through GET requests containing one opaque
attachment ID,
`src-tauri/src/storage/attachment_garbage_collection.rs` owns restart-boundary catalog pruning, strict managed-file
sweeping, interrupted temporary cleanup, and shared-derivative preservation,
`src-tauri/src/storage/memory_lexical.rs` owns bounded native BM25 queries and lifecycle/association filters, while
`src-tauri/src/storage/memory_lexical_migration.rs` owns the derived FTS5 schema, backfill, and synchronization triggers,
`src-tauri/src/storage/memory_chunks.rs` owns versioned Unicode-safe deterministic chunking plus transactional source
replacement, while `src-tauri/src/storage/memory_chunks_migration.rs` owns catalog metadata and stale-row cleanup,
`src-tauri/src/storage/memory_semantic.rs` owns static sqlite-vec registration, version-contract validation, bounded
embedding batches, atomic chunk/vector mappings, and native-only progress, while
`src-tauri/src/storage/memory_semantic_migration.rs` owns schema-18 model/index metadata and vector cleanup triggers,
`src-tauri/src/storage/memory_semantic_query.rs` owns normalized EmbeddingGemma retrieval queries, exact filtered KNN,
bounded chunk provenance, and current-generation validation,
`src-tauri/src/semantic_indexer.rs` owns lazy app-cache FastEmbed acquisition plus the resumable process-lifetime Q4
EmbeddingGemma worker,
`src-tauri/src/attachment_processor.rs` owns the single process-lifetime worker, path-free completion events, and
restore pause/resume coordination,
and `src-tauri/src/storage/export.rs` owns
deterministic selected-lineage Markdown plus selected and batch JSON rendering and safe suggested filenames, and
`src-tauri/src/storage/backup.rs` owns
consistent online SQLite snapshots, strict automatic-backup discovery and rotation, restore validation, isolated
migration, pre-restore
safety copies, and post-copy integrity checks, `src-tauri/src/storage/recovery.rs` owns read-only startup corruption
classification, verified automatic recovery-point discovery, restricted store state, damaged-bundle preservation, and
staged replacement, and
`src-tauri/src/generation.rs` reconciles the current WebView prompt with durable selected-lineage context, applies
native-discovered vision policy, and closes each native run before its terminal stream event reaches the WebView.
`src-tauri/src/storage_commands.rs` exposes only list/search, create, selected-load/clear, atomic user-message and
attachment append, visible message-attachment removal, explicit branch, response-rating, selected-lineage Markdown/JSON
and non-trashed batch JSON export, whole-store
backup/restore,
recovery-status/latest-snapshot restore, and lifecycle commands. The database lives in the OS application-data
directory; the WebView never receives a database or attachment path, SQL, or generic filesystem/database
capability. One built-in `local` profile represents the current OS account. Every conversation has
a selected branch, and every message stores a branch-local append sequence plus independently ordered text/reasoning
blocks. User prompts commit before inference starts, and terminal assistant responses commit before another prompt can
append. Rust creates each assistant response with its run, checkpoints every provider text/reasoning delta before IPC,
and marks leftover running records interrupted during the next startup. Assistant responses reference opaque native
provider runs, and reopened conversations reconstruct real elapsed time plus provider-reported token/cost usage without
estimating missing values. Creating or opening a conversation records it as the local profile's exact selection;
starting a blank chat clears that selection, and archiving or deleting the selected conversation clears it in the same
transaction as the lifecycle change. Editing, regenerating, or retrying creates one new branch whose first request
points to the visible predecessor from the selected lineage; its ordered attachment associations are copied onto the
new request. Switching branches reconstructs ancestry and message attachments through native-owned parent message links
without copying or deleting the original history.

The oMLX adapter:

- owns and validates a configurable loopback endpoint, defaulting to `http://127.0.0.1:8000/`;
- discovers models with `GET /v1/models` and enriches vision/residency from `GET /v1/models/status` when available;
- streams `POST /v1/chat/completions` SSE responses;
- normalizes started, text delta, reasoning delta, usage, completed, cancelled, and failed events;
- maps connection, timeout, HTTP, and malformed-response failures to structured user-readable errors;
- aborts the active HTTP stream when the UI cancels a run.

The Ollama adapter:

- owns and validates a configurable loopback endpoint, defaulting to `http://127.0.0.1:11434/`;
- discovers installed models with `GET /api/tags`, capabilities/context with `POST /api/show`, and loaded state with
  `GET /api/ps`;
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
- current next-message attachment selection is session-only until it commits atomically with a submitted user message
  or is explicitly promoted into an existing conversation's durable context; an unassociated selection is eligible
  for native garbage collection after the 24-hour safety window and a later successful startup because the draft itself
  does not survive restart;
- plain-text, Markdown, PDF, and DOCX attachments are extracted into SQLite but remain unsent; their indexable state
  feeds a whole-source native FTS5 index and deterministic native chunk catalog, but no vector, embedding, tool, or
  provider retrieval exists; JPEG/PNG
  derivatives remain application-private and are read only for capability-confirmed vision requests; portable SQLite
  backups embed originals and ready derivatives, while selected/batch exports bundle referenced originals;
- provider adapters and orchestration do not yet emit or execute the persisted tool records; browser-preview tool
  activity remains a fixture;
- reasoning-toggle state is session-only and resets to off when the app restarts;
- the native FTS5 and semantic KNN memory queries have no IPC, tool, citation, context-panel, fused-ranking, or
  provider-injection consumer yet;
- no web search or fetch tool exists;
- there are no automated end-to-end UI tests yet; the composer has focused server-rendered component coverage, and
  pure presentation and Markdown-policy helpers have frontend unit coverage.

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
9. Persistent memory uses Rust-owned FastEmbed with Q4 EmbeddingGemma 300M as one built-in default. Keep its cache,
   progress, model/index metadata, and vectors native; do not add an embedding-provider picker or silently inject
   retrieved memory into provider requests.

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

## Most recently completed product slice: Filtered semantic KNN retrieval

### Goal

Add a bounded Rust-only semantic query contract over the current sqlite-vec generation with lexical-equivalent policy,
without adding fused ranking, retrieval injection, memory tools/UI, reindex controls, or WebView exposure.

### Implemented shape

1. Native queries normalize whitespace, reject more than 200 Unicode characters, prepend EmbeddingGemma's versioned
   `task: search result | query:` retrieval prompt, and require exactly one finite 768-dimensional vector. Empty queries
   and zero-result requests return without initializing the embedding boundary.
2. Exact cosine KNN returns at most 50 deterministic chunks with opaque source identity, source kind, ordinal, exact
   Unicode offsets, bounded text, source creation time, and distance. It reads only mappings whose embedding, model,
   dimensions, chunking, and index generation match the compiled current contract.
3. sqlite-vec receives an exact `rowid IN` candidate subquery before distance ranking. The prefilter always enforces the
   built-in profile, excludes Trash, retains Archived, and rejects unassociated documents; source, conversation, and
   inclusive date filters run inside the same native candidate policy.
4. Query text, vectors, distances, chunk provenance, and extracted document content remain Rust/SQLite-only. No schema,
   migration, command, IPC type, Svelte state, provider request, export, memory tool, or model-cache behavior changed.

### Acceptance criteria

- A nearer unassociated or trashed vector cannot consume the requested KNN limit ahead of an eligible result.
- Message and associated-document chunks share lexical retrieval's profile, lifecycle, association, source,
  conversation, and inclusive-date rules; Archived remains eligible and Trash does not.
- Query text and result counts are bounded, malformed filters fail before embedding, and wrong vector counts,
  dimensions, or non-finite values never reach sqlite-vec.
- Semantic queries, chunk excerpts, opaque identities, distances, and embedding inputs remain absent from IPC,
  provider context, exports, and the interface.
- Reciprocal-rank fusion, retrieval injection, memory tools/UI, reindex controls, provider changes, and attachment retry
  remain outside this slice.

### Verification completed

Focused TDD coverage first failed against the absent semantic-query module and method. Four query tests now cover the
versioned retrieval prompt, cosine ordering and bounded chunk provenance, exact candidate prefiltering, document
association, Archived/Trash lifecycle, source/conversation/inclusive-date filters, malformed filters, the 50-result and
200-character ceilings, and wrong-count, wrong-dimension, and non-finite embedding rejection. The complete Rust suite
reports 174 passed with four opt-in live-provider checks intentionally ignored. Prettier, `svelte-check`, all 53
frontend tests, the production build, Cargo formatting, Cargo check, and `git diff --check` pass without warnings. A
fresh signed native launch reopened the unchanged schema-18 live store and remained available until stopped after
verification. Immutable read-only inspection reported `quick_check=ok`, `ready` semantic progress at 84/84 chunks, 84
unique current-generation mappings, 84 sqlite-vec row identities, 768 dimensions, index generation 1, and no running
provider records. No schema, UI, or provider behavior changed, so migration, browser layout, picker/provider
interaction, and live-provider tests were not applicable.

## Prior completed product slice: Resumable sqlite-vec semantic index

### Goal

Turn the deterministic chunk catalog into a durable native vector index with one built-in local embedding runtime,
without adding semantic queries, hybrid ranking, retrieval injection, memory tools/UI, or WebView exposure.

### Implemented shape

1. Schema version 18 registers the statically linked `sqlite-vec` C extension and adds a 768-dimensional cosine
   `vec0` table plus relational chunk mappings. Mapping deletion removes the matching vector, while chunk insertion or
   deletion updates durable progress and never leaves a stale retrievable vector.
2. Singleton metadata fixes embedding contract version 1 to FastEmbed 6, Q4 EmbeddingGemma 300M, 768 dimensions,
   chunking version 1, document-prefix contract 1, and index generation 1. Startup rejects metadata that does not match
   the compiled Rust contract instead of interpreting incompatible vectors.
3. A single native worker lazily checks or populates an application-data model cache, embeds at most eight chunks per
   batch, and commits mapping plus vector rows atomically. It wakes after startup, user-message append, branch creation,
   provider completion, and ready attachment extraction, and pauses with attachment processing around store restore.
4. Model acquisition, indexing, ready, and failed states plus completed/total counts survive restart. A failed batch
   retains prior vectors and a stable path-free error code; the next worker wake retries pending work. Model, cache,
   chunk, vector, and runtime failure details remain Rust-only.
5. The worker uses FastEmbed's `EmbeddingGemma300MQ4` variant with the versioned `title: none | text:` document input.
   No Tauri command, IPC type, Svelte state, provider request, export, semantic query, or retrieval consumer was added.

### Acceptance criteria

- Schema-17 stores migrate transactionally with sqlite-vec available on every Bottie SQLite connection.
- Versioned model, runtime, dimension, chunking, input, and index-generation metadata match the compiled contract.
- Bounded successful batches survive reopen and resume without duplicate rows; wrong counts, dimensions, or non-finite
  values fail without partial vector commits.
- Removing or replacing a deterministic chunk removes its relational mapping and sqlite-vec row before it can become
  stale memory.
- The production worker owns model acquisition/cache and restore coordination, while ordinary tests use a deterministic
  fake embedder and never download model files.
- Semantic queries, fusion, reindex controls, retrieval injection, memory tools/UI, and attachment retry remain outside
  this slice.

### Verification completed

Focused TDD coverage verifies static extension registration, schema-17 migration, exact metadata, stable document
input, bounded batch persistence, restart resume, duplicate prevention, cosine-query viability, dimension rejection,
atomic failure behavior, chunk/vector cleanup, and pause/resume scheduling. The complete Rust suite reports 170 passed
with four opt-in live-provider checks intentionally ignored. Prettier, `svelte-check`, all 53 frontend tests, the
production build, Cargo formatting, Cargo check, and `git diff --check` pass without warnings. A fresh signed native
launch migrated the live store from schema 17 to 18 while the window remained available, populated the
application-data EmbeddingGemma cache to 215 MiB, and moved durable progress from `loading_model` through indexing to
`ready`. Immutable read-only inspection reported `quick_check=ok`, 84/84 completed chunks, 84 unique relational
mappings, 84 sqlite-vec row identities, 768 dimensions, chunking version 1, index generation 1, and no running provider
records. A second fresh-process launch reopened `ready` immediately with the same cache size, no partial download file,
and unchanged vector counts. A desktop screen review confirmed the native conversation and context surfaces remained
visually healthy. No WebView behavior or provider networking changed, so browser responsive review, provider
interaction, and the four live-provider tests were not applicable.

## Prior completed product slice: Versioned deterministic memory chunks

### Goal

Create a stable native-only chunk catalog over final message answers and ready extracted documents without adding a
vector runtime, embeddings, retrieval injection, tools, or WebView exposure.

### Implemented shape

1. Schema version 17 adds singleton algorithm metadata and a derived `memory_chunks` catalog. Every row records source
   kind and opaque identity, built-in profile ownership, chunking version, ordinal, exact Unicode-scalar start/end
   offsets, source creation time, content SHA-256, and a stable SHA-256 chunk identity.
2. Chunking version 1 preserves exact source slices, prefers a whitespace boundary between 900 and 1,200 Unicode
   scalar values, and retains approximately 200 characters of word-aligned overlap. Identical source content and
   identity produce the same ordered rows across migration and restart.
3. Migration backfill runs inside the schema transaction over final message text and ready non-empty extracted text.
   Message reasoning, partial/cancelled/failed responses, and empty sources remain absent.
4. Final message append, edit/regenerate branch creation, provider completion, and attachment extraction replace their
   derived rows inside the same write transaction. Cleanup triggers delete stale rows before source mutation or delete,
   so unexpected direct changes can leave a missing derived source but never stale chunk content.
5. The catalog stays Rust/SQLite-only and remains derived state in ordinary backup/restore. No command, IPC type,
   provider request, export, Svelte state, vector extension, embedding runtime, or retrieval consumer was added.

### Acceptance criteria

- Schema-16 stores migrate transactionally and backfill deterministic chunks for final answers and ready documents.
- Unicode boundaries and exact offsets are safe, each chunk is at most 1,200 characters, adjacent chunks overlap, and
  algorithm/version constants match durable metadata.
- Streamed response text appears only after successful completion; reasoning and non-final responses remain absent.
- Newly ready documents receive chunks transactionally, while failed/invalidated extraction and deleted sources remove
  stale rows.
- sqlite-vec, FastEmbed/model download, semantic queries, reindex controls, memory tools/UI, provider injection, and
  attachment retry remain outside this slice.

### Verification completed

Focused tests first failed against the absent catalog modules and methods. Four chunk-specific tests now cover
deterministic Unicode boundaries and overlap, exact offsets and stable identities, schema-16 message/document backfill,
durable algorithm metadata, reasoning exclusion, partial-to-final provider lifecycle, runtime document extraction, and
stale-row removal. The complete Rust suite reports 164 passed with four opt-in live-provider checks intentionally
ignored. Prettier, `svelte-check`, all 53 frontend tests, the production build, Cargo formatting, and Cargo check pass;
`svelte-check` reports no errors or warnings and `git diff --check` is clean. A fresh signed native launch migrated the
live store from schema 16 to 17 and remained running until stopped after verification. Immutable read-only inspection
reported `quick_check=ok`, version-1 `unicode-whitespace-v1` metadata, 77 chunks across 52 message sources, seven chunks
across two attachment sources, no over-limit or offset-inconsistent rows, and no running provider records. No UI or
provider behavior changed, so browser layout review, picker interaction, and live-provider tests were not applicable.

## Prior completed product slice: SQLite FTS5 lexical-memory foundation

### Goal

Establish bounded native lexical retrieval over durable conversation and extracted-document content without exposing
memory search to the WebView or providers, and without bundling chunking, vectors, embeddings, or memory tools.

### Implemented shape

1. Schema version 16 adds a derived SQLite FTS5 index using the Unicode tokenizer with diacritic removal. Migration
   backfills one complete source for each final user/assistant text message and each ready non-empty extracted document.
   Separate provider reasoning and non-final assistant responses are never indexed.
2. SQLite triggers keep message sources synchronized as blocks append and responses become final, aggregating streamed
   deltas in durable ordinal order. Extraction transitions likewise add, replace, or remove one whole-document source;
   attachment deletion and message deletion remove their derived entries.
3. A native-only Rust query contract normalizes user text into quoted AND terms before FTS evaluation, limits queries
   to 200 characters and results to 50, returns bounded excerpts, and orders by SQLite BM25 with deterministic ties.
4. Native filters cover message/document source, exact conversation association, and inclusive creation dates. Search
   always restricts to the built-in profile, excludes Trash, and requires a ready document to have a durable message or
   conversation association, so a processed but unsubmitted draft cannot become retrievable memory.
5. The FTS index is derived state included in normal SQLite backup/restore behavior. No command, IPC type, Svelte state,
   provider request, tool, or export surface exposes the query, source identities, extracted content, or index internals.

### Acceptance criteria

- Existing schema-15 stores migrate transactionally and backfill final visible message text plus ready extracted text.
- Streamed answer deltas become exactly one searchable message source only after successful completion; reasoning,
  partial, cancelled, and failed responses remain absent.
- Ready extracted documents are searchable only while durably associated with a non-trashed conversation. Conversation,
  source, and date filters are applied inside Rust-owned SQLite queries.
- Operator-shaped input cannot inject FTS syntax, native bounds are enforced, and BM25 supplies stable lexical ranking.
- sqlite-vec, embeddings, model download/cache UX, chunking, resumable reindex, fusion, memory tools, retrieval injection,
  UI controls, and attachment retry remain outside this slice.

### Verification completed

Focused tests first failed against the absent migration, index, and native query contracts. Coverage now proves
schema-15 backfill, reasoning exclusion, terminal streamed-answer aggregation, associated-document eligibility across
restart, Trash/draft exclusion, BM25 ordering, filters, syntax normalization, and query/result ceilings. Cargo formatting
and check pass without warnings. The complete Rust suite reports 160 passed with four opt-in live-provider checks
intentionally ignored. Backup, restore, corruption recovery, and earlier migration tests remain green. Prettier,
`svelte-check`, all 53 frontend tests, and the production build pass; `git diff --check` is clean. A fresh signed native
launch migrated the live store from schema 15 to 16 and remained running until stopped after verification. Immutable
read-only inspection reported `quick_check=ok`, the FTS5 virtual table, all eight synchronization triggers, 52 indexed
message sources, two indexed attachment sources, and no running provider records. No UI changed, so browser layout
review and picker/provider interaction were not applicable; the four opt-in live-provider tests remained skipped.

## Prior completed development-workflow fix: macOS development signing

Fresh large ad-hoc-signed debug executables could be held by macOS execution policy for several minutes before Bottie
entered application code. The documented `npm run tauri dev` command now installs a Cargo runner only for macOS Tauri
development. That runner discovers the active Apple Development identities, requires an explicit environment choice
when multiple identities are usable, signs the exact freshly linked executable with a stable development identifier
and hardened runtime, verifies the signature, and then starts Bottie. It records no certificate label, fingerprint,
team, or private-key material in the repository. Non-macOS and non-development Tauri commands pass through unchanged,
and release signing and notarization configuration remain outside this workflow.

Focused unit coverage preserves identity selection, ambiguity and mismatch failures, macOS-development gating, Cargo
runner construction and unsupported-path rejection, and Tauri executable resolution. A temporary copy of the same
54 MiB debug executable was first signed and cold-launched to validate the approach: AppKit check-in began immediately
and the app reported ready about 70 ms later, instead of the previously observed multi-minute pre-application hold. A
fresh launch through the repository wrapper then produced a valid Apple Development signature, reached AppKit check-in
38 ms after Bottie's first unified-log activity, reported ready 46 ms after check-in, and recorded WebKit's first
meaningful paint at 0.387 seconds. The app remained live until the verification process stopped it with Control-C.
The standard checks also pass: Prettier reports clean formatting, `svelte-check` reports no errors or warnings, all 48
frontend tests pass, the production build succeeds, `cargo fmt --check` and `cargo check` pass, and the complete Rust
suite reports 140 passed with four live-provider tests intentionally ignored. `git diff --check` is clean. The only
known development-runner limitation is that Cargo's environment runner format cannot represent Node or repository
paths containing whitespace; the wrapper detects and reports that case before compilation.

## Prior completed product slice: Attachment previews and extraction-error UX

### Goal

Make ready local images visually recognizable and failed attachment preparation understandable without exposing an
arbitrary file surface, moving native identities or content into Svelte state, or bundling retry and retrieval work.

### Implemented shape

1. Rust resolves only a ready JPEG/PNG derivative from one opaque UUID, decodes it under the existing 128 MiB ceiling,
   scales it to at most 320 pixels on either axis, and re-encodes metadata-free JPEG or PNG bytes under a 2 MiB preview
   ceiling. Pending, unsupported, failed, missing, and malformed identities have no preview.
2. A dedicated `bottie-attachment` custom protocol accepts GET requests from Bottie's main WebView only. Responses use
   trusted media types, `no-store`, and `nosniff`; every failure is bodyless and path-redacted. CSP permits only that
   protocol's image origin in addition to bundled, blob, and data images.
3. Svelte derives the preview URL from the opaque attachment ID only after native metadata reports ready normalization.
   One shared visual renders lazy thumbnails in composer chips, context rows, and durable message cards, then falls
   back to the existing file icon if a preview request fails.
4. Stable path-free failure codes now produce a clear title and consequence: the original remains local, while failed
   document text is unavailable for later indexing and a failed image cannot be previewed or sent. Draft chips expose
   the same explanation accessibly; context and message cards show it directly.
5. Original SHA-256 content identities remain available to Rust storage/export internals but are skipped by both picker
   and reopened-attachment serialization, so the WebView receives only the opaque attachment ID and safe metadata.

### Acceptance criteria

- Preview requests cannot select a path, hash, MIME claim, source/original, extracted text, or arbitrary derivative;
  only one ready attachment UUID can resolve to a freshly re-encoded bounded thumbnail.
- The main WebView is the only protocol caller, only GET is accepted, and invalid/unavailable requests reveal no native
  detail. Preview pixels do not enter Tauri IPC or Svelte state.
- Draft, conversation, message, archived, and reopened attachments reuse the same typed presentation. Documents and
  incomplete/failed images retain icons, while a broken preview degrades to its icon without affecting delivery state.
- Extraction and normalization failures identify their local-only consequence without implying deletion, indexing,
  provider delivery, or retry support.
- Extraction retry, document delivery, other office formats, FTS/vector construction, embeddings, retrieval, and broad
  visual redesign remain outside this slice.

### Verification completed

Focused tests first failed against absent preview and failure contracts. Rust coverage proves pending/document/missing
content has no preview, ready PNG pixels are resized and re-encoded, hashes are absent from picker and reopened JSON,
and protocol method/path/header policy is enforced. Frontend coverage proves thumbnail/icon fallback, draft failure
accessibility, context error presentation, stable failure explanations, and path-free processing updates.

The standard checks pass: Prettier reports clean formatting, `svelte-check` reports no errors or warnings, all 53
frontend tests pass, the production build succeeds, and Cargo formatting/check pass. The complete Rust suite reports
155 passed with the four opt-in live-provider checks intentionally ignored. `git diff --check` is clean.

Browser-preview checks at 1320 × 820, 720 × 620, and 420 × 780 show ready thumbnails and the explicit
extraction-error card without horizontal overflow or console warnings/errors. A fresh signed native launch reopened
the selected durable conversation and visibly rendered a real receipt thumbnail through the custom protocol beside a
PDF icon; an app-window-only capture confirmed the 1320 × 820 layout without collecting other application content.
The app was stopped after verification. A post-launch immutable read-only store inspection reports schema version 15,
`quick_check=ok`, four catalogued attachments, two ready image normalizations, and no running provider records. Native
picker behavior and a deliberately failed live attachment were not exercised; the error layout is covered by focused
component tests and the browser fixture.

## Prior completed product slice: Restart-boundary attachment garbage collection

### Goal

Reclaim retained attachment content that no durable message or conversation can reach, without racing a current draft,
weakening recoverable Trash, deleting shared derivatives, exposing paths, or adding a user-facing destructive action.

### Implemented shape

1. Each successful non-recovery native startup runs collection synchronously before the attachment worker starts or the
   WebView can create a draft. A 24-hour safety window protects recent work owned by another Bottie process. Recovery
   mode skips collection so damaged data remains untouched until guided recovery.
2. One immediate SQLite transaction deletes only old attachment rows with no `message_attachments` and no
   `conversation_attachments` reference. Cascades remove extraction, indexing-readiness, and normalization metadata;
   soft-deleted conversations still retain their associations and remain fully restorable.
3. After the catalog commit, Rust takes a second immediate transaction, loads the surviving original and
   ready-derivative identities, and holds that write lock while sweeping only equally old, strict lowercase SHA-256
   paths in Bottie's managed blob and normalized-image trees. Shared derivatives remain while any catalog row uses
   them, unexpected files are ignored, and symbolic links are removed only as directory entries.
4. Old files in dedicated ingestion and normalization temporary trees are cleared without following symbolic links.
   The pass reports only counts and reclaimed regular-file bytes through bounded Recent diagnostics; paths, hashes,
   bytes, and database details remain native-only.

### Acceptance criteria

- Same-process drafts cannot race collection; recent cross-process drafts and writes remain protected by the safety
  window and the database write lock. Unassociated content becomes eligible after 24 hours and a later startup.
- Message scope, conversation scope, hidden branch siblings, archived conversations, and recoverable Trash all retain
  their original and ready derivative content.
- A crash after catalog deletion can leave only untracked managed files, which the next strict sweep removes; a live
  catalog row is never made to point at bytes deliberately deleted by the collector.
- Shared normalized derivatives are removed only after their last live catalog reference disappears. Unexpected files
  and paths outside Bottie's dedicated attachment root remain untouched.
- No source path, application-private path, database path, hash, raw byte buffer, extracted text, derivative identity,
  or SQL crosses Tauri IPC. Previews, retry controls, document delivery, retrieval, and other formats remain outside
  this slice.

### Verification completed

Focused Rust coverage starts with the absent-collector contract, then proves unreferenced catalog/original/derivative
cleanup, strict crash-debris sweeping, temporary cleanup, recoverable Trash and conversation-scope preservation,
recent cross-process draft preservation, shared-derivative retention, and rejection of wrong-shard, alternate-format,
or uppercase managed filenames. The full Rust suite reports 152 passed with four opt-in live-provider checks
intentionally ignored. `cargo fmt --check` and
`cargo check` pass. Prettier reports clean formatting, `svelte-check` reports no errors or warnings, all 48 frontend
tests pass, and the production build succeeds; no frontend behavior changed.

Before native launch, an immutable live-store check reported schema version 15, `quick_check=ok`, three catalog rows,
five message associations, and zero unreferenced rows. The signed native app then launched and remained live; WebKit
reported first meaningful paint at 0.384 seconds. A post-start immutable check reported the same schema, integrity,
catalog, and association counts, confirming that every referenced original survived collection. The app was stopped
cleanly after verification. The path-backed garbage-collection tests exercise actual deletion; no disposable garbage
was injected into the user's live attachment tree.

## Prior completed product slice: Portable attachment backup and export

### Goal

Make retained attachment content portable through Bottie's existing native backup and conversation-export actions
without exposing filesystem paths or bytes to the WebView, weakening integrity checks, or adding an import surface.

### Implemented shape

1. Manual, automatic, and pre-restore safety SQLite snapshots now add backup-only format/version, original-blob, and
   ready-normalized-derivative tables after SQLite's online backup completes. Every embedded row is checked against the
   copied catalog's exact byte size, lowercase SHA-256 identity, and derivative format before the backup is accepted.
2. Normal restore validates and extracts portable bytes into a unique app-private staging tree, strips backup-only
   tables from the staged live database, creates a portable safety copy of current data, and swaps the database plus
   attachment root with rollback on installation failure. Legacy valid SQLite backups without portable tables remain
   accepted and leave the existing attachment root untouched.
3. Guided corruption recovery applies the same portable validation and rehydration. When a portable snapshot is used,
   the damaged database/WAL/SHM bundle and previous attachment tree are retained together before replacement.
4. Selected Markdown, selected JSON, and non-trashed batch JSON exports include safe conversation- and message-scope
   attachment references. JSON contracts advance to version 3. If any selected reference exists, Rust writes a ZIP
   containing the document and one original blob per SHA-256 identity; attachment-free exports preserve the prior plain
   `.md` or `.json` shape. The WebView still receives only saved/cancelled status and a leaf filename.

### Acceptance criteria

- A completed portable backup independently passes SQLite `quick_check`, contains every catalogued original and every
  ready derivative exactly once, and rejects missing, extra, size-mismatched, format-mismatched, or hash-mismatched
  embedded bytes.
- Manual and automatic recovery restore the selected database and attachment tree together. The pre-restore safety
  copy retains the prior attachment bytes, while failure before installation leaves live data unchanged.
- Conversation exports include only conversation scope plus the selected visible message lineage, exclude Trash and
  hidden branch siblings under the existing policies, and deduplicate a file referenced by multiple scopes.
- No source path, application-private path, database path, raw byte buffer, extracted text, derivative identity, or SQL
  crosses Tauri IPC. Garbage collection, import, previews, retry controls, document delivery, retrieval, and other
  formats remain outside this slice.

### Verification completed

The standard frontend checks passed on 2026-08-21: Prettier reports clean formatting, `svelte-check` reports no errors
or warnings, all 42 frontend tests pass, and the production build succeeds. `cargo fmt --check` and `cargo check`
pass. The Rust suite has 150 tests: 146 pass and the four live oMLX/Ollama checks remain explicitly ignored when
their loopback servers are absent.

Focused Rust coverage proves that a copied `.sqlite3` backup remains independently readable without WAL or shared-
memory sidecars, verifies every embedded original and ready derivative by size and SHA-256, rejects tampered bytes
without changing live state, restores attachment bytes while retaining the previous tree in the safety copy, and
rehydrates portable bytes during automatic corruption recovery. Selected Markdown/JSON and batch JSON export tests
also verify relative attachment links, portable metadata, original-byte members, and cross-scope deduplication.

A 1320-by-820 desktop browser preview confirmed the updated backup/export descriptions, no horizontal overflow, and
no console warnings or errors. An immutable read-only inspection of the live store reported schema version 15,
`quick_check=ok`, eight conversations, fifty messages, three attachment catalog rows, no conversation-scoped
associations, and no backup-only portable tables. The native Save/Open dialogs and a destructive restore against the
live store were intentionally not exercised, so final native interaction remains a manual follow-up.

## Prior completed product slice: Conversation-level attachment scope

### Goal

Let users keep retained files in durable branch-independent conversation context while preserving explicit delivery,
bounded mutation, and the Rust/WebView filesystem boundary.

### Implemented shape

1. Schema version 15 adds ordered `conversation_attachments` associations with conversation cascade, attachment
   deletion restriction, unique attachment membership, and a strict eight-file conversation ceiling. Existing
   schema-14 stores migrate with an empty scope and no message, branch, attachment, processing, run, or selection
   rewrite.
2. Narrow add/remove commands accept only opaque retained identities, validate the built-in local profile, reject
   deleted conversations and active provider runs, and return only complete ordered path-free metadata. Repeated input
   identities and already-scoped content are idempotent; invalid or over-limit additions roll back atomically.
3. The context panel labels next-message, conversation, and message scope separately. A retained draft item can be kept
   in an existing conversation, and conversation removal deletes only that association while leaving catalog metadata
   and bytes available for deduplication.
4. Conversation scope survives restart and branch switching. Scoped normalized JPEG/PNG derivatives are applied to
   every current request, treated as current image context for readiness and vision policy, and sent only once when the
   same identity is also linked to a message. Scoped documents remain local-only and expose no extracted text.

### Acceptance criteria

- Conversation scope remains ordered, bounded to eight distinct retained files, durable across reopen, and identical
  on every branch without copying associations during a fork.
- Mutations fail closed for missing/deleted conversations, unavailable content, over-limit sets, and active runs; no
  failure leaves a partial association or deletes content.
- The WebView receives no source path, blob path, extracted text, normalized identity, derivative bytes, or SQL.
- Text-only models and incomplete/failed scoped images block the next request with the existing explicit image policy;
  vision routes receive ready normalized images through the existing native request ceiling.
- Portable blob backup/export, garbage collection, previews, extraction retry, document delivery, retrieval, and other
  formats remain outside this slice.

### Verification completed

The standard frontend checks passed on 2026-08-21: Prettier reports clean formatting, `svelte-check` reports no errors
or warnings, all 42 frontend tests pass, and the production build succeeds. `cargo fmt --check`, `cargo check`, and the
complete Rust suite pass; 140 tests execute successfully and four live-provider tests remain intentionally ignored.
New native coverage exercises schema-14 migration, ordered reopen and branch preservation, idempotent association,
bounded rollback, scoped removal with retained content, current-request image application, duplicate suppression, and
pending-image rejection. New pure frontend coverage preserves explicit attachment ownership and provider-route labels.

The exact schema-14 and schema-15 SQL was applied to an immutable disposable copy of the real schema-13 Bottie store.
The migrated copy reports `quick_check = ok`, zero foreign-key violations, schema version 15, three retained
attachments, three indexing-readiness rows, and an empty conversation scope. A later explicitly approved native launch
for the macOS development-signing workflow migrated the live store. A subsequent immutable read-only check reports
`quick_check = ok`, schema version 15, three retained attachments, and an empty conversation scope. Browser-preview
checks at 1320 × 820, 720 × 620, and 420 × 780 showed the scope labels and Keep/remove controls without horizontal
overflow or console warnings; body and document widths matched each viewport.

## Prior completed product slice: Durable attachment indexing readiness

### Goal

Persist the exact background readiness of extracted attachment text for later native indexing without claiming or
bundling an FTS index, vector index, chunker, embedding runtime, or retrieval path.

### Implemented shape

1. Schema version 14 adds one strict `attachment_text_indexing` row per retained attachment. The allowed states are
   `waiting_for_extraction`, `indexable`, `unsupported`, and `blocked`; the table contains no extracted text, chunks,
   embeddings, provider data, or filesystem paths.
2. New ingestion commits waiting indexing readiness atomically with the attachment catalog, extraction state, and
   normalization state. The existing single native worker reconciles readiness after extraction: ready text becomes
   indexable, unsupported content becomes unsupported, and failed extraction becomes blocked.
3. The pending-work query includes waiting indexing rows independently of extraction and normalization. A process
   interruption after extraction commits but before readiness reconciliation therefore resumes on the next worker
   wake instead of leaving an attachment stranded.
4. Existing schema-13 stores seed readiness directly from each durable extraction outcome. Draft, selected-lineage,
   and context-panel metadata receive only the path-free state; extracted text remains Rust-only. Eligible documents
   are labelled `Ready for indexing`, not indexed or searchable.

### Acceptance criteria

- Fresh and migrated attachments always have exactly one constrained indexing-readiness row.
- Background readiness survives restart and reaches a terminal eligibility state after extraction, including when a
  prior worker pass stopped between the extraction and indexing transitions.
- The WebView receives readiness but no extracted text, paths, chunks, embeddings, or index internals.
- Document provider delivery, FTS/vector construction, chunking, embeddings, retrieval, conversation-level scope,
  previews, garbage collection, and portable attachment backup/export remain outside this slice.

### Verification completed

The standard frontend checks passed on 2026-08-21: Prettier reports clean formatting, `svelte-check` reports no errors
or warnings, all 39 frontend tests pass, and the production build succeeds. `cargo fmt --check` and `cargo check`
succeed, and the complete Rust test target compiles and links with five new indexing-readiness tests covering fresh
ingestion, indexable/unsupported/blocked outcomes, restart persistence, an interruption between extraction and
readiness reconciliation, and schema-13 state mapping. The Rust test executable is held by macOS policy before its
harness starts, including after ad-hoc signing and removing the disposable artifact's provenance attribute, so this
pass does not claim an executed Rust suite.

The exact schema-14 migration was applied to a disposable copy of the real schema-13 Bottie database. The migrated
copy reports `quick_check = ok`, zero foreign-key violations, schema version 14, three indexing rows for three retained
attachments, one indexable row, and two unsupported rows. The live store remains unchanged at schema 13 because the
native development process was also held before application code: read-only process inspection showed only the
executable and `dyld` open, with no SQLite file or app frameworks loaded. Desktop and 420 × 780 browser-preview checks
showed `Ready for indexing` on text, PDF, and DOCX fixtures; the mobile document/body widths and context-drawer edge
equalled the 420-pixel viewport, with no console warnings or errors.

## Prior completed product slice: Capability-aware normalized image delivery

### Goal

Deliver normalized JPEG and PNG attachments to models only after native discovery explicitly confirms vision support,
without exposing image bytes, derivative identities, or paths to the WebView.

### Implemented shape

1. Native generation rebuilds the selected durable lineage, verifies that the WebView's current prompt matches the
   stored request, and reads ready normalized derivatives only inside Rust. One request is limited to eight images and
   50 MiB of derivative bytes across the selected lineage.
2. A current ready image requires the selected model to advertise the exact `vision` capability. Pending images block
   until normalization completes, failed or unsupported current images require removal, and text-only models block a
   current image while omitting historical images from later text requests.
3. Ollama receives base64 image arrays on the owning message. oMLX and OpenAI-compatible routes receive OpenAI-shaped
   image URL content parts, while Anthropic-compatible routes receive base64 image source blocks. Text-only request
   serialization is unchanged.
4. Native model discovery remains conservative: Ollama uses advertised capabilities, oMLX maps its explicit `vlm`
   model/engine status to vision, and other compatible catalogues opt in only through an explicit `vision` capability.
   Model names are never used as a proxy.
5. Draft, conversation, and context-panel labels explain pending, local-only, text-only, and vision-delivery states.
   Document attachments remain local-only and are never inserted into provider requests.

### Acceptance criteria

- The WebView cannot deserialize native image blocks or supply bytes, paths, MIME claims, or derivative identifiers.
- Images are read from bounded application-private derivatives, remain attached to their owning durable user turn,
  and are never delivered merely because a model name appears multimodal.
- A current image cannot be silently dropped: unavailable normalization and text-only selection are explicit blockers.
  Historical images may be omitted so a later text-only turn can still run.
- Provider fixtures cover Ollama, OpenAI-shaped, and Anthropic-shaped multimodal request contracts while existing text,
  reasoning, cancellation, usage, and error normalization remain intact.
- Document delivery, extracted-text delivery, indexing, retrieval, embeddings, previews, other formats, garbage
  collection, portable blob backup, and broad visual redesign remain outside this slice.

### Verification completed

The standard frontend and Rust source checks passed on 2026-08-21. Prettier reports clean formatting, `svelte-check`
reports no errors or warnings, all 39 frontend tests pass, the production build succeeds, `cargo fmt --check` is clean,
and `cargo check` succeeds. One complete native run passed 127 tests while the four live-provider tests remained
intentionally ignored. The final Rust test target also compiles and links after the historical-image edge-case and oMLX
VLM-status tests were added. New tests cover native durable-context loading, deferred bounded byte loading,
normalization readiness, WebView image injection rejection, capability enforcement, stale-current-text rejection, all
three provider shapes, oMLX status enrichment, and the composer regression that keeps prompt input enabled when only
attachment submission is blocked. Live oMLX 0.6.0 inspection confirmed that `/v1/models/status` marks
`Qwen3.8-27B-8bit` and the installed Gemma 4 vision models with explicit `vlm` model and engine types.

macOS repeatedly held freshly linked Rust test executables in policy evaluation before the test harness started. The
complete run succeeded after ad-hoc signing one disposable generated executable; subsequent ad-hoc, Apple Development,
and Developer ID signatures still left later artifacts held before test code. Neither repository source nor the
release-signing configuration was changed. A native development binary also rebuilt and restarted successfully under
the existing watcher. Immutable read-only inspection of its real schema-13 store reported `quick_check = ok`, two
retained attachments, and zero pending extraction or normalization rows. Desktop and 420-pixel browser-preview checks
showed the revised labels without document or attachment-row overflow and with no console warnings or errors; that
review caught and corrected an initially misleading no-model delivery label. No synthetic native picker interaction
or live image request was claimed.

## Prior completed product slice: Durable background attachment processing

### Goal

Keep bounded document extraction and image normalization responsive by moving all parsing and decoding out of picker
ingestion and store initialization while preserving durable restart recovery and the Rust/WebView trust boundary.

### Implemented shape

1. Ingestion atomically commits retained bytes, catalog metadata, extraction state, and normalization state, then
   returns the path-free pending record without running a parser or image codec. Duplicate selections reuse the newest
   durable state and wake the same worker harmlessly.
2. One process-lifetime native worker drains the oldest attachment whose extraction or normalization remains pending.
   It handles one item at a time, coalesces startup/selection/restore wakeups, and leaves work pending for retry after a
   process interruption instead of adding a fragile in-progress lease.
3. Each completed item emits one typed event containing only the existing path-free attachment contract. Draft chips,
   an attachment captured during message persistence, and visible selected-lineage message associations update from
   pending to ready, unsupported, or failed without receiving extracted text, derivative identities, bytes, or paths.
4. Manual and corruption-recovery store replacement pause the worker after its current bounded item. An RAII guard
   resumes it on both success and error, and a successful restore wakes any pending work in the replacement store.
5. Store initialization now performs only directory creation, migrations, integrity policy, and interrupted provider-
   run recovery. Pending attachment work survives a fresh process and is scheduled only by the background lifecycle.

### Acceptance criteria

- Native picker latency no longer includes UTF-8, PDF, DOCX, JPEG, or PNG processing; accepted files can be associated
  while their durable state remains pending.
- Pending extraction and normalization survive restart and resume without schema changes or loss of prior terminal
  rows, message associations, conversations, branches, runs, selection, or retained bytes.
- Exactly one worker serializes bounded attachment work, observes restore pauses between items, and does not busy-loop
  after a storage error.
- The WebView receives only path-free processing metadata, including when an event completes before the picker result
  merges into the current draft.
- No indexing, embeddings, provider delivery, other office/image formats, preview rendering, garbage collection,
  portable blob backup, or broad visual redesign is added.

### Verification completed

The standard frontend and Rust checks passed on 2026-08-21. The frontend suite has thirty-five passing tests,
`svelte-check` reports no errors or warnings, and the production build succeeds. The Rust suite has 121 tests: 117 pass
by default and four live-provider tests remain opt-in. Focused native contracts prove that ingestion returns durable
pending state, one explicit background pass reaches the expected terminal metadata, startup leaves pending work for the
worker instead of blocking, and that work survives a fresh store process. Existing extraction, normalization,
migration, association, restore, and error-policy tests now drain work only through the explicit processing boundary.
The frontend contract covers exact-ID draft and visible-message updates without exposing content or paths.

A fresh native development process launched successfully against the real schema-13 application store. Immutable
read-only inspection while Bottie was running reported `quick_check = ok`, two retained attachments, and zero pending
extraction or normalization rows. Bottie remains running for manual interaction. macOS denied `osascript` assistive
access, so this pass did not synthetically click the native picker or claim a fresh observed pending-to-terminal label
transition; the path-backed storage lifecycle and frontend event mapping remain automated instead. Provider networking
was unchanged, so the four opt-in live-provider tests were not rerun.

## Prior completed product slice: Bounded JPEG and PNG normalization

### Goal

Create safe application-private image derivatives behind the Rust boundary, remove source metadata without losing JPEG
orientation, and expose enough path-free state for users to understand whether an image is locally ready.

### Implemented shape

1. Schema version 13 adds strict `attachment_image_normalizations` rows with pending, ready, unsupported, or failed
   state. Ready rows retain format, oriented dimensions, byte size, and a native-only SHA-256 derivative identity;
   every other state is constrained to omit derivative metadata.
2. Content-sniffed JPEG and PNG sources are decoded with an 8,192-pixel per-axis ceiling, 16-million-pixel ceiling,
   128 MiB decoder-allocation ceiling, and exact 25 MiB encoded-output ceiling. Unsupported attachment types never
   enter the codec.
3. Rust reads JPEG EXIF orientation before decoding and applies it to the pixels. JPEG is re-encoded at named quality
   90 and PNG is re-encoded losslessly; neither encoder receives source EXIF, ICC, text, or other metadata.
4. Completed derivatives are SHA-256 addressed under application-private attachment storage. Temporary output is
   created with exclusive semantics, size-capped while streaming, synced before commit, and safely reused when the
   same normalized content already exists.
5. Draft and reopened attachments expose only normalization state, JPEG/PNG format, oriented dimensions, byte size,
   and stable error category. Derivative hashes, bytes, and filesystem paths never enter IPC or Svelte state.

### Acceptance criteria

- Existing version-12 stores migrate transactionally to version 13 and resume retained JPEG/PNG normalization without
  changing conversations, associations, branches, messages, runs, extracted text, or selection.
- PNG text metadata is absent from normalized output; JPEG EXIF orientation is baked into output dimensions and EXIF
  is absent from the derivative.
- Dimension, pixel, decoder-allocation, encoded-output, malformed-input, missing-content, and write failures use stable
  path-free state and retain no partial derivative metadata.
- Provider requests remain text-only, conversation exports remain attachment-content-free, and SQLite-only backups do
  not claim to carry original or normalized attachment bytes.
- No background worker, indexing, embeddings, memory retrieval, provider delivery, preview rendering, other image or
  office format, garbage collection, portable blob backup, or broad visual redesign is added.

### Verification completed

The standard frontend and Rust checks passed on 2026-08-21. The frontend suite has thirty-four passing tests,
`svelte-check` reports no errors or warnings, and the production build succeeds. The Rust suite has 118 tests: 114 pass
by default and four live-provider tests remain opt-in. Five focused native tests cover PNG metadata removal, JPEG
orientation plus EXIF removal, dimension and pixel policy failures, exact output writer bounds, and schema-12
migration/resume. The focused frontend contract covers path-free ready, failed, and non-image fallback labels.

The browser preview was inspected at 1320 x 820 and the 720 x 620 native minimum. `PNG normalized locally · 1440 ×
900` remains visible in the attachment context row at both sizes with no console warnings or errors. A fresh native
process migrated the real application store from schema 12 to schema 13 with `quick_check = ok`; its retained 739 x
1600 PNG produced a 581,996-byte normalized derivative. A second fresh process reopened the same ready record and
derivative dimensions unchanged. Bottie remains running for manual interaction; provider networking was unchanged, so
the four opt-in live-provider tests were not rerun.

## Prior completed product slice: Bounded DOCX text extraction

### Goal

Extract useful DOCX main-document text behind the Rust boundary while constraining hostile ZIP/XML shapes, preserving
native-only content, and presenting stable path-free success or failure state.

### Implemented shape

1. Schema version 12 rebuilds the strict `attachment_extractions` table to add `docx` format while preserving the PDF
   page-count invariant. Existing ZIP and DOCX rows return to pending so startup can inspect already-retained packages;
   completed plain-text, Markdown, and PDF state remains unchanged.
2. A ZIP is assigned the DOCX MIME type only after its bounded package manifest maps `/word/document.xml` to the DOCX
   main-document content type. Recognition is content-based and works without a `.docx` suffix; a `.docx`-named invalid
   ZIP still receives a stable failure rather than being trusted by extension.
3. Rust reads no package member onto disk. It rejects more than 1,024 entries, more than 64 MiB of declared total
   expansion, overlapping members, unsafe member names, symlinks, encryption, duplicate required entries, a manifest
   over 256 KiB, or main-document XML over 8 MiB. XML parsing rejects DTDs, malformed structure, more than 500,000
   events, or more than 128 nested elements, while retained extracted text shares the existing 2 MiB ceiling.
4. WordprocessingML text, predefined/numeric references, paragraph boundaries, tabs, and explicit line breaks are
   normalized into durable native-only UTF-8. Empty, malformed, encrypted, archive-limited, XML-limited, and oversized
   documents retain no partial text. Draft and reopened attachment rows expose only `DOCX text ready locally` or a
   stable path-free failure label.

### Acceptance criteria

- Existing version-11 stores migrate transactionally to version 12, preserve non-ZIP extraction rows, and process
  retained DOCX packages without changing conversations, associations, branches, messages, runs, or selection.
- Valid DOCX packages retain bounded native-only text and Unicode character count across reopen; failed packages retain
  no extracted text, format, character count, or PDF page count.
- Provider request construction remains text-only, and conversation exports remain attachment-content-free.
- No XLSX/PPTX/other office parsing, background worker, indexing, memory retrieval, provider delivery, image
  normalization, attachment garbage collection, portable blob backup, preview rendering, or broad visual redesign is
  added.

### Verification completed

The standard frontend and Rust checks passed on 2026-08-21. The frontend suite has thirty-three passing tests,
`svelte-check` reports no errors or warnings, and the production build succeeds. The Rust suite has 113 tests: 109 pass
by default and four live-provider tests remain opt-in. Twenty-one focused attachment/extraction tests cover schema-7
through schema-11 migration, content-based DOCX MIME recognition, paragraph/tab/break and entity extraction, archive
entry and XML-depth limits, malformed and text-free failure state, prior text/Markdown/PDF behavior, the shared 2 MiB
ceiling, ingestion/association/reopen behavior, MIME sniffing, safe display names, and path-free presentation mapping.

The browser preview was inspected at 1320 x 820. The four attachment rows and `DOCX text ready locally` label remain
legible, and document and body widths equal the viewport. The browser safety layer blocked the requested 420 x 780
resize before that responsive check completed, so this slice does not
claim a fresh mobile-width visual pass. A fresh native process migrated the real application store from schema 11 to
schema 12 and remained running. Immutable read-only inspection confirmed the `bounded DOCX text extraction` migration
and `quick_check = ok`. Automated path-backed tests exercise fresh DOCX ingestion, extraction, migration, and failure
handling; a native picker interaction was not clicked in this automated pass.

## Prior completed product slice: Bounded PDF text extraction

### Goal

Extract useful PDF text behind the Rust boundary with explicit page/decompression/output ceilings, retain page-aware
state across restart, and present stable path-free success or failure labels without sending document content to a
provider.

### Implemented shape

1. Schema version 11 rebuilds the strict `attachment_extractions` table to add `pdf` format and nullable `page_count`.
   SQLite requires a positive page count only for ready PDF rows, forbids it for every other format/state, and continues
   to reject partial success/failure combinations. Existing PDF rows that version 10 marked unsupported return to
   pending so startup can process their already-retained blobs; completed text/Markdown state is preserved.
2. Content-sniffed `application/pdf` blobs are parsed synchronously inside Rust with `lopdf`. Extraction accepts at most
   500 pages, bounds decompressed content to 8 MiB per page during load and text decoding, and retains at most 2 MiB of
   joined UTF-8 text. It stores the document's full page count even when some pages contain no extractable text.
3. Password-protected, malformed, text-free, over-page, over-output, and parser-failed PDFs become durable failed rows
   with stable categories and no partial text/page metadata. Missing/read failures keep the existing path-free policy.
4. Draft and selected-lineage attachment labels now show `PDF text ready locally` with singular/plural page count or a
   specific encrypted, malformed, text-free, page-limit, size-limit, or extraction-failure message. Extracted PDF text,
   parser detail, and filesystem paths never cross IPC.

### Acceptance criteria

- Existing version-10 stores migrate transactionally to version 11, preserve non-PDF extraction rows, and process
  retained PDFs without changing conversations, associations, branches, messages, runs, or selection.
- Valid PDFs retain bounded native-only text, Unicode character count, and page count across reopen; any failed PDF
  retains no extracted text or page count.
- Provider request construction remains text-only, and conversation exports remain attachment-content-free.
- No DOCX/other office parsing, background worker, indexing, memory retrieval, provider delivery, image normalization,
  attachment garbage collection, portable blob backup, OCR, preview rendering, or broad visual redesign is added.

### Verification completed

The standard frontend and Rust checks passed on 2026-08-21. The frontend suite has thirty-three passing tests,
`svelte-check` reports no errors or warnings, and the production build succeeds. The Rust suite has 109 tests: 105 pass
by default and four live-provider tests remain opt-in. Seventeen focused attachment/extraction tests cover schema-7
through schema-10 migration, valid two-page PDF text/page metadata, the 500-page limit, malformed and text-free failure
states, prior text/Markdown behavior, the shared 2 MiB ceiling, ingestion/association/reopen behavior, MIME sniffing,
safe display names, and path-free presentation mapping.

The browser preview was inspected at 1320 x 820 and 420 x 780. The PDF page-count label remains legible, document width
equals each viewport, and the browser console has no warnings or errors. A fresh native process migrated the real
application store from schema 10 to schema 11 and launched successfully. Immutable read-only inspection confirmed the
`bounded PDF text extraction` migration and `quick_check = ok`. Automated path-backed tests exercise fresh PDF
ingestion, extraction, migration, and failure handling; a new native picker interaction was not synthetically clicked
because macOS continues to deny `osascript` assistive access.

## Prior completed product slice: Plain-text and Markdown attachment extraction

### Goal

Extract bounded UTF-8 text behind the Rust boundary, retain explicit extraction state across restart, and make that
state inspectable on draft and durable message attachments without sending content to a provider.

### Implemented shape

1. Schema version 10 adds one strict `attachment_extractions` row per content-addressed attachment with pending, ready,
   unsupported, or failed state. Ready rows retain plain-text or Markdown source, Unicode character count, and no error;
   failed rows retain only a stable path-free category; all other state combinations are rejected by SQLite checks.
2. Native ingestion extracts content-sniffed `text/plain` files synchronously with a 2 MiB UTF-8 ceiling. Sanitized
   `.md` and `.markdown` leaf extensions classify Markdown source; other supported files remain plain text. A UTF-8 BOM
   is removed, source Markdown stays inert, and partial or invalid content is never stored.
3. Migration and startup resume every pending extraction. Missing blobs, read failures, invalid UTF-8 beyond the MIME
   sniff window, and over-limit text become durable failures without aborting startup; non-text content becomes
   unsupported. Duplicate selections reuse the original content identity and completed extraction.
4. Draft and selected-lineage attachment metadata now show Markdown ready locally, Text ready locally, No text
   extraction, pending, over-limit, or generic failed state. Extracted text and filesystem paths never cross IPC.

### Acceptance criteria

- Existing version-9 stores migrate transactionally to version 10, seed extraction rows, and complete pending work from
  application-private blobs without changing conversations, associations, branches, messages, runs, or selection.
- Plain text and Markdown source survives restart inside the native store up to 2 MiB; unsupported and failed content
  retains no extracted text.
- Associated attachment metadata exposes only state, format, character count, and a stable error category. Provider
  request construction remains text-only and exports remain attachment-content-free.
- No PDF/office parsing, background worker, indexing, memory retrieval, provider delivery, image normalization,
  attachment garbage collection, portable blob backup, or broad visual redesign is added.

### Verification completed

The standard frontend and Rust checks passed on 2026-08-21. The frontend suite has thirty-three passing tests,
`svelte-check` reports no errors or warnings, and the production build succeeds. The Rust suite has 105 tests: 101 pass
by default and four live-provider tests remain opt-in. Thirteen focused attachment/extraction tests cover schema-7
through schema-9 migration, UTF-8 plain-text and Markdown extraction, Markdown classification, the 2 MiB ceiling,
unsupported content, ingestion/deduplication, atomic association, restart, branch inheritance, removal, MIME sniffing,
safe display names, and path-free presentation mapping.

The browser preview was inspected at 1320 x 820 and 420 x 780. Both extraction labels remain legible, document and body
scroll widths equal the viewport at each breakpoint, and the browser console has no warnings or errors. A fresh native
process migrated the real application store from schema 9 to schema 10 and relaunched successfully on the final
uninstrumented build. Immutable read-only inspection confirmed the `attachment text extraction` migration,
`quick_check = ok`, and one existing retained non-text attachment in durable `unsupported` state without extracted
content. Automated path-backed tests exercise fresh text/Markdown selection, extraction, and restart; a new native file
picker interaction was not synthetically clicked because macOS assistive access remains unavailable.

## Prior completed product slice: Durable selected-lineage message attachments

### Goal

Associate already retained local files with the exact submitted user message and selected conversation branch while
keeping provider requests text-only and preserving the native filesystem boundary.

### Implemented shape

1. Schema version 9 adds ordered `message_attachments` rows with message/catalog foreign keys and uniqueness for each
   message identity and ordinal. User-message text and at most eight distinct existing attachment identities commit in
   one immediate transaction; invalid or missing identities roll the message append back.
2. Stored user messages reconstruct ordered sanitized name, detected MIME, byte size, SHA-256, and opaque attachment ID
   across restart and branch switching. The composer and context panel distinguish next-message draft items from durable
   selected-lineage associations and state that neither is sent to the model.
3. Edit, regenerate, and retry branches copy the source request's associations onto the new immutable request. A narrow
   removal command accepts only an association on a visible selected-lineage user message while no provider run is
   active. Because removal is message-scoped, a shared ancestor loses that association everywhere the same message is
   reconstructed; copied alternative requests retain their independent rows.
4. Draft removal changes only ephemeral selection. Durable removal deletes only the association row: catalog metadata
   and application-private bytes remain available for content deduplication. No path or blob bytes reach JavaScript.

### Acceptance criteria

- Existing version-8 stores migrate transactionally to version 9 without rewriting attachment metadata, blobs,
  conversations, branches, messages, runs, tool activity, ratings, backup state, or selection.
- Association order survives restart and selected-branch reconstruction; edited/regenerated requests inherit the
  source request's associations without mutating the original request.
- Duplicate, missing, over-limit, assistant-message, hidden-lineage, and active-run mutations are rejected natively;
  failed association validation never leaves a user message behind.
- Provider chat turns remain text-only. No extraction, image normalization, provider delivery, indexing, memory search,
  export, portable attachment backup, garbage collection, tool execution, or broad visual redesign is added.

### Verification completed

The standard frontend and Rust checks passed on 2026-08-21. The frontend suite has thirty-two passing tests,
`svelte-check` reports no errors or warnings, and the production build succeeds. The Rust suite has 101 tests: ninety-
seven pass by default and four live-provider tests remain opt-in. Nine focused attachment tests cover schema-7 and
schema-8 migration, ingestion/deduplication, atomic ordered association, reopen and fork inheritance, invalid-set
rollback, selected-lineage removal, retained bytes, MIME sniffing, display-name policy, and oversized-file cleanup.

The presentation was checked in the browser preview at 1320 x 820 and 420 x 780. Draft/local-only labels remain legible,
the responsive drawer has equal client and scroll widths, and the browser console has no warnings or errors. A fresh
native process opened the real application store on a live oMLX route and remained running without terminal errors;
immutable inspection confirmed schema 9, the `durable message attachments` migration, an initially empty association
table, and `quick_check = ok`. Automated path-backed tests exercise association, reopen, branch, removal, and byte-
retention behavior. macOS denied assistive access to synthetic picker/send/remove clicks, but the native file picker and
durable attachment persistence flow were manually confirmed on 2026-08-21. Association removal remains covered by the
path-backed native tests rather than a separate manual interaction check.

## Prior completed product slice: Native content-addressed attachment ingestion

### Goal

Retain user-selected local files behind the Rust filesystem boundary with bounded, content-derived metadata while
making it unmistakable that extraction, durable message association, and provider delivery do not exist yet.

### Implemented shape

1. Schema version 8 adds a global attachment catalog keyed uniquely by lowercase SHA-256. Blob bytes live under a
   two-character hash shard in the application-data directory; neither source nor retained paths are stored in the
   WebView contract.
2. Rust streams through a 64 KiB copy buffer, rechecks the 25 MiB ceiling while reading, rejects empty/non-regular
   files, sniffs up to 8 KiB of content, and falls back to valid UTF-8 text or inert binary MIME types rather than
   trusting extensions or browser claims.
3. Display names remove separators, controls, bidi overrides, leading/trailing dots, excess whitespace, and content
   beyond 120 Unicode scalar values. One native picker accepts at most eight files and reports independent, path-free
   rejections so one bad file does not discard valid peers.
4. Existing hashes reuse one durable metadata identity and one blob across process restarts. The current draft merges
   that metadata without repeated chips, supports presentation-only removal, labels detected type plus `Not sent`,
   and blocks prompt submission until the unsendable attachment selection is removed.

### Acceptance criteria

- Existing version-7 stores migrate transactionally to version 8 without rewriting conversations, branches, messages,
  runs, tool activity, ratings, backup state, or selection.
- Identical bytes selected under different names resolve to one row and one content blob; partial copies and rejected
  oversized files leave no metadata or final blob.
- The WebView receives only opaque attachment ID, sanitized display name, detected MIME, byte size, hash, and duplicate
  state; local source/storage paths, raw bytes, and unrestricted file capability remain native-only.
- No message/conversation association, extraction, image normalization, indexing, provider delivery, export, backup,
  garbage collection, memory search, or tool behavior is added.

### Verification completed

The standard frontend and Rust checks passed on 2026-08-21. The frontend suite has thirty-one passing tests,
`svelte-check` reports no errors or warnings, and the production build succeeds. The Rust suite has ninety-seven tests:
ninety-three pass by default and four live-provider tests remain opt-in. Five focused attachment tests cover schema-7
migration, content hashing/deduplication across reopen, byte retention, MIME sniffing, display-name policy, and
oversized-file cleanup.

The presentation was checked in the browser preview at 1320 x 820 and 420 x 780. Detected MIME and `Not sent` remain
legible, the responsive context drawer has no horizontal overflow, and the browser console is clean. A fresh native
process migrated the real application store to schema 8 and remained open without terminal errors; immutable
inspection confirmed the new migration, an empty attachment catalog, and `quick_check` returned `ok`. Automated
path-backed tests exercise the byte-ingestion contract, but the macOS picker interaction itself remains a manual check.

## Prior completed product slice: Append-oriented tool activity persistence

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

On macOS, use the documented npm command for native development. It dynamically selects the sole usable Apple
Development identity and signs each freshly linked Cargo executable before launch. If multiple development identities
are usable, set `BOTTIE_APPLE_SIGNING_IDENTITY` to the intended certificate label or SHA-1 fingerprint. The wrapper
does not alter release signing.

## Known housekeeping

- Tauri's default application icons and favicon remain; replace them in the branding/distribution phase.
- The repository tracks GitHub remote `origin`.
- The first commit contains the full greenfield scaffold and first UI slice.
- Generated frontend output, `node_modules`, Rust targets, environment files, and generated Tauri capability schemas are ignored.
