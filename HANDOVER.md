# bottie handover

Last verified: 2026-08-24

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
provider call identities. An explicit Memory composer toggle now lets tool-capable oMLX, Ollama, OpenAI-compatible,
and Anthropic-compatible models use the three native memory tools through their distinct function wire shapes,
Bottie's bounded multi-round state machine, and those durable tool records. An explicitly capable oMLX endpoint uses the
same Bottie-owned clock, Memory, and Web loops without invoking its MCP or server-side tool routes. Native attachment
selection now streams chosen local files into application-private content-addressed storage with SHA-256 identities,
content-based MIME
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
blocks. Documents remain excluded from automatic provider context. Extracted text now also moves through durable
waiting-for-extraction, indexable,
unsupported, or blocked readiness in the same resumable native worker. Indexable is eligibility for native derived
memory construction, not provider delivery. Up to eight retained
files can now be kept in durable conversation scope independently of any branch or message. The interface distinguishes
next-message, conversation, and message associations and supports narrow removal without deleting retained content.
Conversation-scoped normalized images apply to every current request, require explicit vision capability, and are
deduplicated when the same file is also linked to a message; explicitly enabled tool-capable oMLX, Ollama,
OpenAI-compatible, or Anthropic-compatible requests may retrieve bounded document excerpts. Portable backups now
retain every original blob and ready normalized
derivative, while selected and batch exports bundle only referenced originals
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
lexical retrieval without consuming the result limit on ineligible nearer vectors. One native hybrid query now applies
a shared filter and bound contract to both retrieval paths and combines their source ranks with reciprocal-rank fusion.
Each source contributes at most once per list; an overlapping source retains its strongest semantic chunk's exact
excerpt and offsets. Settings now exposes durable path-free semantic-index progress and one explicit reindex action.
The native command serializes with restore, pauses the worker before atomically removing only derived vector mappings,
retains source chunks and the application-owned model cache, then resumes the existing bounded worker. A typed native
`search_memory` contract now applies hybrid retrieval to final conversation messages.
It accepts bounded query, conversation, inclusive-date, and result-limit arguments, returns ranked excerpts with
path-free conversation/message provenance and optional exact chunk offsets, and hides engine scores and embedding
details. A matching Rust-owned `open_memory` contract now resolves that opaque provenance into the matched message's
own immutable branch lineage without changing the selected branch. It returns at most three final text turns on each
side, caps each turn at 2,000 Unicode scalars, retains Archived conversations, excludes Trash and non-final responses,
and omits reasoning, provider details, attachments, and native paths. Automatic retrieval injection remains absent;
only an explicitly enabled tool-capable oMLX, Ollama, OpenAI-compatible, or Anthropic-compatible request may call the
native memory tools. A
matching Rust-owned `search_attached_files` contract now applies hybrid retrieval only to
ready extracted documents with durable active or Archived conversation/message associations. It returns bounded
excerpts with safe file metadata and optional exact chunk offsets while omitting hashes, paths, scores, embeddings,
full extracted text, and association internals. A single native definition set now exposes only `search_memory`,
`open_memory`, and `search_attached_files` with closed JSON schemas and converts raw provider-style JSON into exact
typed arguments. It rejects unsupported names, non-objects, missing or unknown fields, JSON null/type mismatches,
blank or overlong strings, contradictory date ranges, and out-of-range result/window counts without reflecting raw
arguments in errors. A provider-neutral native dispatcher now validates and executes those three contracts through one
structured success/error envelope. Successful envelopes have a 64 KiB serialized ceiling; failures use stable
`unsupported_tool`, `invalid_arguments`, `unavailable`, `execution_failed`, or `output_too_large` categories without
forwarding query, argument, embedding, storage, or path details. A provider-neutral native state machine now executes
repeated batches through that dispatcher while correlating opaque call identities. One generation is bounded to eight
calls, four tool rounds, 256 KiB of aggregate serialized output, and five minutes; cancellation and deadline checks run
before and after every native call, and every exceptional outcome closes the loop. oMLX, Ollama, OpenAI Chat
Completions, and Anthropic Messages now map the three closed definitions, accumulate streamed calls, execute and
durably checkpoint
ordered results, then continue generation with cumulative usage and shared cancellation. Successful selected-lineage
`search_memory`, `open_memory`, and `search_attached_files` results now produce deduplicated, path-free conversation or
file citation cards in the Context panel. Removing a card is session-local presentation state and deliberately leaves
the append-only tool audit untouched. Every active or Archived conversation now also has one durable reversible
exclude-from-memory control. Schema version 19 retains the preference without deleting source content; excluded
message chunks and vectors are removed and rebuilt on re-inclusion, while lexical, semantic, hybrid, `search_memory`,
`open_memory`, and `search_attached_files` paths recheck the preference before returning data. Shared documents remain
eligible only through another non-excluded conversation association. Trash now adds one separately confirmed permanent
forget action. The native command accepts only a trashed local-profile conversation without an active response, then
transactionally deletes its conversation-owned source records and message-derived lexical, chunk, and vector data.
Shared content-addressed attachments remain available; newly unreferenced attachment sources and derivatives retain
the existing 24-hour cross-process safety window before startup garbage collection. Existing exports and backup
snapshots are not rewritten, and the application-owned embedding-model cache is retained. Time-based retention is now
also complete. Schema version 20 stores one optional built-in-profile Trash period: 30 days,
90 days, or one year, with manual retention as the default. Expiry is measured from the durable deletion timestamp and
applied only during a healthy app startup; active and Archived conversations are never eligible. Automatic forget uses
the same live-store cascade, attachment safety window, and external-copy/model-cache boundary as explicit forget. The
native tool runtime now applies one fail-closed execution policy before validation or dispatch. The three explicitly
enabled, bounded, read-only memory tools, the zero-argument UTC clock, and the closed `web_search` and `web_fetch`
contracts are classified safe;
any future approval-required tool needs a one-use trusted native grant bound to the exact provider call identity, tool
name, and arguments. Provider calls cannot create
approvals, and unknown or mismatched calls return fixed redacted errors through the existing durable result path. No
approval UI or approval-required tool is registered yet. Schema version 21 now adds native execution classification,
stable terminal outcome, and native-work duration to each append-only tool record. Historical rows remain explicitly
legacy. Reopened responses show a calm call summary and one expandable audit record per call, with raw arguments and
results behind nested disclosures. A provider-neutral native web-search boundary now has fixed Brave Search and Exa
Search adapters. Rust validates bounded queries, confines either API key to a sensitive header, disables redirects,
caps response bytes, and normalizes only inert HTTP(S) result metadata. Settings stores the credentials independently
in the operating-system vault, tests either fixed route with one bounded native probe, and persists one secret-free
active-engine choice. Credential material, probe results, provider bodies, and arbitrary endpoints never cross IPC.
One closed provider-independent `web_search` definition adds optional day, week, month, or year freshness and bounded
include/exclude domain filters. Native validation rejects malformed DNS names, conflicts, and complete queries that
exceed Bottie's fixed limits. Brave maps those filters to its query API; Exa uses native domain arrays and an absolute
publication-date lower bound. Both adapters recheck normalized result hosts before returning them. The safe read-only
native dispatcher executes the typed contract through the selected search provider and maps all failures into the
existing 64 KiB redacted envelope. A session-only Web toggle now
advertises that definition only to explicitly enabled, tool-capable oMLX, Ollama, OpenAI-compatible, or
Anthropic-compatible models. All four mapped providers execute calls through the same bounded native loop, durable
audit, cancellation,
and common result envelope used by memory tools. A separate closed provider-independent `web_fetch` definition now
accepts one absolute public HTTP(S) URL. Its native client strips fragments, rejects embedded credentials, IP literals,
special-use DNS names, non-default ports, and any DNS answer outside a fail-closed public-address policy. Every
redirect is resolved and revalidated explicitly, the client disables ambient proxies and automatic redirects, pins
the accepted DNS answers for each hop, and shares one 15-second deadline across at most three redirects. Successful
responses are limited to 48 KiB of valid UTF-8 HTML, XHTML, or plain page source. HTML/XHTML is parsed into bounded
visible text after executable and embedded elements are removed; plain text passes through the same whitespace,
control-character, and UTF-8 boundary policy. Results contain the final source URL, optional bounded title and
publication metadata, at most 24 KiB of inert text, and an explicit untrusted marker before the existing 64 KiB common
envelope. Explicitly enabled tool-capable oMLX, Ollama, OpenAI-compatible, and
Anthropic-compatible requests now advertise `web_fetch` after `web_search`, execute it through the same safe
dispatcher, checkpoint the exact bounded result, and append that result to the next provider request through the
existing durable Web loop. Successful selected-lineage `web_search` and `web_fetch` results now also produce
deduplicated, removable, path-free Web source cards in the Context panel. Fetched-page metadata supersedes an earlier
search result for the same fragment-free HTTP(S) URL, and dismissal remains session-local without changing the
append-only audit. Fetched-page source cards and their durable audit results now carry calm prompt-injection
presentation derived only from the exact native `untrusted: true` result marker. Claim-level Web citation linking is
also complete. Native capability-gated Web requests now receive fixed citation guidance, while the stored assistant
Markdown and successful response-owned Web results provide the durable link after completion and reopen. The UI marks
only normalized URLs that match the same response's exact retained Web provenance and labels the matching Context card
without changing ordinary safe external links. User-configurable Web destination policy is also complete. A
secret-free HTTPS-only setting defaults on; optional allowed and blocked parent-domain lists are normalized and
bounded in Rust. Each generation snapshots that policy, filters normalized search results before serialization, and
rechecks the initial fetch plus every redirect without weakening the fixed public-address baseline. The next bounded
implementation slice is complete: new installations now pause before the first conversation to identify the discovered
provider/model route and disclose Bottie's provider-delivery, local-storage, Memory, Web, attachment, and credential
boundaries. Completion requires an available remembered provider/model pair and persists through one narrow native
command. Settings written before this flow remain acknowledged, so existing users are not forced through a false first
run. Staged migration and promotion rollback are now complete: supported older stores are preflighted read-only and
migrated only in an isolated same-volume candidate. A verified source-version recovery point and atomic native-only
marker precede live promotion, and restart reconciliation accepts the exact target or restores the verified source
before ordinary corruption classification. Source identities, the exact migration ledger, foreign keys, integrity,
and current semantic metadata are validated; attachment files and the embedding-model cache remain unchanged. Reverse
SQL and automatic binary downgrade are explicitly unsupported. Native clock and primary-provider tool parity are now
complete: Rust owns the closed UTC clock, oMLX joins the durable native tool loop only after fixed endpoint-capability
discovery, and Anthropic model discovery accepts its current structured response. Structured local diagnostics export
is also complete: Settings can snapshot the existing bounded current-session events as versioned JSON through an
explicit native Save dialog, while Rust reapplies credential, path, and content-shaped redaction and returns only a
saved/cancelled outcome plus the selected leaf filename. A first-party read-only Localmail connector now owns the HTTPS
origin, explicit certificate inspection and pinning, bounded server and bearer-authentication tests, vault-only token
storage, and typed native email-search and exact-result-open contracts. Search validates its complete
query/filter/result-limit shape
before config, credential, or network access, calls only the fixed authenticated `POST /v1/search` route, and returns
bounded path-free plain-text summaries marked untrusted. No email body, HTML, attachment content, account/folder
internals, cursor, or score crosses that boundary. Open validates one strict decimal search-result identity before the
same native state, calls only `GET /v1/messages/{id}` with compact headers and external images disabled, and returns
bounded inert subject, address, UTC date, body, and attachment-presence fields. HTML, external-image URLs, Bcc, full
headers, attachment details and bytes, account/folder internals, paths, and credentials remain excluded. The next
provider-independent layer is now complete: `search_email` and `open_email` have closed definitions, strict raw JSON
conversion into the existing connector requests, explicit safe read-only policy entries, and one configured native
executor behind Bottie's existing 64 KiB success/error envelope. Invalid calls fail before Localmail configuration,
vault, or network access, and connector failures map to fixed redacted categories. One off-by-default session Email
control now advertises those definitions only to an explicitly tool-capable oMLX, Ollama, OpenAI-compatible, or
Anthropic-compatible selection after native pinned trust and credential metadata are both present. All four mapped
providers execute through the configured Localmail dispatcher and retain the existing durable safe audit and
cancellation/call/round/output/deadline limits. oMLX and Ollama prompts plus bounded Localmail results stay on
loopback; OpenAI-compatible and Anthropic-compatible prompts plus bounded results use the selected visible cloud route.
In every case, only model-selected queries and exact message identities go to the pinned Localmail server. Anthropic
Messages preserve exact `tool_use` identities, immediate correlated `tool_result` blocks, and ordered
thinking/redacted-thinking state.
oMLX uses its discovered Chat Completions-compatible route without enabling endpoint-owned MCP or arbitrary server
tools. Bottie's CSP and Tauri capability review is also complete. The main WebView now receives only the two core event
permissions it uses to listen for and release native attachment-processing updates; app, path, window, webview, image,
resource, menu, tray, and frontend event-emission commands are no longer granted by `core:default`. The CSP permits
bundled UI, Tauri IPC, inline component styles, and Bottie's opaque attachment-preview protocol while denying frames,
forms, objects, base-URL changes, unused asset schemes, and blob/data images. Secret-vault and filesystem-boundary
contract coverage is now also complete. Saved and absent credential status is collected without reading secret values;
provider settings, diagnostics, attachment ingestion and previews, export, backup, and restore expose exact typed,
path-free IPC shapes under adversarial fixtures. Secret-bearing native command inputs reject unknown fields. The next
bounded slice is now complete: Bottie has a deterministic locked macOS Rust/npm dependency inventory, reviewed
security-sensitive Cargo features and native/runtime assets, authoritative licence sources, explicit classification,
and six release gates. Current resolved package metadata has no unknown licence declaration, but distribution remains
blocked on Bottie's own licence text, a generated notice bundle, packaged ONNX Runtime evidence, pinned and accepted
EmbeddingGemma terms, artwork provenance, and Windows/Linux verification. The keyboard-shortcut slice is now complete:
Command/Ctrl+K opens one accessible local command palette over five safe existing interface actions; exact direct
shortcuts, local filtering, wrapped keyboard selection,
disabled reasons, modal gating, and Escape focus restoration stay entirely in the WebView. Local System, Light, and
Dark themes plus Comfortable and Compact density are now complete. The next bounded product slice is refined
empty/offline/error states. Do not bundle dependency upgrades or remediation, Email provenance cards,
attachment download or opening, Localmail administration, outbound mail, migration-recovery UI, a new feature schema,
automatic retrieval injection, model-cache deletion, packaging, signing, updates, or release work.

A 2026-08-24 reliability correction extends the provider-neutral tool-loop deadline from 30 seconds to five minutes.
Two retained oMLX Email failures showed successful `search_email` results followed by timeout termination while the
27B local model prepared the next round; the shorter whole-loop ceiling contradicted the independently permitted
120-second provider stream-idle window. A focused regression now proves that two fully permitted slow provider
follow-ups can complete while the four-round, eight-call, 64 KiB per-result, 256 KiB aggregate-output, cancellation,
and provider stream-idle limits remain unchanged. The full default Rust suite and the synthetic two-request oMLX Email
fixture pass. This correction adds no schema, IPC, tool definition, provider mapping, credential, or WebView change.

Read these files first:

1. `HANDOVER.md`
2. `ROADMAP.md`
3. `MIGRATION-ROLLBACK.md`
4. `README.md`
5. `CONTRIBUTING.md`
6. `src/routes/+page.svelte`
7. `src/routes/page-state.svelte.ts`
8. `src-tauri/src/lib.rs`
9. `src-tauri/tauri.conf.json`

The repository tracks `origin/main` at `https://github.com/hherb/bottie.git`. The current product slice is on local
branch `codex/keyboard-command-palette`.

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
- a context inspector containing attachments, real tool-sourced memory provenance, privacy routing, and a token meter;
- native attachment selection, application-private ingestion, durable selected-lineage message and branch-independent
  conversation associations, explicit scope labels, narrow removal, and path-redacted outcome feedback;
- durable background plain-text, Markdown, page-aware PDF, DOCX, JPEG, and PNG processing with live path-free labels;
- bounded ready-image thumbnails plus explicit local-only extraction and normalization failure presentation;
- durable extracted-text indexing readiness with honest indexable, unsupported, and blocked presentation;
- capability-aware normalized JPEG/PNG delivery labels and current-draft blocking for text-only models;
- a composer with off-by-default Memory, Web, and configured oMLX/Ollama/OpenAI-compatible/Anthropic-compatible Email
  affordances;
- live normalized inference activity and token streaming;
- an off-by-default reasoning toggle with low effort when enabled;
- collapsed reasoning sections that can be expanded independently of answer text;
- working stop-generation cancellation backed by a Rust abort handle;
- durable conversation creation on first send, recent-conversation navigation, and exact last-open restoration;
- crash-safe partial answer/reasoning checkpoints and visibly interrupted-run recovery;
- real Today, Yesterday, Previous 7 days, Archived, and Trash navigation groups;
- inline conversation rename plus archive, unarchive, recoverable trash, and restore actions;
- durable reversible conversation memory exclusion with a compact `Memory off` navigation label;
- separately confirmed permanent conversation forgetting from Trash with source/derived-data retention disclosure;
- opt-in 30-day, 90-day, or one-year Trash retention in Settings, with manual retention as the default and explicit
  healthy-startup/external-copy disclosure;
- inline user-message editing, assistant-response regeneration, and preserved branch switching;
- native conversation search with snippets, archived-result labels, matching-branch selection, and keyboard focus and
  clear behavior;
- sanitized assistant Markdown with headings, lists, tables, quotes, safe external links, and code presentation;
- assistant-response and reasoning copying as labelled Markdown with visible and screen-reader-readable feedback;
- response retry for interrupted, cancelled, and retryable failed attempts, preserving the original branch;
- durable Good/Poor response ratings with accessible pressed state, replacement, and clearing;
- expandable persisted tool audit summaries with policy, outcome, timing, and nested inert argument/result payloads;
- selected-lineage Markdown/JSON and non-trashed batch JSON export through native Save dialogs with compact feedback;
- complete manual SQLite backup creation through a native Save dialog with compact saved/error feedback;
- confirmed manual restore from validated Bottie backups with a named pre-restore safety copy;
- verified daily automatic SQLite snapshots with a seven-snapshot app-private retention policy;
- corruption-aware startup with guided automatic/manual restore and app-private damaged-data preservation;
- a provider settings dialog with endpoint editing, OS-vault credential management, connection tests, bounded Web
  HTTPS/allowed-domain/blocked-domain policy, timeout policy, and secret/path-redacted session diagnostics including
  automatic-backup outcomes;
- a native-only first-run provider/privacy gate that identifies the available selected route, explains provider versus
  local-data boundaries, keeps Memory and Web off by default, opens full Settings when configuration is needed, and
  persists completion without credentials or paths;
- context-panel open/close behavior;
- reduced-motion and keyboard-focus support.

`src/lib/chat.ts` contains tested pure presentation helpers, `src/lib/presentation.ts` owns typed fixtures and named UI
constants, `src/lib/memory-provenance.ts` strictly derives path-free cards from successful durable native tool
envelopes, and `src/lib/styles/` keeps cohesive stylesheets below the project file-size limit. `src/lib/Icon.svelte`
is the dependency-free local icon set used by the shell.

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
`src-tauri/src/storage/memory_filters.rs` owns the shared query bounds plus source, conversation, and inclusive-date
filter contract, `src-tauri/src/storage/memory_hybrid.rs` owns bounded source-level reciprocal-rank fusion,
`src-tauri/src/storage/memory_exclusion_migration.rs` owns schema-19 durable per-conversation exclusion preferences,
`src-tauri/src/storage/retention.rs` owns the bounded built-in-profile Trash policy and healthy-startup expiry, while
`src-tauri/src/storage/retention_migration.rs` owns its schema-20 optional preference row,
`src-tauri/src/storage/memory_chunks.rs` owns versioned Unicode-safe deterministic chunking plus transactional source
replacement, while `src-tauri/src/storage/memory_chunks_migration.rs` owns catalog metadata and stale-row cleanup,
`src-tauri/src/storage/memory_semantic.rs` owns static sqlite-vec registration, version-contract validation, bounded
embedding batches, atomic chunk/vector mappings, durable progress, and derived-only reset, while
`src-tauri/src/storage/memory_semantic_migration.rs` owns schema-18 model/index metadata and vector cleanup triggers,
`src-tauri/src/storage/memory_semantic_query.rs` owns normalized EmbeddingGemma retrieval queries, exact filtered KNN,
bounded chunk provenance, and current-generation validation,
`src-tauri/src/storage/memory_tool.rs` owns typed bounded `search_memory` arguments and ranked path-free
conversation/message results over hybrid retrieval,
`src-tauri/src/storage/memory_open.rs` owns typed bounded `open_memory` provenance and branch-correct final-turn
reconstruction without changing conversation selection,
`src-tauri/src/storage/memory_file_tool.rs` owns typed bounded `search_attached_files` arguments and ranked path-free
ready-document results over hybrid retrieval,
`src-tauri/src/tool_contract.rs` and `src-tauri/src/tool_contract/` own the provider-independent memory, Web, and
Localmail definitions, closed JSON schemas, raw name/argument validation, and conversion into exact typed native
arguments without executing them,
`src-tauri/src/tool_dispatch.rs` owns provider-neutral execution of typed memory, Web, and Localmail tools plus the
common success/error envelope without model wire policy, while
`src-tauri/src/tool_policy.rs`
owns the explicit safe or approval-required classification and exact-call native approval binding applied before
dispatch,
`src-tauri/src/tool_loop.rs` owns provider-neutral multi-call correlation, recursion/call/output/deadline policy, and
shared cancellation checks used by mapped provider generation,
`src-tauri/src/web_search/` owns typed provider-neutral arguments, freshness and domain policy, the pluggable
query/result/error contract, and fixed-endpoint Brave Search and Exa Search adapters with strict request/response
bounds, safe result-URL policy, and redacted provider failures,
`src-tauri/src/web_fetch/` owns the closed public-URL contract, fail-closed DNS/address policy, pinned per-hop
resolution, explicit redirects, shared timeout, accepted UTF-8 media types, response byte ceiling, HTML/XHTML parsing,
bounded inert text/title/publication extraction, untrusted result, and redacted failures,
`src-tauri/src/web_policy.rs` owns the secret-free HTTPS-only default, bounded normalized allowed/blocked domain lists,
parent-domain matching, and blocked-domain precedence shared by search-result and fetch enforcement,
`src-tauri/src/storage/tool_audit_migration.rs` owns schema-21 audit columns and honest legacy backfill,
`src-tauri/src/generation_tools.rs` owns oMLX/Ollama/OpenAI/Anthropic call correlation, durable call/result checkpoints,
cumulative usage/cost, worker-backed query embedding, and provider-result serialization without leaking paths or
embedding details, while `src-tauri/src/generation_web_tools.rs` selects mapped Web availability, resolves only the
active search engine's native credential, and routes accepted calls into the strict provider-independent dispatcher,
`src-tauri/src/generation_localmail_tools.rs` withholds Email until native pinned trust and credential metadata are
configured, then snapshots the exact path- and credential-owning Localmail executor for mapped oMLX, Ollama,
OpenAI-compatible, or Anthropic-compatible generation,
`src-tauri/src/generation_tool_audit.rs` maps bounded execution envelopes into their durable audit outcomes,
`src-tauri/src/generation_usage.rs` owns provider-neutral usage and cost accumulation across repeated tool rounds,
`src-tauri/src/storage/message_content.rs` owns shared ordered text/reasoning block insertion and reconstruction,
`src-tauri/src/semantic_indexer.rs` owns lazy app-cache FastEmbed acquisition plus the resumable process-lifetime Q4
EmbeddingGemma worker and its synchronous query-embedding proxy,
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
recovery-status/latest-snapshot restore, path-free semantic progress/derived-only reindex, lifecycle commands,
per-conversation memory exclusion, Trash-only permanent forget, and bounded Trash-retention get/set commands. The
database lives in the OS application-data
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
- maps the three closed native memory definitions into Ollama functions only when the user enables Memory and the
  selected model advertises tools, accumulates streamed calls, and appends ordered tool-result messages for each
  follow-up round;
- shares the same Rust abort-handle and typed-channel cancellation path as oMLX.

The remote adapters:

- use separate native OpenAI Chat Completions and Anthropic Messages request and stream shapes;
- validate configurable HTTPS roots, reject embedded credentials/query/fragment values, and disable redirects;
- retrieve API keys just in time from the operating-system credential vault without returning them to Svelte;
- require Touch ID for the first read of each saved credential per macOS app session, then retain the unlocked value only
  in process memory;
- discover remote models and normalize answer, reasoning, usage, cancellation, and provider errors;
- map the three closed native memory definitions into OpenAI Chat Completions only after explicit Memory enablement and
  advertised model capability, reconstruct streamed call fragments, and correlate `tool_call_id` results exactly;
- preserve provider-reported USD cost metadata when compatible endpoints include it.

Requests include a provider ID because model names can collide across providers. Initial discovery tolerates either
local provider being offline and reports a combined retryable error only when neither reports a streaming text model.

Provider configuration now:

- persists normalized endpoint roots and the remembered provider/model pair in the OS application-config directory;
- persists one normalized path-free Web destination policy with HTTPS-only enabled for new and older settings files;
- keeps API keys out of that file and stores only remote credential availability in UI state;
- remembers the last successfully selected provider/model pair in the same Rust-owned settings file;
- accepts HTTP(S) loopback roots for local providers and HTTPS roots for remote profiles, with no embedded credentials,
  queries, or fragments;
- disables redirects for every provider client;
- uses 3-second connect, 5-second discovery, and 120-second stream-idle timeouts;
- keeps the most recent 100 structured diagnostic events in memory and redacts credential-, native-path-, and marked
  content-shaped values before returning them to Svelte or a portable export.

Generation settings now default to reasoning off and a 4,096-token completion ceiling. The toolbar can enable low-effort
reasoning for the next request. Rust maps that provider-neutral setting to oMLX `enable_thinking`/`reasoning_effort` and
Ollama `think`, while normalized reasoning deltas remain separate from assistant answer text throughout IPC and UI
state.

The native application configuration has:

- one explicitly selected main-window capability containing only `core:event:allow-listen` and
  `core:event:allow-unlisten`;
- no app, path, window, webview, image, resource, menu, tray, event-emission, opener, filesystem, shell, clipboard, or
  network plugin permission;
- a restrictive CSP allowing bundled UI, Tauri IPC, necessary inline styles, and only the opaque
  `bottie-attachment` image protocol;
- a 1320 x 820 default window with a 720 x 620 minimum.

## What is still simulated

Do not mistake visual fixtures for implemented backend behavior:

- the browser preview's one memory card and one Web source card are fixtures shaped like successful native results; in
  the native app, Context-panel provenance derives from real selected-lineage durable tool activity and exposes no
  relevance score;
- context-panel token usage remains a fixture; response elapsed time and provider-reported token/cost usage are real
  and survive conversation reopen;
- current next-message attachment selection is session-only until it commits atomically with a submitted user message
  or is explicitly promoted into an existing conversation's durable context; an unassociated selection is eligible
  for native garbage collection after the 24-hour safety window and a later successful startup because the draft itself
  does not survive restart;
- plain-text, Markdown, PDF, and DOCX attachments are extracted into SQLite but remain outside automatic provider
  context; their indexable state feeds native FTS5, deterministic chunk, and semantic-vector indexes. An explicitly
  enabled tool-capable oMLX, Ollama, OpenAI-compatible, or Anthropic-compatible model can request bounded document
  excerpts through `search_attached_files`;
  JPEG/PNG
  derivatives remain application-private and are read only for capability-confirmed vision requests; portable SQLite
  backups embed originals and ready derivatives, while selected/batch exports bundle referenced originals;
- oMLX, Ollama, OpenAI Chat Completions, and Anthropic Messages emit and execute durable native memory-tool records
  when Memory is explicitly enabled; browser-preview tool activity remains a fixture;
- reasoning-toggle state is session-only and resets to off when the app restarts;
- the native lexical, semantic KNN, fused search, and provenance-opening contracts have oMLX, Ollama, OpenAI-compatible,
  and Anthropic-compatible consumers through the bounded dispatcher and loop. Their successful retained results now
  produce real selected-lineage citation cards; dismissals reset with the frontend session and do not delete tool
  records or exclude a source from later retrieval;
- the closed native `web_search` contract is explicitly available only to tool-capable oMLX, Ollama,
  OpenAI-compatible, or Anthropic-compatible models after the user enables Web. Brave calls and exact common results
  enter the durable audit before provider reuse. Successful results now create dedicated Context-panel Web source
  cards;
- the closed native `web_fetch` contract, public-network client, and common dispatcher are mapped to explicitly enabled
  tool-capable oMLX, Ollama, OpenAI-compatible, and Anthropic-compatible requests. Successful fetches return inert text,
  source URL, optional title/publication metadata, and an explicit untrusted marker. Fetched-page source cards and the
  expandable durable result label that content as untrusted and explain that external text may contain misleading
  instructions. Capability-gated native Web requests ask for inline Markdown links to exact result URLs; after
  completion or reopen, only links matched to the same response's successful retained Web results receive claim-level
  citation treatment and a matching Context-card label. Saved HTTPS/domain policy filters search results before their
  common envelope and rejects a fetch before network work or at any policy-mismatched redirect;
- there are no automated end-to-end UI tests yet; the composer and Context panel have focused server-rendered
  component coverage, and pure presentation and Markdown-policy helpers have frontend unit coverage.

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

The cohesively touched product modules remain at or below 500 lines. The crate composition root
`src-tauri/src/lib.rs` remains an existing practical-limit exception; the remaining known indivisible long lines are
SVG path values in `src/lib/Icon.svelte`.

## Prior completed product slice: Staged migration and promotion rollback

### Goal

Prevent a failed forward migration or interrupted promotion from leaving the only live conversation store partly
upgraded, while retaining Bottie's forward-only schema policy and native-only filesystem/database boundary.

### Implemented shape

1. Startup classifies absent, current, supported older, newer, and invalid stores through read-only preflight. Newer
   schemas fail without mutation; ordinary corruption retains the existing restricted recovery path.
2. A supported older store is copied through SQLite's online backup API into one same-volume candidate, so committed
   WAL content is included. Pending SQL and Rust-owned backfills run only against that candidate.
3. Exact candidate validation requires `quick_check`, zero foreign-key failures, the current schema and contiguous
   named ledger, required foundation tables and built-in profile, current semantic metadata, and parity for every
   source-table identity present before migration.
4. A separate verified source-version recovery point is created only after the candidate passes. Recovery points omit
   portable attachment payloads, remain outside automatic-backup rotation, and retain the two newest strict managed
   files without pruning unmanaged lookalikes or the point named by an active marker.
5. One bounded native-only marker records the operation, source/target versions, exact managed leaf names, and prepared
   phase before live promotion. It contains no paths, SQL, content, hashes, credentials, or unrestricted filenames.
6. Promotion reuses SQLite's restore boundary and validates the live target again. A failed promotion restores and
   validates the source point; a damaged destination also has a same-volume replacement fallback. Attachment files and
   the embedding-model cache are never moved or rewritten.
7. Marker reconciliation runs before ordinary corruption classification. It accepts an exact target and cleans only
   the candidate/marker, or restores an exact source and stops that startup with a redacted failure. If neither copy
   validates, all managed evidence is preserved for a compatible build or later guided migration recovery.

### Acceptance criteria

- New, current, newer, corrupt, and supported historical stores retain their explicit fail-safe startup policy.
- Copy, early/late candidate migration, candidate validation, recovery-point validation, live promotion, and
  post-promotion interruption fault points do not silently lose source data or mutate attachment files.
- WAL-only committed rows, exact ledger names, source identities, semantic metadata, foreign keys, strict filenames,
  two-point retention, active-marker protection, and both reconciliation outcomes have path-backed coverage.
- Migration failures expose only stable `migration_failed` or `newer_schema` messages; no Tauri command, capability,
  frontend type, schema version, reverse SQL, provider behavior, or attachment-file operation is added.
- A disposable copy of the real schema-21 store passes integrity, foreign-key, ledger, and row-identity parity checks
  before native launch; the unchanged current-store startup creates no migration artifacts.

### Explicit exclusions

Do not add migration-recovery UI, reverse migrations, automatic binary downgrade, attachment conversion, a new feature
schema, diagnostic upload/export, provider or tool changes, automatic retrieval, model-cache deletion, document
opening, attachment retry controls, a general MCP runtime, packaging, or release work in this slice.

### Verification completed

Twelve focused path-backed tests cover WAL-aware candidate and source copies, source identity parity, injected copy,
migration, validation, safety-copy, restore, and post-restore faults, exact target acceptance, damaged-live source
restoration, newer-schema and malformed-ledger rejection, foreign-key failure, failed marker-write cleanup,
malformed-marker preservation, unchanged attachment bytes, traversal-shaped managed names, strict cleanup, active
recovery-point protection, and two-point retention. Historical version-three
and version-four fixtures now include the coherent ledgers required of supported real stores. The full Rust suite has
348 tests: 326 pass by default and 22 loopback, public-network, credential, or live-provider checks remain opt-in.

An immutable read-only inspection of the live store returned schema 21, `quick_check=ok`, zero foreign-key failures,
and an exact 21-row ledger. A disposable SQLite copy matched the source schema, ledger, and identities/counts for 13
conversations, 17 branches, 72 messages, 2,740 blocks, 30 provider runs, 35 usage rows, 12 tool calls/results, four
attachments, six message associations, and 97 chunk/vector mappings. The native app then compiled, launched, and
rendered the existing selected conversation normally. A post-launch immutable check remained schema 21 with the same
integrity and ledger state, and no migration marker, candidate, or recovery directory was created for the current
store. Browser and provider checks are not applicable because this slice changes no presentation or networking.

## Prior completed product slice: Native clock and primary-provider tool parity

### Goal

Ground time-sensitive answers in Bottie's real native clock, make the primary oMLX route capable of the same Bottie-owned
Memory and Web loops as the other mapped providers, and restore current Anthropic model discovery without delegating
tool execution to a provider or exposing host location.

### Implemented shape

1. One closed provider-independent `current_time` definition accepts an exact empty object and returns only the
   Rust-owned system clock as a bounded RFC 3339 UTC value. It returns no host timezone, locale, hostname, path, or
   platform detail and performs no network access.
2. The clock is classified as a safe read-only native tool, uses the existing validation, cancellation, deadline,
   output, durable audit, and provider-call correlation policy, and is advertised to every mapped tool-capable route
   independently of the Memory and Web toggles. The existing toggles continue to control only their own definitions.
3. Fixed Web guidance tells a model to call `current_time` before interpreting `now`, `today`, `current`, `latest`, or
   relative publication dates when the answer depends on them. Search-result dates are evidence, not a substitute for
   the clock.
4. oMLX discovery performs one bounded check of its fixed loopback OpenAPI document. Only an explicit
   `/v1/chat/completions` request schema containing both `tools` and `tool_choice` enables tools for discovered oMLX
   text models; absence, over-limit data, or malformed metadata remains fail-closed without model-name heuristics.
5. oMLX receives Bottie's closed OpenAI-shaped function definitions, accumulates fragmented streamed call identities
   and object arguments, checkpoints each call/result, and continues through correlated assistant `tool_calls` plus
   `role: tool` results under the existing four-round/eight-call/five-minute loop.
6. Memory, Web search, and Web fetch remain explicitly user-enabled. Bottie resolves credentials, validates arguments,
   performs retrieval/network work, and stores the audit; oMLX receives only the same bounded inert result envelopes
   used by the other providers. oMLX's own MCP and server-side tool routes are not invoked or forwarded.
7. Anthropic model discovery accepts its current nullable structured `capabilities` object as well as an omitted value
   and the legacy string-array extension used by compatible fixtures. Unknown additive fields are ignored, the current
   `max_input_tokens` value is normalized when present, and malformed top-level/model identity data still returns the
   existing redacted error. Structured fields such as Anthropic server-side `code_execution` are not reinterpreted as
   permission for Bottie's client-executed tools; compatible endpoints retain explicit capability gating.

### Acceptance criteria

- An injected-clock unit test proves exact UTC serialization and rejects null, unknown fields, and non-object
  arguments without reflecting them in an error.
- Ollama, OpenAI-compatible, Anthropic-compatible, and explicitly capable oMLX requests receive `current_time`; models
  and oMLX endpoints without explicit tool support receive no tool definition.
- oMLX Memory-only, Web-only, combined, and clock-only requests preserve exact definition gating, streamed call
  identity, durable audit order, result correlation, cumulative usage, cancellation, and terminal provider-run state.
- A current oMLX live check demonstrates a clock call reporting the real UTC year and an explicitly enabled Memory or
  Web call completing through Bottie's dispatcher. Reopened activity shows the same bounded audit.
- Missing Brave/Exa credentials fail before Web definitions are sent, while the credential-free clock and Memory remain
  usable. No API key, query, source text, clock host detail, path, or provider body crosses WebView IPC.
- The user can select the primary oMLX route and enable Memory and Web when the fixed endpoint contract explicitly
  supports tools; capability loss clears the toggles as it does for other providers.
- A fixture matching Anthropic's current Models API response, including object and null `capabilities`, discovers the
  models and context limits; omitted and legacy array forms remain compatible, and wrong top-level/model identity
  shapes still fail with no provider body or credential in the user-facing error.
- With a valid credential, Settings can test the canonical Anthropic endpoint successfully. Its existing Messages tool
  loop and compatible-endpoint capability boundary remain unchanged by the decoder repair.

### Explicit exclusions

Do not add local-time or timezone settings, location inference, oMLX-owned MCP execution, arbitrary oMLX server tools,
approval UI, automatic memory retrieval, a storage migration, migration rollback implementation, model-cache deletion,
document opening, attachment retry controls, packaging, or release work in this slice.

### Verification completed

Focused contract tests inject an exact clock and prove millisecond RFC 3339 UTC output, empty-object-only arguments,
safe policy, bounded results, durable audit, and redacted failures. Provider request fixtures prove clock-only, Memory,
Web, and combined definition gating across all four mapped routes. oMLX fixtures cover bounded OpenAPI discovery,
nullable catalogue capabilities, explicit text/vision/embedding status, fragmented call reconstruction, exact result
correlation, cumulative usage, reopen, and terminal audit state. A host-local two-round integration check completed
through an exact `role: tool` clock result. A live oMLX 0.6.3rc2 check with the loaded Qwen3.8 model called both
`current_time` and `open_memory`; the reopened clock result reported the current UTC year and the reopened memory result
contained the exact durable message. Anthropic fixtures match the current nullable structured `capabilities` object,
`max_input_tokens`, omitted and legacy array forms, ignored additive fields, and fail-closed identity errors. No schema,
credential, WebView IPC, provider-owned MCP, arbitrary server tool, approval UI, or host location surface was added.

Follow-up native validation found two regressions before merge. The Svelte presentation gate still omitted `omlx` from
its mapped-provider allowlist even after Rust discovered explicit tool support, leaving Memory and Web disabled for the
live Qwen3.8 selection. One shared pure allowlist now includes all four mapped providers, and regression tests require
tool-capable oMLX selections to enable both controls while preserving the false-capability denial. Anthropic model
discovery worked, but the two live Claude Sonnet 5 runs in the immutable store ended as `invalid_request`: Bottie's
provider-neutral default added `temperature: 0.7`, while current Sonnet 5 rejects non-default sampling parameters.
Anthropic requests now remove that implicit setting before durable provider-run provenance and wire serialization;
focused tests require its absence while retaining explicit disabled/adaptive thinking behavior. The user subsequently
confirmed authenticated live Web search completes successfully with both Claude Sonnet 5 through Anthropic and Qwen3.8
through oMLX.

## Prior completed product slice: Redacted local diagnostics export

### Goal

Let a user explicitly save Bottie's bounded current-session diagnostics for local support without broadening the
WebView, filesystem, provider-content, or automatic-upload boundaries.

### Implemented shape

1. One narrow `export_diagnostics` command snapshots the existing 100-event in-memory history and opens a native JSON
   Save dialog only when at least one event exists. The destination remains native-only; IPC returns saved/cancelled
   state and the selected leaf filename.
2. Version 1 of `bottie-local-diagnostics` records the generation timestamp, current-session scope, and each existing
   stable timestamp, level, event, optional provider identity, and optional detail in recording order. It uses a
   date-normalized default filename and a trailing newline.
3. Rust reapplies the native credential, path, and content-shaped redaction boundary before serialization. The document
   explicitly declares that credentials/authentication material, provider request and response bodies, raw tool
   arguments/results, database and attachment content, and native filesystem paths are omitted.
4. Settings exposes one labelled Export JSON action beside Refresh. It is unavailable in browser preview and for an
   empty history, reports cancellation calmly, and presents path-redacted failures without closing or saving Settings.
5. The change adds no schema, durable diagnostic history, background task, upload path, generic file command, provider
   traffic, credential access, tool payload access, or database/attachment read.

### Acceptance criteria

- Empty history fails before any Save dialog, native cancellation writes nothing, a selected destination receives exact
  UTF-8 JSON, and the WebView receives no directory.
- Export rendering is deterministic for an injected generation time and retains event insertion order.
- Secret-, path-, request-body-, and user-content-shaped fixtures do not survive the redaction boundary.
- The portable omission policy is explicit and independent from Bottie's SQLite schema version.
- The Settings action remains accessible and contained at the desktop default and native minimum viewport.

### Explicit exclusions

Do not add automatic upload, provider request or response bodies, raw tool arguments/results, database or attachment
content, durable diagnostic storage, a schema migration, migration-recovery UI, packaging, or release work.

### Verification completed

Focused Rust coverage exercises the versioned portable contract, deterministic recording order and filename,
secret/path/content-shaped redaction, empty-history rejection, clean cancellation, exact UTF-8 file output, and
leaf-filename-only outcome. The frontend contract renders the labelled action and disclosure. The standard frontend
suite has 22 passing files and 87 tests; `svelte-check` reports zero errors or warnings, production build succeeds, and
Prettier reports every tracked frontend source formatted. The full Rust suite has 353 tests: 331 pass by default and 22
loopback, public-network, credential, or live-provider checks remain opt-in. Cargo formatting and compile checks pass.

The browser preview was reviewed at 1320 × 820 and the 720 × 620 native minimum. The diagnostics heading, disclosure,
Export JSON, Refresh, empty state, and Settings footer remain contained with no horizontal document overflow; the
browser console reported no warnings or errors. Browser preview correctly disables export because it has no native
session history. The native development app compiled, development-signed, and launched against the existing store.
Immutable read-only inspection returned schema version 21, `quick_check=ok`, zero foreign-key failures, an exact
21-row migration ledger, 13 conversations, 72 messages, 30 provider runs, and 12 tool invocation/result pairs. The
slice adds no schema or live-store mutation. The real macOS Save-panel cancellation/write interaction was not
synthetically clicked; native path-backed coverage proves the exact write/cancellation and path-redacted outcome
contract. Live-provider tests were not applicable because this slice changes no provider networking or wire mapping.

## Next bounded product slice: focused accessibility audit and reduced-motion verification

Audit the existing conversation shell, composer, navigation, Context, Settings, first-run, recovery, and command-palette
surfaces as one WebView-only accessibility slice. Verify keyboard order and focus return, accessible names and state,
status announcements, dark/light contrast, zoom containment, and reduced-motion behavior at the desktop and native
minimum viewports. Fix only presentation semantics and styling found by that audit. Keep native/provider contracts,
retry and recovery policy, persistence, IPC/schema, dependencies, network behavior, performance testing, native menu
or tray work, packaging, signing, updates, and release work unchanged.

## Most recently completed product slice: refined empty, offline, and error states

### Goal

Make the blank-conversation, provider-disconnected, and durable failed-response states calm, distinct, actionable, and
contained without changing any native error, retry, persistence, or trust-boundary contract.

### Implemented shape

1. `ConversationStatus.svelte` now owns one typed presentation boundary for connected, checking, browser-preview, and
   offline provider states. Blank conversations receive state-specific headings and guidance, while existing
   conversations retain a compact status banner.
2. Offline banners keep the stable native message and optional diagnostic, and expose the existing retry callback as a
   labelled `Retry connection` button with visible keyboard focus. Checking remains a polite live status and browser
   preview remains explicitly native-only without offering a false retry action.
3. Empty connected conversations identify the selected provider and explain that the next message starts a private
   local conversation. The prior empty `Current conversation` divider is absent until a message exists.
4. Durable failed assistant responses retain their exact stored text, metadata, and retry eligibility while adding a
   compact `Response needs attention` label. No error text or retryability is reclassified in the WebView.
5. Cohesive dark/light presentation lives in `conversation-status.css`. The global reduced-motion policy now also
   removes ambient decoration, while retaining the existing near-zero animation and transition durations.

### Acceptance and explicit exclusions

- Rendered contracts distinguish all four provider states, keep diagnostics visible only when supplied, expose retry
  only for offline state, and preserve failed-response content plus its existing retry action.
- The browser-preview empty and existing-conversation states remain contained at 1320 x 820 and 720 x 620 in dark and
  light themes with visible focus, zero horizontal document overflow, and no console warning or error.
- Reduced-motion styles disable repeated animation and transitions and remove ambient motion. No new animation is
  introduced by the status surfaces.
- No native error category, provider recovery/retry behavior, native command, capability, credential, storage, schema,
  IPC, dependency, network, menu, tray, performance, packaging, signing, update, or release behavior changed.

### Verification completed

Focused TDD first failed because `ConversationStatus.svelte` did not exist and because durable failed responses lacked
the new visible state label. Four rendered tests now cover connected, checking, browser-preview, and offline empty
states, compact offline presentation above existing messages, diagnostic and retry visibility, and preservation of a
durable failed response's stable content and retry action.

Prettier accepts all tracked frontend and script sources; `svelte-check` reports zero errors or warnings; all 31 root
Vitest files and 108 tests pass; and the production build succeeds. Cargo formatting and compilation pass. The full
Rust suite remains at 416 tests: 387 pass by default and 29 loopback, public-network, credential, or live-provider tests
remain explicitly ignored. `git diff --check` is clean, and touched implementation and style files remain below the
practical 500-line limit after extracting the cohesive status stylesheet.

The browser preview was reviewed at 1320 x 820 and the 720 x 620 native minimum in dark and light themes. The refined
browser-disconnected banner, blank-conversation guidance, composer, and responsive overlays remain contained; exact
DOM measurements report no horizontal document overflow. Focus styling is visible, parsed reduced-motion rules cover
all animations and transitions plus ambient decoration, and the browser console reports no warnings or errors.

The native development app compiled, development-signed, and launched against the existing store. Immutable read-only
inspection returned schema version 21, `quick_check=ok`, and zero foreign-key failures. Native connected/offline UI
interactions were not synthetically clicked; focused rendered coverage exercises those exact path-free presentation
states. Live-provider tests are not applicable because this slice changes no provider networking or wire mapping.

## Prior completed product slice: themes and density options

### Goal

Add one local presentation preference surface for System, Light, and Dark themes plus Comfortable and Compact density
without extending native authority or mixing display state into provider settings.

### Implemented shape

1. `appearance.ts` owns the closed preference types, dark/comfortable fallback, unknown-value normalization, safe
   local persistence, explicit theme resolution, document attributes, and a controller whose OS color listener exists
   only while System is selected.
2. A same-origin external preload applies valid theme, theme-preference, density, color-scheme, and theme-color state
   before Svelte and the generated stylesheet can paint. The existing CSP remains unchanged and permits that bundled
   script through `script-src 'self'`.
3. Settings exposes labelled radio groups with visible focus for all five choices. Changes apply immediately and remain
   independent of native provider Save/reconnect, credentials, diagnostics, tools, and storage.
4. `appearance.css` retains the current dark/comfortable visuals as the default, adds a complete light semantic palette,
   and applies compact spacing across navigation, messages, composer, Context, and Settings without changing content.
5. The root shell uses non-scrollable clipping so focus within the fixed Settings dialog cannot leave the application
   content programmatically scrolled after the modal closes.

### Acceptance and explicit exclusions

- Missing, malformed, and out-of-contract saved state resolves to dark/comfortable. Valid state survives reload and is
  applied before ordinary app initialization.
- System follows current OS light/dark state and listens for changes only while selected; explicit Light or Dark removes
  the listener. Every choice remains keyboard-focusable and labelled.
- Comfortable and Compact remain contained at the 720 × 620 native minimum and the 1320 × 820 desktop viewport without
  horizontal document overflow or a retained shell scroll offset after closing Settings.
- No Tauri command, capability, provider/tool mapping, credential, native storage, schema, dependency, network, menu,
  tray, packaging, signing, update, or release behavior changed.

### Verification completed

Focused TDD first failed because the appearance module and component did not exist. Three pure/controller tests cover
the migration-safe fallback, closed persisted shape, resolved themes, safe parsing, immediate writes, conditional OS
listener lifecycle, and document state. Two rendered component tests cover the labelled radio groups and Settings
integration.

Prettier accepts all tracked frontend and script sources; the locked dependency inventory is unchanged; `svelte-check`
reports zero errors or warnings; all 29 root Vitest files and 104 tests pass; and the production build succeeds. Cargo
formatting and compilation pass. The full Rust suite remains at 416 tests: 387 pass by default and 29 loopback,
public-network, credential, or live-provider tests remain explicitly ignored. `git diff --check` is clean.

The browser preview was reviewed at 1320 × 820 and 720 × 620 across dark/comfortable, light/compact,
light/comfortable, dark/compact, and System selection. Preference reload applied the saved explicit document state,
both densities remained contained, focus rings were visible, modal close retained a zero shell scroll offset, and there
was no horizontal document overflow or console warning/error. System resolved to the current OS scheme; focused unit
coverage proves subsequent OS changes are ignored after an explicit theme is selected. The native app compiled,
development-signed, and launched against the existing store. Native UI interaction was not synthetically clicked;
provider, schema, credential, filesystem, and live-network checks are not applicable to this local WebView slice.

## Prior completed product slice: keyboard shortcuts and command palette

### Goal

Add one accessible, locally filtered command palette over a bounded registry of existing safe interface actions without
creating new native authority or letting shortcuts cross existing modal and generation gates.

### Implemented shape

1. `command-palette.ts` owns a pure five-command registry for New conversation, conversation search, conversation
   navigation, Context-panel toggling, and Settings. It derives Command or Ctrl labels from the desktop platform,
   provides exact direct-shortcut matching, token filtering, and wrapped enabled-item navigation.
2. Command/Ctrl+K and a labelled toolbar button open the modal palette. Command/Ctrl+N, Command/Ctrl+Shift+F,
   Command/Ctrl+Shift+B, Command/Ctrl+Shift+C, and Command/Ctrl+, run the same registered actions directly.
3. The palette uses a labelled modal dialog, search combobox, related listbox, active-descendant state, visible
   shortcuts, local descriptions, disabled reasons, Arrow-key navigation, Enter execution, a Tab focus trap, Escape
   dismissal, and exact invoking-control focus restoration.
4. New conversation stays disabled while generation, message persistence, or conversation management is active.
   Browser-preview search stays visible with its native-storage-unavailable reason. Disabled direct shortcuts open the
   palette to expose that reason instead of silently running or cancelling work.
5. Existing first-run and Settings modals retain priority. Actions reuse only existing UI functions and focus targets;
   there is no global OS shortcut, Tauri plugin/capability, IPC command, provider/tool execution, lifecycle command,
   schema, credential, filesystem, or network change.

### Acceptance and explicit exclusions

- The complete command registry is small, deterministic, locally filtered, platform-labelled, and accessible from both
  pointer and keyboard input. Escape restores focus, and direct shortcuts share the exact registry and availability.
- Context labels reflect current visibility; responsive navigation commands reveal the sidebar before focusing its
  existing New conversation or search control.
- Do not add global shortcuts, native menu/tray work, destructive lifecycle actions, provider/tool execution,
  dependency upgrades or licence remediation, themes, broad accessibility remediation, performance work, packaging,
  signing, updates, or release work.

### Verification completed

Focused TDD first failed because the registry and palette component did not exist. Five focused tests now cover the
complete platform-labelled registry, busy and native-storage disabled reasons, local multi-field filtering, exact
shortcut matching, wrapped disabled-item skipping, and the modal combobox/listbox structure.

Prettier accepts all tracked frontend and script sources; `svelte-check` reports zero errors or warnings; all 27 root
Vitest files and 100 tests pass; and the production build succeeds. Cargo formatting and compilation pass. The full
Rust suite contains 416 tests: 387 pass by default and 29 loopback, public-network, credential, or live-provider tests
remain explicitly ignored. `git diff --check` is clean.

The browser preview was reviewed at 1320 × 820 and the 720 × 620 native minimum. The palette remained contained with
no horizontal document overflow. Pointer opening, local filtering, wrapped Arrow/Enter execution, disabled-search
feedback, direct Context toggling, Escape dismissal, invoking-control focus restoration, and responsive navigation
focus all worked; the console reported no warnings or errors. The browser preview deliberately exposes conversation
search as unavailable because it has no native store. Native launch is recorded separately below. Live-provider,
schema, credential, filesystem, and immutable-store checks are not applicable because this slice changes only local
WebView presentation and existing UI action routing.

The native development app compiled, development-signed, and launched against the existing store with the updated
shell composition. The macOS window was not synthetically clicked, so native launch is not claimed as a second
keyboard-interaction check; the rendered browser interaction evidence covers the exact shortcut and focus behavior.

## Prior completed product slice: dependency and licence review

### Goal

Produce a reproducible inventory of the Rust, npm, and Tauri dependencies shipped by Bottie, including exact direct
and transitive versions, declared licences, native/runtime assets, and the security-relevant features that select
them, without changing dependency resolution or executing untrusted package work.

### Implemented shape

1. `scripts/dependency-inventory.mjs` runs locked, offline Cargo tree resolution for macOS arm64 and x64 normal/build
   graphs and reads npm's version-three lockfile directly. It invokes no Cargo build, npm installation, lifecycle
   script, network service, telemetry, or upload.
2. `dependency-inventory.json` records exact package versions, direct/transitive and graph/install scope,
   target membership, declared licence, review class, resolved Rust features or npm optional/peer state, authoritative
   registry source, npm integrity, security-sensitive direct choices, asset metadata, and hashes for every reviewed
   input. `npm run dependencies:check` reproduces and compares the snapshot byte-for-byte.
3. The reviewed macOS union contains 399 unique Rust crates: 29 direct, 376 in a normal runtime graph, and 23
   build-only. `Cargo.lock` retains a 649-package all-platform superset. The root npm lock contains 157 exact package
   paths: 14 direct, eight production-install, and 149 development-install. npm's development marker does not prove
   code is absent from the compiled frontend, so every locked npm entry remains in the notice review. The separately
   deployed `website/` project is not bundled into the desktop app and retains its own lockfile and Node test command.
4. Across those packages and six non-package asset groups, 10 entries are compatible, 543 require release notices,
   nine require human review, and none has an unknown declaration. Five MPL-2.0 Rust crates and npm's Python-2.0
   `argparse` remain review-required rather than being declared incompatible without legal review.
5. `DEPENDENCY-LICENCES.md` records the classification policy, authoritative sources, selected network/TLS/storage/
   decoder/native features, SQLite/sqlite-vec/ONNX Runtime/EmbeddingGemma delivery, and six release gates: missing
   Bottie licence text, no notice bundle, uncaptured ORT artefact notices, unpinned/unaccepted Gemma terms, unknown
   artwork provenance, and unverified Windows/Linux bundles.
6. Root Vitest now excludes the independent `website/` project and unrelated repository-root `assets/` tree so
   standard desktop validation tests only its `src/` and `scripts/` surfaces. This slice adds no dependency version,
   compiled feature, app code, schema, IPC, capability, UI, provider/tool behavior, credential, or live-store change.

### Acceptance and explicit exclusions

- Every resolved macOS Rust package and every root npm lock path has an exact version, declared licence, scope, and
  authoritative package source in the generated snapshot. Input or graph drift fails the deterministic check.
- Classification is a conservative technical release policy, not legal advice. Unknown and review-required entries
  are never silently accepted; declared licences do not substitute for the exact distributable texts and notices.
- Native/runtime assets distinguish repository-bundled bytes, build-time inputs, runtime downloads, and OS-supplied
  frameworks. The report does not claim Windows/Linux or final package contents were verified.
- Do not upgrade, replace, or vendor dependencies; fetch or run package scripts; change model-cache behavior or add a
  licence-acceptance UI; change providers, tools, IPC, schema, capabilities, CSP, or rendered UI; or bundle shortcuts,
  themes, broad accessibility work, performance work, packaging, signing, updates, or release work.

### Verification completed

Focused TDD first failed because the dependency inventory module did not exist. Four tests now cover Cargo
deduplication/feature merging, multi-target runtime-versus-build scope, conservative licence classification, and exact
npm path/scope/integrity/directness. The deterministic locked offline check passes against the committed snapshot.

Prettier accepts all tracked frontend and script sources; `svelte-check` reports zero errors or warnings; all 25 root
Vitest files and 95 tests pass; and the production build succeeds. Cargo formatting and compile checks pass. The full
Rust suite contains 416 tests: 387 pass by default and 29 loopback, public-network, credential, or live-provider checks
remain explicitly ignored. `git diff --check` is clean.

Browser and native-app presentation review, live-provider checks, and immutable live-store inspection are not
applicable because the slice changes no application code, WebView/native boundary, provider protocol, schema, or
rendered behavior. The inventory command used only locked offline metadata and did not fetch dependencies, execute
package build/lifecycle scripts, inspect credentials, or mutate the live store.

## Prior completed product slice: Secret-vault and filesystem-boundary tests

### Goal

Prove adversarially that Bottie's existing credential, settings, diagnostics, attachment, preview, export, backup, and
restore commands expose only their narrow typed metadata across Tauri IPC without reading real credentials, mutating
the live store, or granting the WebView new native authority.

### Implemented shape

1. Credential status collection now uses one independently tested helper that calls only secret-free
   `configured`/`unlocked` metadata methods. Saved, absent, locked, and unlocked fixtures serialize as the exact four
   provider-status fields; a panic-on-read fake proves status collection never requests a secret value.
2. Credential-status failures discard backend detail and return one stable error without a diagnostic. Provider
   settings, credential updates, provider/model selection, and provider/search connection drafts now deny unknown
   fields, including filesystem-, shell-, database-, and credential-shaped additions.
3. Exact provider-settings and current-session diagnostic serialization tests assert the complete field allowlists.
   Secret-, Unix-path-, and Windows-path-shaped diagnostic fixtures remain redacted both in current IPC entries and
   portable diagnostic exports; selected-path write failures return a fixed error.
4. Isolated attachment ingestion covers one accepted and one rejected path while proving source paths, hashes, bytes,
   extracted text, and derivative identities are absent from the returned metadata. Cancellation returns one exact
   empty result. The opaque preview protocol rejects non-GET, nested, query-bearing, non-UUID, and encoded traversal
   requests with bodyless `no-store`/`nosniff` responses.
5. Backup and restore outcomes moved into a focused pure module; conversation export and attachment cancellation use
   matching pure outcome helpers. Tests require saved/restored results to contain only status and leaf filenames and
   cancellation to contain no filename. Storage errors drop injected native I/O detail.
6. The slice adds no credential identity, biometric change, keyring access in tests, capability, CSP destination,
   generic filesystem command, schema, provider or Email behavior, UI surface, dependency, or live-store mutation.

### Acceptance and explicit exclusions

- Tests use only fake credentials, temporary files, and temporary SQLite stores; no real user secret or live database
  is read or changed.
- Malformed structured inputs fail closed before command behavior, while valid existing frontend payloads retain their
  camel-case contracts.
- All file interactions remain Rust-owned; the WebView receives safe leaf labels or opaque attachment IDs, never a
  selected directory, source path, database path, content hash, byte payload, or extracted content.
- Do not change keyring identities or biometric policy; add a generic filesystem, shell, SQL, network, clipboard,
  opener, MCP, or plugin surface; change Email behavior; add provenance cards; perform a schema migration; or bundle
  dependency review, shortcuts, packaging, signing, updates, or release work.

### Verification completed

Focused TDD first failed because secret-bearing structured command inputs accepted unknown fields. The exact
credential/status/settings/diagnostic and native-file outcome tests pass after applying closed deserialization and
extracting pure path-free outcome helpers. The full Rust suite has 415 tests: 386 pass by default and 29 loopback,
public-network, credential, or live-provider checks remain explicitly opt-in. Cargo formatting and compile checks pass.
The frontend production build succeeds, `svelte-check` reports zero errors or warnings, and Prettier reports every
tracked frontend source formatted. The literal local `npm test` also discovers the unrelated untracked
`website/tests/rendered-html.test.mjs` file as an empty suite while all 91 tracked tests pass; the tracked suite passes
cleanly when only that untouched untracked tree is excluded.

Browser presentation review is not applicable because this slice changes no Svelte, CSS, or rendered behavior. The
sandboxed `npm run tauri dev` could not bind the local Vite port, and a host retry found an existing listener already
using port 1420. The freshly built Bottie binary was instead development-signed through the repository wrapper,
launched against that existing local server, remained running without a terminal error, and stopped cleanly. Immutable
read-only checks before and after launch were identical: schema version 21, `quick_check=ok`, zero foreign-key failures,
an exact 21-row migration ledger, 18 conversations, 96 messages, 42 provider runs, and 22 tool invocation/result pairs.
No real credential was read, no native dialog was exercised, and no live-store row changed. The temporary path-backed
tests cover accepted/rejected attachment ingestion, diagnostic writing, and saved/cancelled export, backup, and restore
outcome construction. Live-provider tests are not applicable because provider networking, discovery, streaming,
cancellation, credentials, and tool mappings are unchanged.

## Prior completed product slice: CSP and Tauri capability review

### Goal

Reduce Bottie's WebView authority to the exact Tauri core commands and CSP destinations required by the implemented UI,
opaque attachment previews, native events, and Rust-owned networking without changing product behavior.

### Implemented shape

1. `tauri.conf.json` explicitly selects only the checked-in `default` capability, so adding another capability file
   cannot silently merge authority into the main window.
2. The main-window allowlist contains only `core:event:allow-listen` and `core:event:allow-unlisten`, which are required
   for path-free attachment-processing updates. The frontend cannot emit native events or invoke Tauri's default app,
   path, window, webview, image, resource, menu, or tray commands.
3. Registered Bottie commands remain narrow Rust-owned `invoke_handler` entries. The dialog plugin is called only by
   Rust, and no frontend dialog, filesystem, shell, opener, clipboard, SQL, HTTP, or arbitrary plugin permission exists.
4. CSP now permits only bundled UI and fonts, Tauri's two IPC transports, necessary inline component styles, and the
   opaque `bottie-attachment` image scheme plus its Windows/Android localhost form. Unused `customprotocol:`, `asset:`,
   `blob:`, and `data:` sources were removed.
5. `base-uri`, `form-action`, `frame-src`, and `object-src` are fixed to `none`; scripts remain self-only. Existing safe
   external links retain their tested `_blank` plus `noopener noreferrer` behavior without adding an opener surface.
6. Rust contract tests parse the checked-in Tauri configuration and capability file and assert the complete exact
   boundary, including the absence of remote-domain IPC and asset-protocol configuration.

### Acceptance and explicit exclusions

- Provider, Web, and Localmail network access remains Rust-owned and is not represented in `connect-src` or a frontend
  network permission.
- Attachment previews still accept only opaque IDs through the existing main-WebView-only protocol; filesystem paths,
  bytes, hashes, extracted text, and derivative identities remain absent from IPC.
- Native Save/Open dialogs remain Rust-owned. Clipboard writes continue through the standard browser API and gain no
  native clipboard permission.
- This slice adds no generic filesystem, shell, SQL, network, clipboard, opener, MCP, or plugin surface; Email change;
  provenance card; schema migration; shortcut; packaging; signing; update; or release work.

### Verification completed

Focused TDD first failed because no explicit capability selection existed and `core:default` remained broader than the
two event commands Bottie uses. The two configuration-contract tests pass after tightening the policy. Prettier,
Svelte diagnostics, production build, Cargo formatting, and Cargo check pass. The tracked frontend suite has 24 files
and 91 passing tests. The literal root `npm test` also discovers the unrelated untracked
`website/tests/rendered-html.test.mjs` file and reports it as an empty suite while all 91 tracked tests pass; rerunning
with only that untouched untracked tree excluded passes. The full Rust suite has 399 tests: 370 pass by default and 29
loopback, public-network, credential, or live-provider checks remain explicitly opt-in.

The browser preview was reviewed at 1320 x 820 and the 720 x 620 native minimum. Both fixture images loaded, the shell
and compact Context overlay stayed within the document width, and the browser console reported no warnings or errors.
The preview does not receive Tauri's production CSP or capabilities, so it verifies presentation only. The native
development app compiled, development-signed, launched with the tightened configuration, and remained running without
a terminal error. Immutable read-only inspection returned schema version 21, `quick_check=ok`, zero foreign-key
failures, an exact 21-row migration ledger, 17 conversations, 92 messages, 40 provider runs, and 20 tool
invocation/result pairs. This slice adds no schema or intended live-store mutation. No native attachment update was
available to trigger during the launch, so event delivery is covered by the exact capability contract and existing
frontend listener behavior rather than a newly completed background job. Live-provider tests were not applicable
because provider networking, discovery, streaming, cancellation, credentials, and tool mappings did not change.

## Prior completed product slice: oMLX Email mapping

### Goal

Let one explicitly tool-capable oMLX model use Bottie's two configured read-only Localmail tools after the existing
off-by-default session choice, while preserving native endpoint-capability, trust, credential, provider-call, WebView,
and shared tool-loop boundaries and making both loopback and pinned-Localmail destinations explicit.

### Implemented shape

1. The Email control is now available for tool-capable oMLX models as well as Ollama, OpenAI-compatible, and
   Anthropic-compatible models after the same secret-free Localmail status confirms saved certificate trust and a
   vault credential. Provider/model changes, missing readiness, and absent tool capability continue to disable it with
   an actionable path-free reason.
2. Enabled oMLX Email states that the prompt stays with the selected oMLX loopback endpoint, while only model-selected
   email queries and exact message identities go to the pinned Localmail server. Existing Ollama loopback and cloud
   provider disclosures remain unchanged.
3. Native generation rechecks explicit Email intent, exact oMLX routing, discovered per-model tool capability, and
   configured Localmail trust/credential metadata before retaining the request flag or constructing one immutable
   generation-scoped executor. Eligibility still retrieves no credential material.
4. An Email-only oMLX Chat Completions request advertises `search_email`, then `open_email`, then `current_time` through
   the fixed endpoint's explicitly discovered tool contract. It does not invoke or forward oMLX MCP or arbitrary
   server-owned tools.
5. oMLX-emitted calls preserve the exact provider call identity in durable invocation and immediate correlated
   `role: tool` results. Execution reuses the safe policy, strict typed Localmail conversion, pinned fixed-route client,
   64 KiB redacted envelope, four-round/eight-call/five-minute loop, 256 KiB aggregate ceiling, and shared cancellation.
6. Focused oMLX Email orchestration tests live in their own cohesive module. No connector route, Tauri IPC payload or
   capability, provider credential behavior, storage schema, or existing mapped-provider behavior changed.

### Acceptance and explicit exclusions

- Email remains off by default and session-only. oMLX availability requires configured pinned trust, a saved vault
  credential, fixed-endpoint capability discovery, and explicit model tool capability.
- Accepted calls cannot select another Localmail route or weaken certificate pinning, bearer authentication,
  redirect/proxy denial, response/result bounds, identity correlation, inert text, `untrusted: true`, durable audit,
  cancellation, or shared loop policy.
- Prompts and bounded Localmail results stay on the already-visible selected oMLX loopback route. Only the exact
  model-selected query/filter arguments or message identity enter the pinned Localmail request.
- This slice adds no Email provenance cards, MCP execution, arbitrary oMLX server tools, attachment access, account or
  sync administration, outbound mail, memory indexing, migration, automatic retrieval, packaging, or release work.

### Verification completed

Focused TDD first failed on oMLX frontend availability, native eligibility, the false Email definition gate, and the
missing generation-scoped Localmail executor argument. Frontend tests now cover oMLX capability reasons and the exact
loopback/pinned-Localmail disclosure. Socket-free Rust tests prove exact provider-call identity, bounded untrusted
result reuse, safe durable audit, and fail-closed behavior without a configured executor. A host-local two-request
Chat Completions fixture passed and confirmed Email-only definition order, one immediate correlated `search_email`
result, final-answer streaming, cumulative usage, and terminal loop completion. It used only synthetic provider and
email data; no real Localmail credential or archive content was accessed.

Prettier, Svelte diagnostics, all 91 tracked frontend tests across 24 files, the production build, Cargo formatting and
check with no warnings, and the full Rust suite pass. The Rust suite has 397 tests: 368 pass by default and 29
loopback, public-network, credential, or live-provider checks remain explicitly opt-in. The literal `npm test` command
also discovered the unrelated untracked `website/tests/rendered-html.test.mjs`, which has no Vitest suite; excluding
that untouched untracked directory produced the recorded 24-file/91-test pass. The focused oMLX Email loopback fixture
and all three existing live oMLX streaming, cancellation, clock, and Memory checks pass from the host.

The browser preview was reviewed at 1320 x 820 and the 720 x 620 native minimum. The expanded four-provider
prerequisite, Email control, and compact status remain visible, accessible, aligned, and contained without horizontal
overflow or browser-console warnings/errors. The disconnected preview cannot satisfy native Localmail/oMLX readiness,
so the enabled loopback disclosure has rendered component coverage rather than an interactive browser toggle.

The native development app compiled, development-signed, launched, and remained running against the existing store.
Immutable read-only inspection returned schema version 21, `quick_check=ok`, zero foreign-key failures, an exact
21-row migration ledger, 17 conversations, 92 messages, 40 provider runs, and 20 tool invocation/result pairs. This
slice adds no schema or live-store mutation. No production pinned-TLS Localmail call was exercised, so actual vault
unlock and authenticated archive reading remain manually unverified.

## Prior completed product slice: Anthropic-compatible Email mapping

### Goal

Let one explicitly tool-capable Anthropic-compatible model use Bottie's two configured read-only Localmail tools after
the existing off-by-default session choice, while preserving native trust, credential, provider-call, WebView, and
shared tool-loop boundaries and making both cloud and pinned-Localmail destinations explicit.

### Implemented shape

1. The Email control is now available for tool-capable Anthropic-compatible models as well as Ollama and
   OpenAI-compatible models after the same secret-free Localmail status confirms saved certificate trust and a vault
   credential. Provider/model changes, missing readiness, oMLX selection, and absent tool capability continue to
   disable it with an actionable path-free reason.
2. Enabled Anthropic-compatible Email states that the prompt and bounded Localmail tool results go to the selected
   cloud endpoint, while only model-selected email queries and exact message identities go to the pinned Localmail
   server. Existing Ollama loopback and OpenAI-compatible cloud disclosures remain unchanged.
3. Native generation rechecks explicit Email intent, exact `ollama|openai|anthropic` routing, discovered per-model
   tool capability, and configured Localmail trust/credential metadata before retaining the request flag or
   constructing one immutable generation-scoped executor. Eligibility still retrieves no credential material.
4. An Email-only Anthropic Messages request advertises `search_email`, then `open_email`, then `current_time`. oMLX
   sessions continue passing a false Email gate and cannot advertise those definitions.
5. Anthropic-emitted calls preserve the exact provider `tool_use` identity in durable invocation and immediate
   correlated `tool_result` blocks. Ordered signed thinking and redacted-thinking blocks survive the follow-up request
   unchanged. Execution reuses the safe policy, strict typed Localmail conversion, pinned fixed-route client, 64 KiB
   redacted envelope, four-round/eight-call/five-minute loop, 256 KiB aggregate ceiling, and shared cancellation.
6. Focused Anthropic Email orchestration tests live in their own cohesive module. No connector route, Tauri IPC
   payload or capability, provider credential behavior, storage schema, or existing Ollama/OpenAI behavior changed.

### Acceptance and explicit exclusions

- Email remains off by default and session-only. Anthropic-compatible availability requires configured pinned trust,
  a saved vault credential, and explicit model tool capability.
- Accepted calls cannot select another Localmail route or weaken certificate pinning, bearer authentication,
  redirect/proxy denial, response/result bounds, identity correlation, inert text, `untrusted: true`, durable audit,
  cancellation, or shared loop policy.
- Prompts and bounded Localmail results use the already-visible selected Anthropic-compatible cloud route. Only the
  exact model-selected query/filter arguments or message identity enter the pinned Localmail request.
- This slice adds no oMLX Email mapping, Email provenance cards, MCP execution, attachment access, account/sync
  administration, outbound mail, memory indexing, migration, automatic retrieval, packaging, or release work.

### Verification completed

Focused TDD first failed on Anthropic-compatible frontend availability, native eligibility, and the false Email gate
in Anthropic's definition mapping. Frontend tests now cover supported/unsupported provider and capability reasons plus
the Anthropic cloud and pinned-Localmail disclosure. Socket-free Rust tests prove exact provider-call identity,
bounded untrusted result reuse, safe durable audit, and fail-closed behavior without a configured executor. A
host-local two-request Messages fixture passed and confirmed Email-only definition order, preserved signed-thinking
and redacted-thinking blocks, one immediate correlated `search_email` result, final-answer streaming, cumulative
usage, and terminal loop completion. It used only synthetic provider and email data; no real Anthropic-compatible
credential or Localmail archive content was accessed.

Prettier, Svelte diagnostics, all 91 frontend tests across 24 files, the production build, Cargo formatting/check with
no warnings, and the full Rust suite pass. The Rust suite has 394 tests: 366 pass by default and 28 loopback,
public-network, credential, or live-provider checks remain explicitly opt-in. The isolated Anthropic-compatible Email
loopback fixture also passes explicitly from the host. Cargo compilation and tests ran on the host after sandboxed
Rust build scripts stalled in environment filesystem/process I/O.

The browser preview was reviewed at 1320 x 820 and the 720 x 620 native minimum. The expanded three-provider
prerequisite reason and Email control remain visible, accessible, aligned, and contained without horizontal overflow
or browser-console warnings/errors. The disconnected preview cannot satisfy native Localmail/Anthropic readiness, so
the enabled cloud disclosure has rendered component coverage rather than an interactive browser toggle.

The native development app compiled, development-signed, launched, and remained running against the existing store.
Immutable read-only inspection returned schema version 21, `quick_check=ok`, zero foreign-key failures, an exact
21-row migration ledger, 17 conversations, 92 messages, 40 provider runs, and 20 tool invocation/result pairs. This
slice adds no schema or live-store mutation. No real remote Anthropic-compatible generation or production pinned-TLS
Localmail call was exercised, so cloud credential unlock and authenticated archive reading remain manually
unverified.

## Prior completed product slice: OpenAI-compatible Email mapping

### Goal

Let one explicitly tool-capable OpenAI-compatible model use Bottie's two configured read-only Localmail tools after the
existing off-by-default session choice, while keeping connector trust, credential, provider-call, WebView, and shared
tool-loop boundaries unchanged and making both network destinations explicit.

### Implemented shape

1. The existing Email control is now available for tool-capable OpenAI-compatible models as well as Ollama after the
   same secret-free Localmail status confirms saved certificate trust and a vault credential. Provider/model changes,
   missing readiness, unsupported providers, and absent tool capability continue to disable it with an actionable
   path-free reason.
2. Enabled Ollama retains its loopback disclosure. Enabled OpenAI-compatible Email states that the prompt and bounded
   Localmail tool results go to the selected cloud endpoint, while only model-selected email queries and exact message
   identities go to the pinned Localmail server.
3. Native generation rechecks explicit Email intent, exact `ollama|openai` routing, discovered per-model tool
   capability, and configured Localmail trust/credential metadata before retaining the request flag or constructing
   one immutable generation-scoped executor. Eligibility still retrieves no credential material.
4. An Email-only OpenAI Chat Completions request advertises `search_email`, then `open_email`, then `current_time`.
   Anthropic-compatible and oMLX sessions continue passing a false Email gate and cannot advertise those definitions.
5. OpenAI-emitted calls preserve the exact provider `tool_call_id` in the durable invocation and correlated tool
   result message. Execution reuses the safe policy, strict typed Localmail conversion, pinned fixed-route client,
   64 KiB redacted envelope, four-round/eight-call/five-minute loop, 256 KiB aggregate ceiling, and shared cancellation.
6. Focused OpenAI Email orchestration tests live in their own cohesive module. No connector route, Tauri IPC payload or
   capability, provider credential behavior, storage schema, or existing Ollama behavior changed.

### Acceptance and explicit exclusions

- Email remains off by default and session-only. OpenAI-compatible availability requires configured pinned trust, a
  saved vault credential, and explicit model tool capability.
- Accepted calls cannot select another Localmail route or weaken certificate pinning, bearer authentication,
  redirect/proxy denial, response/result bounds, identity correlation, inert text, `untrusted: true`, durable audit,
  cancellation, or shared loop policy.
- Prompts and bounded Localmail results use the already-visible selected OpenAI-compatible cloud route. Only the exact
  model-selected query/filter arguments or message identity enter the pinned Localmail request.
- This slice adds no Anthropic-compatible or oMLX Email mapping, Email provenance cards, MCP execution, attachment
  access, account/sync administration, outbound mail, memory indexing, migration, automatic retrieval, packaging, or
  release work.

### Verification completed

Focused TDD first failed on OpenAI-compatible frontend availability, native eligibility, and the absent Localmail
executor in OpenAI's durable round path. Frontend tests now cover supported/unsupported provider and capability reasons
plus both loopback and cloud delivery disclosures. Socket-free Rust tests prove exact provider-call identity, bounded
untrusted result reuse, safe durable audit, and fail-closed behavior without a configured executor. A host-local
two-request Chat Completions fixture passed and confirmed exact Email-only definition order, one correlated
`search_email` result, final answer streaming, cumulative usage, and terminal loop completion. It used only synthetic
provider and email data; no real OpenAI-compatible credential or Localmail archive content was accessed.

Prettier, Svelte diagnostics, all 91 frontend tests across 24 files, the production build, Cargo formatting/check with
no warnings, and the full Rust suite pass. The Rust suite has 390 tests: 363 pass by default and 27 loopback,
public-network, credential, or live-provider checks remain explicitly opt-in. The isolated OpenAI-compatible Email
loopback fixture also passes explicitly from the host.

The browser preview was reviewed at 1320 x 820 and the 720 x 620 native minimum. The widened prerequisite reason and
Email control remain visible, accessible, aligned, and contained without horizontal overflow or browser-console
warnings/errors. The disconnected preview cannot satisfy native Localmail/OpenAI readiness, so the enabled cloud
disclosure has rendered component coverage rather than an interactive browser toggle.

The native development app compiled, development-signed, launched, and remained running against the existing store.
Immutable read-only inspection returned schema version 21, `quick_check=ok`, zero foreign-key failures, an exact
21-row migration ledger, 17 conversations, 92 messages, 40 provider runs, and 20 tool invocation/result pairs. This
slice adds no schema or live-store mutation. No real remote OpenAI-compatible generation or production pinned-TLS
Localmail call was exercised, so cloud credential unlock and authenticated archive reading remain manually unverified.

## Prior completed product slice: Session-only Email enablement and Ollama mapping

### Goal

Let one explicitly tool-capable local Ollama model use Bottie's two configured read-only Localmail tools after an
off-by-default session choice, without exposing a usable route when native trust or credential state is absent and
without widening the provider, WebView, connector, or tool-loop boundaries.

### Implemented shape

1. The composer adds one session-only Email toggle. It is available only for a tool-capable Ollama model after the
   existing secret-free Localmail status confirms a saved HTTPS origin, certificate fingerprint, and vault credential.
   Provider changes, unsupported model changes, and lost connector readiness disable it; browser preview and every
   other provider stay unavailable.
2. The enabled disclosure states that prompts remain with Ollama on loopback, while only model-selected email queries
   and exact message identities go to the pinned Localmail server. No origin, fingerprint, token, query, identity, email
   content, provider call identity, or native path is added to frontend state beyond the existing bounded tool audit.
3. `ChatRequest.emailEnabled` defaults false. Native generation rechecks explicit intent, exact `ollama` routing,
   discovered per-model tool capability, pinned trust, and credential configuration before retaining that flag or
   constructing one immutable generation-scoped Localmail executor. Definition gating inspects vault metadata only; it
   does not retrieve or unlock credential material.
4. Ollama receives `search_email`, then `open_email`, then the existing `current_time` definition when Email alone is
   enabled. OpenAI-compatible, Anthropic-compatible, and oMLX adapters pass an explicit false Email gate even if an
   internal request were malformed, so they cannot advertise or execute these tools in this slice.
5. Ollama-emitted calls enter the existing four-round, eight-call, five-minute, 64 KiB per-result, and 256 KiB aggregate
   loop. Each call is checkpointed before execution, passes through the existing safe policy, strict Localmail argument
   conversion, pinned client, fixed search/open routes, redacted common envelope, and exact ordered Ollama tool-result
   message, then retains its duration/outcome and result before provider reuse.
6. Composer DOM registration, resizing, scrolling, and focus moved into one cohesive interaction helper, while pure
   audit-envelope mapping moved into a focused Rust module; touched source files remain within the practical 500-line
   limit without changing behavior.

### Acceptance and explicit exclusions

- Email is off by default and session-only. It cannot be enabled for a model without explicit Ollama tool capability or
  without both configured certificate trust and a saved vault credential.
- Accepted calls cannot select a different Localmail route or weaken certificate pinning, bearer-header, redirect,
  proxy, response-size, result, identity, inert-text, `untrusted: true`, cancellation, or shared loop policy.
- Prompts stay on the configured Ollama loopback route. Only the exact model-selected query/filter arguments or message
  identity enter the pinned Localmail request; bounded inert results return only to Ollama and the durable local audit.
- This slice adds no OpenAI-compatible, Anthropic-compatible, or oMLX Email mapping, Email provenance cards, MCP
  execution, attachment access, account/sync administration, outbound mail, memory indexing, migration, automatic
  retrieval, packaging, or release work.

### Verification completed

Focused TDD first failed on the absent Email availability helper, composer contract/disclosure, request flag, Localmail
definition gate, and Ollama-only eligibility. The focused frontend tests now pass. Native fixtures prove missing trust,
missing credentials, non-Ollama providers, missing model capability, and absent generation executor all fail closed;
definition gating never retrieves credential material. Socket-free tests prove Localmail execution and durable safe
audit. A host-local two-request Ollama fixture passed and confirmed exact ordered definitions, one exact
`search_email` call, bounded untrusted result correlation, final answer streaming, cumulative usage, and terminal loop
completion.

Prettier, Svelte diagnostics, all 90 frontend tests across 24 files, the production build, Cargo formatting/check with
no warnings, and the full Rust suite pass. The Rust suite has 387 tests: 361 pass by default and 26 loopback,
public-network, credential, or live-provider checks remain explicitly opt-in. The native development app compiled,
development-signed, and launched against the existing store. Immutable read-only inspection returned schema version
21, `quick_check=ok`, zero foreign-key failures, an exact 21-row migration ledger, 15 conversations, 80 messages, 34
provider runs, and 13 tool invocation/result pairs. This slice adds no schema or live-store mutation.

Browser presentation was reviewed at 1320 x 820 and the 720 x 620 native minimum. The disabled Email control was
visible, accessible, aligned, and contained with no horizontal document overflow or browser-console warnings/errors.
The disconnected preview cannot satisfy native Localmail readiness, so the enabled disclosure has rendered component
coverage but was not visually toggled in that preview.

A subsequent native user check reported that authenticated Localmail setup still left Email disabled. Secret-free live
inspection confirmed that certificate trust and the credential-vault configured marker were saved, but the persisted
provider selection was oMLX; this slice deliberately maps Email only to Ollama. Focused TDD now covers an actionable
reason for missing saved readiness, non-Ollama selection, or missing Ollama tool capability. The composer renders that
reason beside the disabled control, and a successful connection test now distinguishes a pasted draft token from the
saved vault token required by Email. The native app rebuilt and launched after the correction, and the local Ollama
catalogue exposed installed tool-capable models. macOS denied assistive access to inspect or switch the native control,
and the in-app browser policy blocked reloading the local preview after this follow-up change, so the new message has
component-render and type-check coverage rather than a second visual review. No email endpoint or real model-selected
Email call was exercised; production pinned-TLS archive reading remains manually unverified.

## Prior completed product slice: Provider-independent Localmail tool definitions and dispatch

### Goal

Prepare the two existing read-only Localmail connector contracts for later explicit provider use without advertising
them to any model, reading the archive automatically, exposing native connector state, or adding provider/UI wiring.

### Implemented shape

1. `localmail_tool_definitions()` returns only `search_email` and `open_email` with closed JSON schemas. Search requires
   one bounded query and result limit from 1 to 20; its optional closed filters preserve the existing sender,
   recipient, subject, complete-date, and attachment-presence contract. Open requires one strict decimal `messageId`
   under the connector's existing signed-bigint identity policy.
2. Strict raw conversion rejects non-objects, missing or unknown fields, JSON null/type mismatches, blank or overlong
   text, malformed or reversed dates, invalid limits, and non-decimal identities. Accepted JSON becomes the exact
   existing `SearchEmailRequest` or `OpenEmailRequest`, then passes the connector's complete validator again before any
   configured executor can run.
3. Both names have explicit safe policy entries because their configured executor can call only the existing fixed
   read-only search and detail routes. Unknown names still fail closed. Neither definition is added to
   `enabled_native_tool_definitions`, so oMLX, Ollama, OpenAI-compatible, and Anthropic-compatible requests cannot see
   or call them in this slice.
4. `ConfiguredLocalmailToolExecutor` retains the config path and credential-vault boundary in Rust, selects only the
   exact existing search/open native function, serializes only its bounded path-free response, and never returns the
   origin, certificate fingerprint, bearer token, response bytes, archive internals, or filesystem detail.
5. `dispatch_localmail_tool` applies policy before strict conversion, executes through an injected provider-independent
   boundary, reuses the common exclusive success/error envelope and 64 KiB serialized ceiling, and maps missing trust,
   credential, timeout, or server availability to fixed `unavailable`; malformed/internal connector outcomes become
   fixed `execution_failed` without reflecting provider-controlled arguments or native diagnostics.

### Acceptance and explicit exclusions

- Malformed calls fail before Localmail configuration or vault access; accepted calls cannot select another route or
  weaken the connector's pinned TLS, bearer-header, redirect, proxy, response-size, result, identity, inert-text, or
  `untrusted: true` policies.
- A successful dispatch contains only the existing path-free connector response and remains inside the shared 64 KiB
  envelope. Connector messages, diagnostics, credentials, origins, fingerprints, paths, raw bodies, HTML, attachment
  details/bytes, account/folder internals, cursors, scores, and Bcc cannot enter the error or success envelope.
- This slice adds no provider advertisement, provider-loop or multi-round wiring, Email composer control, provenance
  UI, MCP execution, attachment access, account/sync administration, outbound mail, memory indexing, schema migration,
  automatic retrieval, packaging, or release work.

### Verification completed

Focused TDD first failed on the absent definitions, raw conversion, safe policy entries, executor boundary, validation
ordering, common envelope, redacted failures, and output ceiling. All eight focused tests now pass, including a
configured-executor fixture that panics if malformed arguments reach connector configuration or the credential vault.

Prettier, Svelte diagnostics, all 88 frontend tests across 23 files, the production build, Cargo formatting/check with
no warnings, and the full Rust suite pass. The Rust suite has 383 tests: 358 pass by default and 25 loopback,
public-network, credential, or live-provider checks remain explicitly opt-in. Immutable read-only inspection returned
schema version 21, `quick_check=ok`, zero foreign-key failures, an exact 21-row migration ledger, 15 conversations, 80
messages, 34 provider runs, and 13 tool invocation/result pairs. This slice adds no schema or live-store mutation.

No native archive, vault, or network call was exercised because no provider can advertise these definitions yet and
the existing isolated connector fixtures already cover the exact fixed search/open routes. No native launch or browser
viewport review was repeated because the slice adds no Tauri command, runtime registration, provider request mapping,
frontend type, or rendered UI. Production pinned-TLS archive reading therefore remains manually unverified.

## Prior completed product slice: Localmail open-email connector contract

### Goal

Open one exact Localmail search result through a useful native read boundary without exposing the saved bearer token,
arbitrary routes, HTML rendering, remote-image URLs, attachments, Bcc, full headers, or archive internals.

### Implemented shape

1. The new typed `open_email` Tauri command accepts only one closed `messageId` field. Rust requires Localmail's strict
   decimal string identity within the signed-bigint range before reading connector settings or the credential vault.
   Search-result decoding now applies the same rule, so every accepted result identity is valid for the open contract.
2. Open loads only the saved normalized HTTPS origin and confirmed certificate fingerprint, retrieves only the
   vault-held Localmail credential, rebuilds the proxy-free, redirect-free pinned client, and sends exactly one
   authenticated `GET /v1/messages/{id}?headers=compact&external_images=false` request. The identity cannot introduce
   a path, query, fragment, or alternate route.
3. Native response reading stops at 512 KiB and requires the response identity to match the request exactly. Subject,
   sender, To, and Cc text reuse the search bounds; To and Cc each retain at most 50 entries; the body retains at most
   32,768 Unicode scalars; and dates normalize to UTC RFC 3339.
4. Plain body text is normalized as inert multiline text. When it is absent, Rust parses the server's sanitized HTML,
   removes executable and embedded elements again, and retains visible text only. HTML markup and image URLs never
   serialize through the command, and the fixed request explicitly keeps Localmail external-image rewriting disabled.
5. The path-free response contains only the exact message identity, subject, sender, To/Cc, UTC date, inert body,
   attachment-presence boolean, and `untrusted: true`. Bcc, raw or full headers, HTML, attachment names, hashes, MIME,
   sizes or bytes, account/folder metadata, raw responses, origins, certificate fingerprints, credentials, and native
   paths are discarded. Diagnostics record only fixed success/failure events.

### Acceptance and explicit exclusions

- One accepted call performs only one read-only detail request against the saved explicitly pinned Localmail
  connection, with compact headers and external images disabled.
- Invalid requests fail before config, vault, or network work. Missing connection or token state fails closed, and a
  mismatched or malformed response identity is rejected.
- Returned headers and body are bounded path-free inert text; no HTML, remote-image URL, Bcc, full header set,
  attachment detail/content, account/folder identity, origin, credential, or native path crosses IPC.
- This slice adds no provider-visible definition, tool dispatcher or multi-round mapping, Email composer control,
  provenance UI, MCP execution, attachment access, account/sync administration, outbound mail, memory indexing, schema
  migration, automatic retrieval, packaging, or release work.

### Verification completed

Prettier, Svelte diagnostics, all 88 frontend tests across 23 files, the production build, Cargo formatting/check, and
the full Rust suite pass. The Rust suite has 375 tests: 350 pass by default and 25 loopback, public-network,
credential, or live-provider checks remain opt-in. Six new default Localmail open tests pass. The new isolated
loopback fixture also passed separately from the host and confirmed exactly one authenticated fixed-route GET, compact
headers, external images disabled, bounded response decoding, exact identity correlation, and inert content mapping.
It used a synthetic token and synthetic email; no real Localmail credential or archive content was accessed.

The native development app compiled, development-signed, and launched against the existing store with the new command
registered. Immutable read-only inspection returned schema version 21, `quick_check=ok`, zero foreign-key failures,
and an exact 21-row migration ledger. This slice adds no schema or live-store mutation. The production pinned-TLS open
was not exercised with a real Localmail bearer token, so actual vault unlock and authenticated archive reading remain
manually unverified. No browser viewport review was repeated because this Rust-only slice changes no rendered UI.

## Prior completed product slice: Localmail search connector contract

### Goal

Add one useful first-party Localmail read boundary without exposing the saved bearer token, arbitrary Localmail routes,
email bodies, HTML, attachments, or archive internals to the WebView.

### Implemented shape

1. The new typed `search_email` Tauri command accepts a required query, optional sender, recipient, subject, strict
   after/before date, and attachment-presence filters, plus a result limit from 1 to 20. Serde rejects unknown request
   and filter fields. Rust normalizes whitespace, caps the query at 500 Unicode scalars and textual filters at 320, and
   validates complete dates and ordering before reading connector settings or the credential vault.
2. Search loads only the saved normalized HTTPS origin and confirmed certificate fingerprint, retrieves only the
   vault-held Localmail credential, rebuilds the existing proxy-free, redirect-free pinned client, and sends exactly
   one authenticated `POST /v1/search`. The JSON body contains only normalized query, supported filters, and limit;
   it never enables cursor pagination, smart rewriting, alternate sorting, account/folder selection, or another route.
3. Native response reading stops at 128 KiB. Rust ignores server account, folder, recipient, score, matched-arm, cursor,
   estimate, body, and attachment-detail fields. Each retained result contains only a strict path-safe opaque message
   identity, bounded plain subject and sender metadata, a normalized UTC date, HTML-free visible snippet text, and one
   attachment-presence boolean. At most the requested count is returned and the whole response is marked untrusted.
4. The bearer value exists only in a sensitive native Authorization header. Tauri receives no origin, certificate,
   credential, raw Localmail response, filesystem path, email body, HTML, attachment name/content, or archive-internal
   identity beyond the bounded opaque message ID. Diagnostics record only fixed success/failure events.
5. Focused tests cover the closed request shape, bounds, dates, validation-before-credential ordering, exact fixed
   method/route/header/body construction, response-byte/result/text limits, malformed identities and dates, inert HTML
   removal, and absence of every deliberately discarded field. One opt-in loopback fixture executes the actual
   request/stream/decode path without reading a real archive.

### Acceptance and explicit exclusions

- One accepted call performs only one first-page read-only search against the saved explicitly pinned Localmail
  connection. No automatic search, cursor follow-up, rewrite request, or alternate route is possible.
- Invalid requests fail before config, vault, or network work. Missing connection or token state fails closed.
- Results are bounded path-free inert summaries; no body, HTML, attachment content/name, account/folder identity,
  ranking detail, or cursor crosses IPC.
- This slice adds no `open_email`, provider-visible definition, tool dispatcher or multi-round mapping, Email composer
  control, provenance UI, MCP execution, attachment access, account/sync administration, outbound mail, memory indexing,
  schema migration, automatic retrieval, packaging, or release work.

### Verification completed

Prettier, Svelte diagnostics, all 88 frontend tests across 23 files, the production build, Cargo formatting/check, and
the full Rust suite pass. The Rust suite has 368 tests: 344 pass by default and 24 loopback, public-network,
credential, or live-provider checks remain opt-in. Six new default Localmail search tests pass. The new isolated
loopback search fixture also passed separately from the host and confirmed exactly one `POST /v1/search`, the
sensitive bearer header, bounded request body, streamed response decoding, inert snippet mapping, and absence of
cursor/smart fields. It used a synthetic token and synthetic email result; no real Localmail credential or archive data
was accessed.

The native development app compiled, development-signed, and launched against the existing store with the new command
registered. Immutable read-only inspection returned schema version 21, `quick_check=ok`, zero foreign-key failures,
an exact 21-row migration ledger, 14 conversations, 74 messages, 31 provider runs, and 12 tool invocation/result pairs.
This slice adds no schema or live-store mutation. The production pinned-TLS search was not exercised with a real
Localmail bearer token, so actual vault unlock and authenticated archive search remain manually unverified. No browser
viewport review was repeated because this Rust-only slice changes no rendered UI.

## Prior completed product slice: Localmail connection and authentication

### Goal

Establish an explicit first-party Localmail server and bearer-authentication boundary without reading email, exposing a
token to the WebView, or adding a model-visible tool.

### Implemented shape

1. Settings now includes one focused Localmail archive control. It accepts only an explicit HTTPS origin without
   credentials, a path, query, or fragment; changing the origin invalidates any inspected certificate draft.
2. Native certificate inspection calls only `/v1/version`, accepts the presented leaf certificate for that inspection,
   proves handshake key possession, and returns its SHA-256 fingerprint for comparison through another trusted channel.
   Confirmed connections pin that exact certificate. Redirects and ambient proxies are disabled, connection/request
   timeouts are fixed, and response metadata is capped at 32 KiB.
3. The optional bearer token is trimmed, header-validated, and capped at 4,096 bytes before entering the existing
   operating-system credential vault. `localmail.json` retains only the normalized origin and certificate fingerprint;
   status IPC exposes only configuration/unlock booleans. On macOS, an existing locked token reuses Bottie's Touch ID
   session gate.
4. The Test action calls `/v1/version` and, only when a draft or saved token exists, `/v1/auth/whoami`. It returns a
   bounded authenticated username and server version, never the token or email. Native diagnostics record only fixed
   success/failure categories without origins, fingerprints, usernames, response bodies, or credentials.
5. Focused Rust tests cover origin, fingerprint, token, persistence/reopen/removal, conflict, and certificate-verifier
   policy. A server-rendered component test covers the explicit HTTPS, certificate confirmation, vault, and no-email
   disclosure. One opt-in live test exercises the real TLS inspection and Localmail version contract.

### Acceptance and explicit exclusions

- Certificate trust is explicit and is rechecked for each tested connection; a changed certificate fails closed
  until it is inspected and confirmed again.
- A token is never serialized into connector settings, returned over Tauri IPC, recorded in diagnostics, or sent to the
  unauthenticated version route.
- Setup and testing read no email. This slice adds no `search_email`/`open_email`, tool definition, dispatcher entry,
  provider-loop mapping, Email toggle/provenance card, MCP execution, attachment route, outbound mail, memory indexing,
  SQLite schema, or Localmail account/sync administration.

### Verification completed

Prettier, Svelte diagnostics, all 88 frontend tests across 23 files, the production build, Cargo formatting/check, and
the full Rust suite pass. The Rust suite has 361 tests: 338 pass by default and 23 loopback, public-network,
credential, or live-provider checks remain opt-in. The seven default Localmail tests cover the closed native boundary.
The ignored live Localmail identity probe also passed from the host against the running loopback server with exactly one
unauthenticated `/v1/version` call; it confirmed API 1.0 without requesting a bearer token or reading email.

The native development app compiled, development-signed, and launched against the existing store. Immutable read-only
inspection returned schema version 21, `quick_check=ok`, zero foreign-key failures, an exact 21-row migration ledger,
13 conversations, 72 messages, 30 provider runs, and 12 tool invocation/result pairs. This slice adds no schema or
live-store mutation. The native Localmail bearer-authentication flow was not exercised with a real token, so actual OS
vault persistence/Touch ID and `/v1/auth/whoami` remain manually unverified. The in-app browser control runtime was not
available in this session, so the new settings surface has server-rendered component coverage but no fresh automated
desktop/compact viewport inspection.

## Prior completed planning slice: Migration rollback

### Goal

Define a fail-closed, testable migration safety model before changing startup storage behavior, without claiming older
binaries can read newer schemas or bundling the implementation.

### Accepted shape

1. `MIGRATION-ROLLBACK.md` classifies user-owned source data, rebuildable derived data, and application-private files.
   Schema migrations remain forward-only and may not mutate attachment files or the embedding-model cache.
2. Older supported stores will be preflighted read-only and copied through SQLite's online backup API. Pending schema
   work and Rust backfills will run only on an isolated same-volume candidate before live data changes.
3. Candidate validation requires SQLite integrity and foreign-key checks, exact current schema and ledger state, the
   built-in profile, semantic-contract validation, and migration-specific source-data invariants.
4. A verified source-version recovery point plus an atomic native-only promotion marker will precede candidate restore
   into the live database. Restart reconciliation will validate the promoted target or restore the source recovery
   point before ordinary corruption classification.
5. The attachment tree remains unchanged and valid for either database version. The two newest completed migration
   recovery points stay separate from automatic-backup rotation, and cleanup recognizes only exact managed names.
6. Workers, retention, garbage collection, automatic backup rotation, provider discovery, and WebView setup remain
   after successful live-store validation. The first implementation adds no Tauri command or migration UI.

### Acceptance criteria for the staged rollback implementation

- No pending migration mutates the live database before a candidate and source-version recovery point both validate.
- Copy, migration, validation, safety-copy, promotion, and process-interruption fault points preserve or restore exact
  source data without changing attachment files.
- Historical schema fixtures, WAL content, ledger coherence, current semantic metadata, newer-schema rejection,
  corruption routing, strict cleanup, and source/derived-data policy receive path-backed coverage.
- Migration outcomes and errors remain path-, SQL-, filename-, content-, hash-, and credential-redacted; no WebView
  migration capability is added.
- A disposable real-store copy passes schema, integrity, foreign-key, ledger, and row-parity checks before native
  launch is allowed to migrate the live store.

This planning gate defined the storage slice that was subsequently implemented after native clock and primary-provider
tool parity.

### Explicit exclusions

At the time, this planning slice changed no Rust, TypeScript, Svelte, SQLite schema, Tauri capability, live-store data,
provider, credential, tool, attachment, backup, recovery, worker, packaging, or release behavior. Reverse migrations,
automatic binary downgrade, and migration-recovery UI remain absent after the implementation slice.

### Verification completed

The plan was reconciled against the current schema-version-21 migration orchestrator, manual restore staging, portable
backup boundary, corruption recovery, startup order, attachment worker, and semantic worker. Documentation formatting
and the repository's full standard validation set were run without launching the native app or touching the live
store. Browser and live-provider checks are not applicable because this slice changes no presentation, networking,
credentials, IPC, native behavior, or database bytes.

## Prior completed product slice: First-run provider and privacy setup

### Goal

Require a new native installation to understand and confirm its concrete provider route plus Bottie's privacy
boundaries before chatting, without treating existing settings as a new install or weakening the native/WebView split.

### Implemented shape

1. Secret-free `ProviderSettings` now includes `setupCompleted`. A genuinely absent settings file uses the new default
   `false`, while settings serialized before this field existed deserialize as `true`; legacy users therefore keep
   their established startup path. Normalization and every later settings or remembered-selection write preserve the
   value.
2. After healthy storage initialization and model discovery, one modal gate shows the concrete provider/model and a
   local or cloud route. The completion control remains unavailable until discovery has produced a usable model; an
   honest offline state directs users to the existing full provider Settings flow instead of simulating readiness.
3. The disclosure distinguishes prompt, normalized-image, and explicitly enabled tool-result delivery from Bottie's
   local conversation, attachment, and derived-memory storage. It states that Memory and Web begin disabled for every
   frontend session and that cloud and search credentials stay in the operating-system vault. It neither enables a
   tool nor stores disclosure text.
4. One argument-free `complete_first_run_setup` command reads the active Rust-owned settings, requires an already
   remembered provider/model pair, persists only the completion marker, updates active native state, and returns the
   existing secret-free settings shape. No generic settings, filesystem, credential, or database capability is added.
5. Setup completion/progress/error state is isolated from the established 500-line conversation page-state boundary.
   The modal has no dismiss path around the acknowledgement and traps keyboard focus within its actions. Full Settings
   temporarily replaces it and returns to the same first-run gate after saving or cancellation.

### Acceptance criteria

- A new settings default requires setup; a settings file written before this feature opens normally without a false
  onboarding interruption.
- Setup cannot complete without an available remembered provider/model pair, and successful completion survives an
  independent settings reload.
- Users see local versus cloud routing and the provider/local-data/Memory/Web/credential boundaries before the composer
  becomes usable; offline users get a real provider-configuration path.
- The WebView sends no credential, path, settings payload, provider response, attachment byte, extracted text, hash,
  SQL, or database value to complete setup.
- No SQLite migration, provider endpoint or wire mapping, credential behavior, Memory/Web default, automatic
  retrieval, oMLX Web mapping, model-cache deletion, document opening, attachment retry, MCP runtime, rollback
  implementation, packaging, or release work is added.

### Verification completed

Focused TDD first failed on the absent settings marker, legacy compatibility behavior, persistent completion helper,
and first-run disclosure component. Native tests now prove that new defaults require setup, legacy JSON defaults to
completed, a missing selection cannot create a settings file, and a configured completion persists across an
independent reload. Server-rendered frontend coverage verifies the cloud-route/privacy disclosure and the disabled,
provider-settings-directed offline state.

The full standard checks pass: Prettier, Svelte diagnostics with zero errors or warnings, all 87 frontend tests, the
production build, Cargo formatting/check, and 316 Rust tests with 296 passing and 20 explicit opt-in tests skipped.
No schema migration, credential, endpoint, provider request, tool result, or live-store format changed.

The browser preview was inspected with a temporary native-gate presentation fixture at 1320 x 820 and the 720 x 620
native minimum. The offline state, all disclosures, provider-Settings handoff, disabled completion, and action row are
fully contained; the minimum-size card requires no internal scroll and the console has no warnings or errors. The
temporary fixture was removed after inspection. The signed native development app compiled, launched, and remained
running against the existing installation. Its pre-feature provider settings contained a remembered oMLX/model pair,
no completion field, and no credential-shaped field, exercising the legacy-acknowledged load path. Immutable live-store
inspection reported schema version 21, `quick_check=ok`, nine conversations, and 56 messages. A real new-install
completion click was not performed against the user's established settings; the path-backed native persistence test
covers that mutation. Keyboard verification also confirmed that the first Tab enters the setup actions, forward and
reverse traversal remain contained, and opening full Settings removes the first-run dialog until Settings closes.

## Prior completed product slice: User-configurable Web destination policy

### Goal

Let users narrow Web search and fetch destinations without weakening Bottie's fixed public-network protections,
exposing native network detail to the WebView, or allowing settings to change an accepted generation mid-run.

### Implemented shape

1. `WebNetworkPolicy` persists with the existing secret-free provider settings. HTTPS-only defaults on for new and
   older settings files. Optional allowed and blocked lists accept at most 32 combined normalized public DNS names;
   parent domains include their subdomains, exact duplicates/conflicts fail validation, and blocked domains win.
2. Every explicitly Web-enabled generation snapshots the normalized policy beside its concrete Brave/Exa search and
   credential-free fetch executors. A later Settings write cannot change the route of that accepted run.
3. Search adapters retain their existing model-selected query filters and safe-result normalization. The common native
   dispatcher then drops every normalized result that violates HTTPS, allowlist, or blocklist policy before the result
   can serialize into the durable tool envelope or provider follow-up.
4. `web_fetch` applies the same policy before DNS or HTTP work, rechecks each explicitly resolved redirect target, and
   verifies the final provider result before serialization. Disabling HTTPS-only permits public HTTP but cannot permit
   IP literals, private/loopback/special-use names or addresses, mixed non-public DNS answers, non-default ports,
   embedded credentials, ambient proxies, automatic redirects, or any other fixed baseline rejection.
5. Settings renders one focused path-free policy control with the HTTPS default, line/comma-delimited allowed and
   blocked domains, parent/precedence guidance, and an explicit immutable private-network boundary. The WebView never
   receives DNS answers, resolved addresses, request data, headers, source content, credentials, or native detail.

### Acceptance criteria

- Older settings load with HTTPS-only enabled, saved domain names normalize deterministically, invalid or over-limit
  policies fail without being activated, and the settings file remains secret-free.
- Search results cannot cross the common dispatcher when their exact normalized URL violates the saved policy.
- A blocked fetch fails before provider work; the same immutable policy is reapplied to every redirect and final URL.
- Turning off HTTPS-only restores only public HTTP support and cannot relax Bottie's native public-address baseline.
- Settings remains accessible and horizontally contained at the desktop default and 720 x 620 native minimum.
- No schema migration, new Tauri command, credential, provider definition/wire mapping, approval UI, oMLX mapping,
  automatic retrieval, model-cache deletion, document opening, attachment retry, MCP runtime, packaging, or release
  work is added.

### Verification completed

Focused TDD first failed on the absent policy model, older-settings default, domain validation/matching, dispatcher
filter, pre-network fetch rejection, draft cloning/parsing, and Settings controls. Native coverage now exercises
HTTPS defaults and relaxation, allow/block precedence, unsafe/conflicting/over-limit settings, secret-free
serialization, pre-envelope search filtering, and pre-network fetch rejection. Frontend coverage verifies isolated
nested drafts plus the complete labelled Settings surface.

The full standard checks pass: Prettier, Svelte diagnostics with zero errors or warnings, all 85 frontend tests, the
production build, Cargo formatting/check, and 314 Rust tests with 294 passing and 20 explicit opt-in tests skipped. No
schema migration, credential, provider endpoint, database record, tool definition/result shape, or live-store format
changed.

The browser preview was inspected at 1280 x 720 and the 720 x 620 native minimum. The policy card and both text areas
remain legible inside the existing bounded Settings scroll region, HTTPS is visibly enabled by default, document and
settings content have no horizontal overflow, and the console has no warnings or errors. The signed native development
app compiled and launched successfully. Immutable live-store inspection reported schema version 21 and
`quick_check=ok`, 22 provider runs, and no retained `web_search` or `web_fetch` call; the policy is stored in the
existing native provider-settings file, so no database migration or live-store mutation was required. Live Web calls
were not made because they require a selected search credential and would not add coverage beyond the deterministic
native policy/dispatcher boundaries.

## Prior completed product slice: Durable claim-level Web citations

### Goal

Connect Web-grounded claims to exact successful native sources and retain those links with the conversation without
adding a citation schema, exposing tool identities, or treating arbitrary provider-authored links as trusted
provenance.

### Implemented shape

1. After native discovery confirms an explicitly Web-enabled Ollama, OpenAI-compatible, or Anthropic-compatible model
   supports tools, Rust prepends one fixed system instruction asking for an inline Markdown link immediately after
   each Web-grounded claim. The model must use the exact result URL and must not cite unsupported claims.
2. The assistant's existing checkpointed Markdown retains each produced link unchanged, while the same response's
   append-only successful `web_search` and `web_fetch` results retain exact native provenance. No migration or new
   Tauri command is required, and oMLX receives no Web instruction.
3. One pure Markdown helper extracts normalized HTTP(S) link destinations. The response renderer applies citation
   treatment only when a fragment-free destination exactly matches a safe Web source derived from that response's
   successful durable result. Unsafe, email, unmatched, failed, malformed, and other-response links cannot acquire the
   citation marker.
4. Matching Context cards show `Cited in response`; ordinary source cards and ordinary safe external links remain
   unchanged. Copy and Markdown/JSON export already preserve the exact answer Markdown and tool audit, so the durable
   link survives restart, branch switching, and portable output without an opaque native/provider call identity.

### Acceptance criteria

- Web-disabled requests, oMLX, and models without explicit native tool capability receive no citation instruction.
- A citation marker requires both an accepted assistant Markdown link and an exact normalized URL from the same
  response's successful selected-lineage native Web result; a model-authored external link alone is not provenance.
- Citations survive completion and conversation reopen through existing answer and tool-result persistence, with no
  schema migration or WebView-owned citation record.
- The marked link remains an isolated safe external link, source-card removal stays session-local, and copy/export
  preserve exact Markdown without exposing query, arguments, provider/native call identity, path, header, source
  bytes, credential, or database detail.
- User-configurable domain/network policy, oMLX mapping, automatic retrieval, model-cache deletion, document opening,
  attachment retry, and general MCP interoperability remain outside the slice.

### Verification completed

Focused TDD first failed on absent native citation guidance, Markdown destination extraction, exact response-owned
source matching, inline citation treatment, and Context-card citation state. The focused native and frontend suites now
cover Web enablement, normalized fragments, unmatched and unsafe links, cited versus uncited source cards, and the
fixed path-free presentation contract.

The full standard checks pass: Prettier, Svelte diagnostics with zero errors or warnings, all 83 frontend tests, the
production build, Cargo formatting/check, and 308 Rust tests with 288 passing and 20 explicit opt-in tests skipped. No
schema migration, Tauri command, credential, endpoint, tool result, or live-store format changed.

The browser preview was inspected at 1280 x 720 and the 720 x 620 native minimum. The inline citation and matching
`Cited in response` source label remain legible, the Context panel retains independent scrolling, document width
matches each viewport without horizontal overflow, and the browser console has no warnings or errors. A real
model-generated Web citation was not exercised because that would require a live provider and selected search
credential; native request-shape and durable reopen behavior are covered at their existing typed boundaries. The
signed native development app compiled and launched successfully. Immutable live-store inspection reported schema
version 21, `quick_check=ok`, 22 provider runs, and no retained `web_search` or `web_fetch` call; no migration was added.

## Prior completed product slice: Calm untrusted Web-content presentation

### Goal

Make Bottie's explicitly untrusted fetched-page content recognizable where users inspect it, without adding content
heuristics, changing native execution, or implying that a presentation warning blocks model prompt injection.

### Implemented shape

1. The pure Web-provenance adapter retains one boolean trust marker only after accepting the existing exact successful
   `web_fetch` dispatcher envelope with `untrusted: true`. Search results remain unmarked, and failed, malformed, or
   legacy fetch payloads cannot create a warning.
2. Each fetched-page Context card now pairs a restrained `Untrusted content` label with the plain explanation that
   external page text may contain misleading instructions. The source title, inert excerpt, public host, removal,
   deduplication, and safe external-link behavior remain unchanged.
3. The expandable durable `web_fetch` audit applies the same exact-envelope check, presents the explanation before raw
   payload disclosure, and names that disclosure `Untrusted result`. The result remains inert formatted JSON and is not
   reinterpreted or executed by the WebView.
4. The browser-preview fixture includes one exact successful fetched-page result that supersedes its matching search
   result, so both primary presentation surfaces can be reviewed without a live provider or public network request.

### Acceptance criteria

- Explicitly marked successful fetched-page content is labelled in both the derived source card and durable audit;
  ordinary search results, failed calls, and unmarked fetch payloads do not receive the label.
- The UI explains the risk calmly without claiming that warning text, inert extraction, or the native marker prevents a
  model from following malicious page instructions.
- No query, raw arguments, provider/native call identity, path, header, source bytes, credential, or database detail is
  added to the Context-card model or other new WebView state.
- No Rust code, schema migration, Tauri command, provider mapping, request prompt, execution policy, setting, automatic
  retrieval, claim-level citation, domain/network control, oMLX mapping, model-cache deletion, document opening, or
  attachment retry is added.

### Verification completed

Focused TDD first failed on the absent card marker, exact audit-envelope check, contextual warning, and result label.
Pure and server-rendered coverage now distinguishes exact successful fetched results from search, failure, malformed,
and unmarked payloads and verifies both user-facing surfaces.

The full standard checks pass: Prettier, Svelte diagnostics with zero errors or warnings, all 80 frontend tests, the
production build, Cargo formatting/check, and 287 Rust tests with 20 explicit opt-in tests skipped. No native code,
schema migration, provider request, or live-store state changed.

The browser preview was inspected at 1280 x 720, 900 x 800, and the 720 x 620 native minimum. The fetched-page card and
expanded audit warning remain legible, the responsive Context overlay has no horizontal overflow, its middle content
retains independent scrolling, and the browser console has no warnings or errors.

## Prior completed product slice: Path-free removable Web source cards

### Goal

Replace the Context panel's missing Web provenance with visible, removable, path-free source cards derived only from
successful selected-lineage durable `web_search` and `web_fetch` results, without changing native execution,
persistence, provider mapping, or trust policy.

### Implemented shape

1. A pure frontend adapter accepts only exact successful dispatcher envelopes for `web_search` and `web_fetch`.
   Search results expose only title, public HTTP(S) URL, host, bounded inert snippet, and optional publication metadata;
   fetched pages require the native `untrusted: true` marker and expose the matching bounded inert fields.
2. Unsafe schemes, credential-bearing URLs, localhost names, IP literals, failed results, malformed payloads, and
   fetches without the explicit untrusted marker do not create cards. Fragments are removed before presentation and
   source identity is the normalized URL; no provider ID, query, arguments, path, header, or native call identity is
   retained in the card model.
3. Selected-lineage messages and their tool activity are read newest-first. One fetched page supersedes the earlier
   search result for the same URL, and repeated results deduplicate without inventing claim-level links. A focused
   `WebToolState` keeps source dismissal session-local, so removing a card cannot rewrite the append-only audit.
4. The Context panel renders a dedicated Web sources count, linked title, inert clamped excerpt, optional publication
   value, and source host. External links retain `target="_blank"` plus `rel="noopener noreferrer"`. The existing
   middle content is now one bounded scroll region, keeping the header and footer fixed while allowing attachments,
   memory citations, Web sources, and the privacy route to remain fully reachable at shorter desktop heights.

### Acceptance criteria

- Only successful selected-lineage native Web results create cards; malformed, failed, local, or unsafe legacy
  payloads cannot create visible or navigable sources.
- Search and fetch results deduplicate by normalized fragment-free URL, with the later fetched page supplying the
  richer card. Removing a card immediately shows the accurate empty state without deleting durable tool activity.
- The card model and rendered UI expose no query, provider ID, raw arguments, native/provider call identity, path,
  header, raw page source, or database detail.
- No Rust code, schema migration, Tauri command, provider mapping, Web setting, automatic retrieval, claim-level
  citation linking, prompt-injection presentation, oMLX mapping, model-cache deletion, document opening, or attachment
  retry is added.

### Verification completed

Focused TDD first failed on the absent Web-provenance adapter and Context-panel section. Pure tests now cover exact
search and fetch envelopes, safe URL normalization, fetch-over-search deduplication, dismissal filtering, failed and
malformed results, local/file URLs, and the required untrusted fetch marker. Server-rendered component coverage checks
the dedicated source card, safe link attributes, hidden opaque identity, empty state, and bounded shared scroll region.

The full standard checks pass: Prettier, Svelte diagnostics with zero errors or warnings, all 79 frontend tests, the
production build, Cargo formatting/check, and 287 Rust tests with 20 explicit opt-in tests skipped. No native code or
migration changed.

The browser preview was inspected at 1280 x 720 and 900 x 800. The combined Context-panel content scrolls independently
between its fixed header and footer, both memory and Web cards remain fully reachable, the responsive overlay has no
horizontal overflow, and session-local removal replaces the last Web card with its empty state while the durable tool
activity remains visible. The signed native development app compiled and launched successfully. Immutable live-store
inspection reported schema version 21 and `quick_check=ok`, with no retained successful Web result; therefore no native
source-card interaction is claimed.

## Prior completed product slice: Native inert web-page extraction

### Goal

Replace raw HTML/XHTML page source in successful `web_fetch` results with bounded inert visible text, final source URL,
optional title, and optional publication metadata without changing network policy, provider mapping, IPC, storage
schema, settings, or automatic retrieval behavior.

### Implemented shape

1. The existing 48 KiB response-source ceiling, strict UTF-8 media policy, public-address validation, DNS pinning,
   explicit redirects, and shared timeout remain unchanged. The accepted source stays Rust-only and is parsed before
   common-envelope serialization or durable provider reuse.
2. A tolerant HTML tree parser now handles both HTML and XHTML. Open Graph and Twitter title metadata take precedence
   over the document title. Recognized article/date meta fields and semantic publication-time elements supply one
   optional publication value; both metadata fields are normalized and treated as untrusted.
3. Script, style, template, noscript, SVG, MathML, canvas, frame, object, and embed nodes are removed before visible body
   formatting. Entities are decoded, control characters and excess whitespace are normalized, paragraph boundaries
   remain inert text, and UTF-8-safe ceilings retain at most 24 KiB of content, 512 bytes of title, and 256 bytes of
   publication metadata. Plain-text responses use the same normalization and content ceiling.
4. The common result now contains `sourceUrl`, optional `title`, optional `publishedAt`, inert `content`, and
   `untrusted: true`. Raw markup and response media type no longer enter provider follow-up messages or new durable
   tool results. Existing Ollama, OpenAI-compatible, and Anthropic-compatible loops retain their exact correlation,
   audit, persistence, cancellation, and aggregate-output behavior.

### Acceptance criteria

- HTML/XHTML tags and executable or embedded element content cannot survive in the returned content; entity-decoded
  visible text, normalized metadata, and plain-text paragraph boundaries remain available.
- Content and metadata truncate only at valid UTF-8 boundaries before the existing 64 KiB common-envelope ceiling.
- Source bytes, paths, headers, DNS answers, and provider/network detail remain inside Rust; no new Tauri command,
  WebView state, schema migration, setting, credential, or endpoint is added.
- oMLX mapping, dedicated web-source cards, claim-level citation linking, automatic retrieval injection,
  prompt-injection presentation, model-cache deletion, document opening, and attachment retry remain outside this
  slice.

### Verification completed

Focused TDD first failed on the absent extraction contract and result metadata. Pure tests now cover HTML, XHTML,
plain text, entity decoding, metadata precedence and semantic publication time, executable/embedded-node removal,
whitespace and control-character normalization, and UTF-8-safe content/title/publication ceilings. Dispatcher and all
three mapped-provider durable fixtures assert the inert result shape rather than raw markup.

The full standard checks pass: Prettier, Svelte diagnostics with zero errors or warnings, all 76 frontend tests, the
production build, Cargo formatting/check, and 287 Rust tests with 20 explicit opt-in tests skipped. The restricted
sandbox could not bind loopback fixture sockets; the authorized host-local retry passed all seven relevant ignored
tests, including the three mapped-provider two-request loops, redirect/timeout/size policy, and one direct
production-policy IANA HTTPS fetch whose returned content contained no markup.

The signed native development app compiled and launched successfully. Immutable live-store inspection reported schema
version 21, `quick_check=ok`, 22 provider runs, and no retained `web_fetch` call; no migration was added. No browser
presentation review was repeated because this slice changes no frontend surface. No manual model-initiated
`web_fetch` interaction is claimed.

## Prior completed product slice: Anthropic-compatible web-fetch generation mapping

### Goal

Advertise and execute the closed native `web_fetch` contract through Anthropic Messages' existing durable Web loop
without changing oMLX mapping, WebView IPC, settings, storage schema, or page-source representation.

### Implemented shape

1. `AnthropicToolSession` now appends the closed `web_fetch` definition immediately after `web_search` only when Web
   is explicitly enabled. Memory definitions retain their existing order, and Web-disabled requests advertise neither
   Web tool.
2. Native generation constructs the credential-free public-network fetch executor for explicitly Web-enabled,
   discovered tool-capable Ollama, OpenAI-compatible, or Anthropic-compatible models. The existing selected search
   provider and credential remain required by the Web flow; no fetch credential, endpoint, setting, command, or
   WebView state was added.
3. Anthropic calls retain their exact provider `tool_use` identities, typed arguments, safe audit policy, terminal
   outcome, native-work duration, and bounded result before reuse. The provider follow-up contains the accumulated
   assistant blocks followed by the exactly correlated user `tool_result` block; native structured failures retain
   Anthropic's `is_error` signal.
4. The mapped path reuses the closed URL validator, safe execution policy, DNS/address-pinned `NativeWebFetch`, common
   64 KiB result envelope, cumulative provider usage, and existing eight-call, four-round, 256 KiB aggregate,
   five-minute, and cancellation limits.

### Acceptance criteria

- Web-disabled requests and models without explicit tool capability cannot advertise or execute `web_fetch`.
- Anthropic-compatible requests receive the existing closed search/fetch schemas and exact untrusted result envelope;
  URLs and page source remain inside Rust/provider tool traffic and the durable audit rather than crossing a new Tauri
  command.
- oMLX mapping remains absent. A provider-emitted unadvertised fetch call closes through the fixed redacted
  `unsupported_tool` result when no fetch executor is present.
- No migration, frontend state, settings, search-provider behavior, extraction, source card, citation, or automatic
  retrieval behavior is added.

### Verification completed

Focused TDD first failed on the Anthropic definition order, provider fetch gate, and executor boundary. Socket-free
coverage now proves successful fetch execution, exact `tool_use` identity correlation, durable safe audit/result
reopen, and the explicit mapped-provider-versus-oMLX boundary. Protocol coverage proves closed
memory/search/fetch definition order. All three host-local two-request Anthropic SSE fixtures pass, covering existing
memory and search reuse plus the new fetch path's streamed call accumulation, exactly correlated `tool_result` reuse,
explicit `untrusted: true`, final answer streaming, and cumulative usage. The sandboxed fetch-fixture run was blocked
from binding its local socket; the authorized host-local retry passed.

The full standard checks pass: Prettier, Svelte diagnostics with zero errors or warnings, all 76 frontend tests, the
production build, Cargo formatting/check, and 284 Rust tests with 20 explicit opt-in tests skipped. The signed native
development app compiled and launched successfully. Immutable live-store inspection reported schema version 21,
`quick_check=ok`, 22 provider runs, and no retained `web_fetch` call; no migration was added. No browser presentation
review was repeated because this slice changes no frontend surface. No live remote Anthropic-compatible credential,
public-fetch interaction, or manual model-initiated `web_fetch` interaction is claimed.

## Prior completed product slice: OpenAI-compatible web-fetch generation mapping

### Goal

Advertise and execute the closed native `web_fetch` contract through OpenAI Chat Completions' existing durable Web
loop without changing the Anthropic-compatible or oMLX mappings, WebView IPC, settings, storage schema, or page-source
representation.

### Implemented shape

1. `OpenAiToolSession` now appends the closed `web_fetch` definition immediately after `web_search` only when Web is
   explicitly enabled. Memory definitions retain their existing order, and Web-disabled requests advertise neither
   Web tool.
2. Native generation constructs the credential-free public-network fetch executor only for explicitly Web-enabled,
   discovered tool-capable Ollama or OpenAI-compatible models. Anthropic-compatible requests still receive no fetch
   executor. The existing selected search provider and credential remain required by the Web flow; no fetch
   credential, endpoint, setting, command, or WebView state was added.
3. OpenAI calls retain their exact provider call identities, typed arguments, safe audit policy, terminal outcome,
   native-work duration, and bounded result before reuse. The provider follow-up contains the accumulated assistant
   `tool_calls` message followed by the exactly correlated `role: tool` result.
4. The mapped path reuses the closed URL validator, safe execution policy, DNS/address-pinned `NativeWebFetch`, common
   64 KiB result envelope, cumulative provider usage, and existing eight-call, four-round, 256 KiB aggregate,
   five-minute, and cancellation limits.

### Acceptance criteria

- Web-disabled requests and models without explicit tool capability cannot advertise or execute `web_fetch`.
- OpenAI-compatible requests receive the existing closed search/fetch schemas and exact untrusted result envelope;
  URLs and page source remain inside Rust/provider tool traffic and the durable audit rather than crossing a new Tauri
  command.
- Anthropic-compatible and oMLX mapping remain absent. A provider-emitted unadvertised fetch call closes through the
  fixed redacted `unsupported_tool` result when no fetch executor is present.
- No migration, frontend state, settings, search-provider behavior, extraction, source card, citation, or automatic
  retrieval behavior is added.

### Verification completed

Focused TDD first failed because the OpenAI loop accepted only a web-search executor. New socket-free coverage proves
successful fetch execution, exact provider call correlation, durable safe audit/result reopen, and the explicit
Ollama/OpenAI-versus-Anthropic fetch mapping boundary. Protocol coverage proves closed memory/search/fetch definition
order. All three host-local two-request OpenAI SSE fixtures pass, covering existing memory and search reuse plus the
new fetch path's streamed call accumulation, exact correlated `role: tool` reuse, explicit `untrusted: true`, final
answer streaming, and cumulative usage. The first sandboxed fetch-fixture run was blocked from binding its local
socket; the authorized host-local retry passed. After a later assertion-only rebuild stalled in Cargo's incremental
linker at zero CPU, the exact job was stopped and the non-incremental host-local retry passed all three fixtures.

The full standard checks pass: Prettier, Svelte diagnostics with zero errors or warnings, all 76 frontend tests, the
production build, Cargo formatting/check, and 282 Rust tests with 19 explicit opt-in tests skipped. The signed native
development app compiled and launched successfully. Immutable live-store inspection reported schema version 21,
`quick_check=ok`, 22 provider runs, and no retained `web_fetch` call; no migration was added. No browser presentation
review was repeated because this slice changes no frontend surface. No live remote OpenAI-compatible credential,
public-fetch interaction, or manual model-initiated `web_fetch` interaction was claimed.

## Prior completed product slice: Ollama web-fetch generation mapping

### Goal

Advertise and execute the closed native `web_fetch` contract through Ollama's existing durable Web loop without
changing the other provider mappings, WebView IPC, settings, storage schema, or page-source representation.

### Implemented shape

1. `OllamaToolSession` now appends the closed `web_fetch` definition immediately after `web_search` only when Web is
   explicitly enabled. Memory definitions retain their existing order, and Web-disabled requests advertise neither
   Web tool.
2. Native generation constructs the credential-free public-network fetch executor only for an explicitly Web-enabled,
   discovered tool-capable Ollama model. The existing selected search provider and credential remain required by the
   Web flow; no fetch credential, endpoint, setting, command, or WebView state was added.
3. The generation-only dispatcher accepts `web_fetch` only when that Ollama-scoped executor is present, then reuses
   the closed argument validator, safe execution policy, DNS/address-pinned `NativeWebFetch`, and common 64 KiB result
   envelope. OpenAI-compatible and Anthropic-compatible loops pass no fetch executor and continue advertising only
   `web_search`.
4. Ollama calls retain generated opaque identities, exact typed arguments, safe audit policy, terminal outcome,
   native-work duration, and the exact bounded result before provider reuse. The follow-up remains one ordered
   `role: tool` message under the existing eight-call, four-round, 256 KiB aggregate, five-minute, and cancellation
   limits.

### Acceptance criteria

- Web-disabled requests and models without explicit tool capability cannot advertise or execute `web_fetch`.
- Ollama receives the existing closed schema and exact untrusted result envelope; URLs and page source remain inside
  Rust/provider tool traffic and the durable audit rather than crossing a new Tauri command.
- OpenAI-compatible, Anthropic-compatible, and oMLX mapping remain absent. A provider-emitted unadvertised fetch call
  closes through the fixed redacted `unsupported_tool` result when no fetch executor is present.
- No migration, frontend state, settings, search-provider behavior, extraction, source card, citation, or automatic
  retrieval behavior is added.

### Verification completed

Focused TDD first failed because the Ollama fetch executor boundary did not exist. The new socket-free tests prove
successful execution, durable safe audit/result reopen, and fixed redacted rejection when the executor is absent.
Ollama protocol coverage proves the closed search/fetch definition order, while existing OpenAI-compatible and
Anthropic-compatible tests explicitly retain search-only Web definitions. Both host-local two-request Ollama fixtures
pass: one protects existing `web_search`; the other verifies `web_fetch` definition, streamed call accumulation,
durable execution, ordered `role: tool` reuse, explicit `untrusted: true`, final answer streaming, and cumulative usage.

The full standard checks pass: Prettier, Svelte diagnostics with zero errors or warnings, all 76 frontend tests, the
production build, Cargo formatting/check, and 280 Rust tests with 18 explicit opt-in tests skipped. Both exact live
Ollama streaming/cancellation tests and both exact live oMLX tests pass against the host-local providers. The broad
`live_` filter also selected the unrelated Brave and Exa credential probes; those two refused to run because the
explicit throwaway-key environment variables were absent, so no live search request or credential read was attempted.

The signed native development app compiled and launched successfully. Immutable read-only inspection of the live
store reported schema version 21, `quick_check=ok`, 22 provider runs, and no retained `web_fetch` call; no migration was
added. The production-policy IANA HTTPS smoke returned the stable `unavailable` category in the direct proxy-disabled
client even though host `curl` could reach that URL, so current environment egress did not provide fresh live-public
fetch evidence. Existing production-policy coverage, the socket-free durable tests, and both host-local two-request
Ollama fixtures pass. No browser presentation review was repeated because this slice changes no frontend surface. No
manual model-initiated `web_fetch` interaction was claimed.

## Prior completed product slice: Provider-independent native web fetch

### Goal

Define and execute one closed bounded `web_fetch` contract through Rust-owned public-network policy without exposing a
generic HTTP client, advertising it to a model, changing IPC, or weakening Bottie's common tool limits.

### Implemented shape

1. `web_fetch_tool_definition` publishes one closed object schema with exactly one required URL string capped at 4,096
   Unicode scalars. Validation accepts only absolute HTTP(S) public DNS names, strips fragments for execution, rejects
   embedded credentials, IP literals, special-use names, non-default ports, nulls, unknown fields, and other schemes,
   and never reflects rejected arguments.
2. `NativeWebFetch` performs credential-free GET requests without cookies, provider headers, model-supplied headers,
   ambient proxies, or automatic redirects. Each hostname is resolved once per hop; any empty or non-public DNS answer
   rejects the whole call, while accepted addresses are pinned into that hop's redirect-disabled client to close
   DNS-rebinding gaps.
3. Redirects are limited to HTTP 301, 302, 303, 307, and 308. Relative locations are resolved against the current URL,
   fragments are removed, the complete URL and DNS policy reruns before every hop, and at most three redirects share
   one 15-second operation deadline with a 3-second connection ceiling.
4. Only `text/html`, `application/xhtml+xml`, and `text/plain` with absent or UTF-8 charset are accepted. Declared and
   streamed response bytes are capped at 48 KiB, decoded strictly as UTF-8, and returned with the final normalized URL,
   media type, page source, and `untrusted: true` before the existing 64 KiB common-envelope check.
5. `web_fetch` has an explicit safe/read-only policy entry and one executable provider-neutral dispatcher path. Stable
   invalid-argument, unavailable, execution-failed, and output-too-large results do not include URLs, DNS answers,
   statuses, TLS/proxy detail, or response material.

### Acceptance criteria

- Private, loopback, link-local, carrier, documentation, transition, multicast, reserved, mapped, and special-use
  destinations cannot pass the production request path or a redirect.
- Every redirect, timeout, media type, charset, content-length, streamed byte, and common-envelope boundary is explicit
  and bounded; no Authorization, Cookie, Referer, provider credential, arbitrary request header, or body is accepted.
- Page source and URLs stay Rust/provider-tool-result-only; no filesystem path, credential, database value, page bytes,
  or new command crosses Tauri IPC.
- No schema migration, settings change, search-provider change, provider wire mapping, UI state, or automatic retrieval
  is added.
- HTML-to-inert-text extraction, title/publication metadata, dedicated web citation cards, and prompt-injection UX
  remain deferred.

### Verification completed

Focused TDD first failed because the definition, typed arguments, native fetch boundary, safe policy entry, and
dispatcher were absent. Contract, dispatcher, policy, URL, IP-range, redirect, timeout, content-type, charset, byte,
UTF-8, and redaction coverage now passes. Three loopback-only policy tests passed outside the restricted sandbox, and
one read-only production-policy HTTPS fetch of IANA's example-domains help page passed with DNS pinning, TLS,
media-type, size,
and timeout policy active. The full standard checks pass: Prettier, Svelte diagnostics with zero errors or warnings,
all 76 frontend tests, the production build, Cargo formatting/check, and 278 Rust tests with 17 explicit opt-in tests
ignored. `git diff --check` is clean. No browser or native-app review is required because this slice changes no UI,
IPC, startup, persistence, provider request shape, or advertised generation behavior.

## Prior completed product slice: Exa Search provider and search-engine settings

### Goal

Add Exa as a second fixed native search engine, let users retain independent Brave and Exa keys, and route the existing
closed `web_search` tool through one explicitly selected engine without exposing credentials, provider bodies, search
queries, or arbitrary endpoints to the WebView.

### Implemented shape

1. `ProviderSettings` now stores one validated `brave` or `exa` identity and defaults older settings files to Brave.
   Settings presents an explicit engine selector plus independent add, replace, retain, remove, and test controls for
   both credentials. Selecting one engine never deletes the other key.
2. The Exa adapter owns `POST https://api.exa.ai/search`, uses a sensitive `x-api-key` header, disables redirects,
   applies the existing 3-second connect and 15-second request timeouts, and rejects non-JSON or responses beyond 2 MiB.
3. Bottie's bounded query and result limits remain unchanged. Exa receives the normalized query, result count, native
   domain arrays, moderation, bounded highlight retrieval, and an absolute UTC `startPublishedDate` for day, week,
   month, or year freshness. Returned URLs are normalized and rechecked against Bottie's include/exclude policy;
   titles, highlights, and publication metadata are collapsed and truncated before entering the common envelope.
4. Connection testing accepts only the two compiled-in routes, uses a draft key or selected vault entry, and returns
   provider identity, elapsed time, and a fixed success message without results. Generation reads the saved engine
   choice, unlocks only that credential, and uses the unchanged safe dispatcher, durable audit, bounded loop,
   cancellation, and Ollama/OpenAI/Anthropic wire mappings.
5. Toolbar, activity, and context privacy labels now name the selected Brave Search or Exa Search hop. Web remains
   session-only, off by default, unavailable for oMLX, and gated by explicit discovered tool capability.

### Acceptance criteria

- Existing settings reopen on Brave without a migration, while only `brave` and `exa` can be persisted.
- Both credentials remain Rust/keyring-owned and cross IPC only as secret-free configured/unlocked status.
- Exa authentication never enters a URL or body; redirects, timeouts, response bytes, unsafe URLs, provider bodies,
  and normalized display metadata retain explicit bounds.
- Freshness and domain filters map to Exa's fixed request fields and are rechecked natively after normalization.
- Missing credentials and provider failures use stable redacted guidance for the selected engine.
- `web_fetch`, oMLX mapping, automatic retrieval, arbitrary search endpoints, and schema changes remain outside this
  slice.

### Verification completed

All standard checks pass: Prettier, Svelte diagnostics, 76 Vitest tests, production build, Cargo formatting/check, and
269 Rust tests with 13 explicit opt-in tests ignored. One user-authorized bounded live search passed through each native
adapter using throwaway process-only credentials; no document fetch ran and neither key was saved. The browser preview
was visually checked at 1440 x 900 and the 720 x 620 native minimum. The engine selector, independent Brave/Exa cards,
fixed host disclosures, internal scrolling, and absence of horizontal overflow were confirmed. Browser-preview controls
are intentionally read-only, so OS-vault mutation, Touch ID, and a persisted selection across a native restart remain
manually unverified.

## Prior completed product slice: Explicit Anthropic-compatible native web-search tool loop

### Goal

Map Bottie's closed `web_search` contract into Anthropic-compatible Messages and run explicitly enabled calls through
the existing Brave dispatcher, bounded native loop, durable tool records, provider stream, cumulative usage, and
shared cancellation path without adding oMLX mapping, web-source cards, `web_fetch`, or a migration.

### Implemented shape

1. The composer Web toggle remains session-only and off by default. It is now available for an Ollama, OpenAI-
   compatible, or Anthropic-compatible model that advertises tool capability. Native discovery rechecks that
   capability before resolving the Brave credential or advertising any web definition.
2. Anthropic-compatible requests receive exactly the enabled closed definitions. Web alone advertises one
   `web_search` client tool; Memory alone retains the three memory tools; enabling both preserves memory-definition
   order and appends Web. oMLX requests still cannot advertise Web.
3. Streamed Anthropic `tool_use` blocks retain their exact bounded provider identities and object inputs. Results
   return through correlated user `tool_result` blocks after the common safe dispatcher, four-round/eight-call/
   five-minute loop, 64 KiB per-result and 256 KiB aggregate-output limits, shared cancellation, and cumulative
   usage/cost handling. Native structured failures set Anthropic's `is_error` signal.
4. Each accepted call and its exact bounded success/error envelope commit to the existing append-only provider-run
   record before provider reuse. Credential, query, provider-body, path, embedding, and provider/native call identities
   remain outside WebView IPC and portable presentation.
5. The disabled Web-control explanation now uses one provider-neutral supported-model label. Existing local/cloud
   privacy surfaces continue to identify the selected inference route and additional Brave Search hop when Web is
   enabled.

### Acceptance criteria

- Web remains off by default and unavailable for oMLX or non-tool-capable models; missing Brave configuration fails
  with the fixed Settings prompt before a web definition can be sent.
- Memory and Web independently restrict the executable tool set. Anthropic Web-only requests advertise only the
  closed `web_search` schema, while combined requests retain the three memory tools first and Web last.
- Fragmented Anthropic calls preserve the exact `tool_use` identity and complete object input. The follow-up request
  carries the correlated assistant call plus inert bounded `tool_result` without credentials, paths, or provider
  bodies.
- Calls, results, execution policy, stable outcome, and native-work duration survive reopen through the existing tool
  audit before any result is reused by Anthropic.
- The shared loop, cancellation, output, deadline, and cumulative usage/cost policy remains unchanged for existing
  mapped memory- and web-tool routes.
- oMLX Web mapping, web-source cards, a second search provider, `web_fetch`, automatic retrieval, approval UI, and
  schema changes remain outside this slice.

### Verification completed

Focused TDD first failed on Anthropic Web availability, enabled-definition mapping, generation dispatch, and durable
result persistence. Pure, storage-backed, component, and provider fixture tests now cover provider/capability gating,
closed definition ordering, exact Anthropic call correlation, safe audit metadata, request-disabled memory behavior,
cumulative two-round completion, and provider-neutral disabled-control copy.

Prettier, `svelte-check`, all 76 frontend tests, the production build, Cargo formatting, and warning-free Cargo check
pass. The 274-test default Rust suite passes with 263 tests executed and 11 opt-in loopback/live-network tests skipped.
The new Anthropic Web loop also passes explicitly against a host-loopback two-request SSE fixture: the first request
advertises only `web_search`, the second returns the exact provider `tool_use` identity and bounded result, and final
answer plus cumulative usage complete normally.

The browser preview was inspected at 1,320 x 820 and 900 x 800. The provider-neutral disabled Web label and composer
controls remained contained, the responsive viewport had no horizontal overflow, and the console reported no warnings
or errors for Bottie. The signed native development build compiled, launched, remained active, and stopped on request.
Immutable read-only inspection reported schema version 21, `quick_check=ok`, 20 completed runs, two cancelled runs, one
retained `search_memory` call, and no retained `web_search` call; no migration was added. No live third-party
Anthropic-compatible account, Brave credential, or model-selected Web call was exercised, so that real
credential/provider UI flow remains unverified.

## Prior completed product slice: Explicit OpenAI-compatible native web-search tool loop

### Goal

Map Bottie's closed `web_search` contract into OpenAI-compatible Chat Completions and run explicitly enabled calls
through the existing Brave dispatcher, bounded native loop, durable tool records, provider stream, cumulative usage,
and shared cancellation path without adding Anthropic-compatible or oMLX mapping, web-source cards, `web_fetch`, or a
migration.

### Implemented shape

1. The composer Web toggle remains session-only and off by default. It is now available for an Ollama or OpenAI-
   compatible model that advertises tool capability. Native discovery rechecks that capability before resolving the
   Brave credential or advertising any web definition.
2. OpenAI-compatible requests receive exactly the enabled closed definitions. Web alone advertises one `web_search`
   function; Memory alone retains the three memory functions; enabling both preserves memory-definition order and
   appends Web. Anthropic-compatible and oMLX requests still cannot advertise Web.
3. Streamed OpenAI function calls retain their exact bounded provider identities and object arguments. Results return
   through correlated `role: tool` messages after the common safe dispatcher, four-round/eight-call/five-minute loop,
   64 KiB per-result and 256 KiB aggregate-output limits, shared cancellation, and cumulative usage/cost handling.
4. Each accepted call and its exact bounded success/error envelope commit to the existing append-only provider-run
   record before provider reuse. Credential, query, provider-body, path, embedding, and provider/native call identities
   remain outside WebView IPC and portable presentation.
5. The disabled Web-control explanation now names both mapped providers. The existing local/cloud privacy surfaces
   continue to identify the selected inference route and the additional Brave Search hop when Web is enabled.

### Acceptance criteria

- Web remains off by default and unavailable for non-Ollama/OpenAI-compatible or non-tool-capable models; missing Brave
  configuration fails with the fixed Settings prompt before a web definition can be sent.
- Memory and Web independently restrict the executable tool set. OpenAI Web-only requests advertise only the closed
  `web_search` schema, while combined requests retain the three memory tools first and Web last.
- Fragmented OpenAI calls preserve the exact provider call identity and complete object arguments. The follow-up
  request carries the correlated assistant call plus inert bounded result without credentials, paths, or provider
  bodies.
- Calls, results, execution policy, stable outcome, and native-work duration survive reopen through the existing tool
  audit before any result is reused by OpenAI.
- The shared loop, cancellation, output, deadline, and cumulative usage/cost policy remains unchanged for existing
  Ollama and OpenAI memory-tool routes.
- Anthropic-compatible and oMLX Web mapping, web-source cards, a second search provider, `web_fetch`, automatic
  retrieval, approval UI, and schema changes remain outside this slice.

### Verification completed

Focused TDD first failed on OpenAI Web availability, request-specific definition mapping, generation dispatch, durable
result persistence, provider-specific disabled-control explanation, and explicit cloud-plus-Brave privacy copy. Pure,
storage-backed, component, and fixture tests now cover provider/capability gating, closed definition ordering, exact
OpenAI call correlation, safe audit metadata, request-disabled memory behavior, cumulative two-round completion, and
both local and cloud Web-route explanations.

Prettier, `svelte-check`, all 76 frontend tests, the production build, Cargo formatting, and Cargo check pass. The
270-test default Rust suite passes with 260 tests executed and ten opt-in loopback/live-network tests skipped. Both the
existing OpenAI memory loop and new OpenAI Web loop pass explicitly against host-loopback two-request SSE fixtures; the
Web fixture confirms the first request advertises only `web_search`, the second returns the exact provider call identity
and bounded result, and final answer plus cumulative usage complete normally.

The browser preview was inspected at its default desktop viewport and at 900 x 800. The updated disabled Web label and
composer controls remained contained, the context overlay remained usable, and the console reported no warnings or
errors. The native development build compiled, signed, launched one 1,321 x 821 Bottie window, remained active, and
stopped on request. Immutable read-only inspection reported schema version 21, `quick_check=ok`, 20 completed runs, two
cancelled runs, one retained `search_memory` call, and no retained `web_search` call; no migration was added. No live
third-party OpenAI-compatible account, Brave credential, or model-selected Web call was exercised, so that real
credential/provider UI flow remains unverified.

## Prior completed product slice: Explicit Ollama native web-search tool loop

### Goal

Map Bottie's closed `web_search` contract into Ollama's function wire format and run explicitly enabled calls through
the existing Brave dispatcher, bounded native loop, durable tool records, provider stream, and shared cancellation
path without adding OpenAI-compatible or Anthropic-compatible mapping, web-source cards, `web_fetch`, or a migration.

### Implemented shape

1. The composer Web toggle is session-only, off by default, and available only for an Ollama model that advertises
   tool capability. The typed request defaults `webEnabled` off for older callers, native discovery rechecks the route,
   and the enabled request resolves the Brave key only through the operating-system credential vault.
2. Ollama receives exactly the explicitly enabled closed definitions: Web alone advertises one `web_search` function;
   enabling Memory as well retains the three memory definitions first and appends Web. OpenAI-compatible, Anthropic-
   compatible, and oMLX requests cannot advertise Web.
3. Ollama's ordered calls reuse the existing opaque native identities, four-round/eight-call/five-minute state machine,
   64 KiB per-result and 256 KiB aggregate-output limits, cumulative usage, HTTP abort handle, and before/after native
   cancellation checks.
4. Each accepted `web_search` invocation commits before the existing strict provider-independent dispatcher runs. Its
   exact bounded success/error envelope, safe execution policy, stable outcome, and native-work duration commit before
   the inert result is returned to Ollama. Credential, query, provider-body, path, and call identities stay native.
5. The privacy presentation no longer says `Local only` while Web is enabled. It shows `Local + web` and explains that
   the model prompt stays on Ollama's loopback route while model-selected bounded search queries go to Brave Search.

### Acceptance criteria

- Web remains off by default and unavailable for non-Ollama or non-tool-capable models; missing Brave configuration
  fails with a fixed Settings prompt before a web definition can be sent.
- Memory and Web independently restrict the executable tool set; an Ollama call for a registered but request-disabled
  tool returns the fixed unsupported envelope without executing storage, embedding, credential, or network work.
- The first Web-only Ollama request contains exactly the closed `web_search` definition, and a follow-up contains the
  ordered assistant call plus tool-name/result message without native IDs, credentials, paths, or provider bodies.
- Calls and exact common results survive reopen through the existing append-only provider-run records before any
  result is reused by Ollama.
- Multiple rounds retain the shared loop, output, deadline, usage, and cancellation policy without changing memory-tool
  behavior for Ollama, OpenAI-compatible, or Anthropic-compatible routes.
- OpenAI-compatible, Anthropic-compatible, and oMLX web mapping, web-source cards, a second search provider,
  `web_fetch`, automatic retrieval, approval UI, and schema changes remain outside this slice.

### Verification completed

Focused TDD first failed on the absent Web request flag, Ollama-only availability helper, enabled composer control,
closed definition mapping, native search executor, and durable Web round. Pure and storage-backed tests now cover
off-by-default request compatibility, provider/capability gating, closed definition ordering, missing-credential
failure, exact success persistence, safe audit metadata, privacy copy, and unchanged unavailable states. A host-loopback
two-request Ollama fixture confirms the first request carries only `web_search`, the second carries the exact bounded
result, and the final answer plus cumulative usage complete normally.

Prettier, `svelte-check`, all 75 frontend tests, the production build, Cargo formatting, and Cargo check pass. The
266-test default Rust suite passes with nine opt-in loopback/live-network tests skipped. The focused host-loopback
Ollama Web fixture also passes explicitly. The browser preview was inspected at its default desktop viewport and at
900 x 800: the unavailable Web control stayed labelled and contained, the responsive context overlay remained usable,
and the console reported no warnings or errors. The native development command built, signed, and launched one
1,321 x 821 Bottie window and remained live until stopped cleanly. macOS accessibility exposed only the native window
chrome rather than WebView controls, so no synthetic click or live Brave/model call is claimed. Immutable read-only
inspection reported schema version 21, `quick_check=ok`, 20 completed runs, two cancelled runs, and no retained
`web_search` row in the existing live store; no migration was added by this slice.

## Prior completed product slice: Provider-independent web-search tool contract and dispatcher

### Goal

Define and execute one closed, bounded `web_search` contract with useful freshness and domain controls while keeping
model-provider wire formats, generation, credentials, and the WebView outside this slice.

### Implemented shape

1. `web_search_tool_definition` publishes one closed object schema with a required query, optional
   `day`/`week`/`month`/`year` freshness, up to five combined include/exclude domains, and a one-to-ten result ceiling.
   It remains separate from the three provider-advertised memory definitions.
2. Raw calls reject non-objects, unknown or missing fields, null/type mismatches, unsupported freshness, empty or
   oversized lists, duplicate/conflicting filters, URLs, wildcards, IP addresses, single-label hosts, and invalid DNS
   labels before provider work.
3. `WebSearchRequest` normalizes domain case, maps multiple includes through explicit `site:`/`OR` operators and
   exclusions through `NOT site:`, and rechecks the complete generated query against Brave's 400-character/50-term
   ceiling. The Brave adapter maps freshness to its fixed `pd`/`pw`/`pm`/`py` values.
4. The Brave response path rechecks every normalized URL host against the native include/exclude policy. Exact domains
   and subdomains are accepted; blocked or outside-allowlist results are removed even if provider operator behavior is
   incomplete.
5. `dispatch_web_search_tool` applies the same explicit safe/read-only policy gate used by memory tools, executes only
   typed requests through the pluggable provider trait, enforces the existing 64 KiB result envelope, and maps invalid,
   unavailable, credential, rate-limit, timeout, malformed, and internal failures without reflecting the query,
   domains, provider body, credential, or implementation detail.

### Acceptance criteria

- The native definition is closed, provider-independent, and not mixed into the three definitions currently
  advertised by Ollama, OpenAI-compatible, or Anthropic-compatible generation.
- Freshness and domain filters are strictly typed, bounded before I/O, mapped to the fixed Brave route, and reapplied
  to normalized output hosts.
- Invalid calls cannot reach the provider; provider failures return only stable common dispatcher categories.
- `web_search` has an explicit safe policy entry and one executable native dispatcher path, but no model, generation,
  audit, conversation, IPC, or UI path can invoke it yet.
- No schema migration, second search provider, generic HTTP capability, or `web_fetch` behavior is added.

### Verification completed

Focused TDD first failed because the web definition, typed arguments, filter-aware request, safe policy entry, and
dispatcher were absent. New contract tests cover the closed schema, exact typed conversion, unknown/null/type errors,
freshness, domain, conflict, and result limits. Provider tests cover Brave query/freshness mapping, complete-query
bounds, strict DNS filtering, and defense-in-depth output-host enforcement. Dispatcher tests prove invalid calls stop
before provider work, successful normalized results use the common bounded envelope, and provider detail is redacted.

Cargo formatting and the default Rust suite pass with 252 tests and nine opt-in network tests skipped. The standard
frontend checks and build also pass unchanged. The two isolated Brave loopback tests pass with host-local socket
access, covering the existing fixed header-authenticated request/response path. No browser or native presentation
review was required because this slice changes no IPC, startup, storage, or frontend behavior. No live Brave credential
was read and no live Brave API request was made; provider mapping and generation remain intentionally absent.

## Prior completed product slice: Brave Search credential configuration and connection testing

### Goal

Let users securely configure and verify the fixed Brave Search route without exposing credentials, provider results,
or arbitrary native networking to the WebView and without prematurely registering a model-visible web tool.

### Implemented shape

1. The existing operating-system credential store now accepts the stable `brave` provider identity while continuing to
   return only configured, process-unlocked, and biometric-policy state to the WebView. The key is never serialized or
   persisted in provider settings.
2. Settings adds one Brave Search cloud section with password-style draft entry, explicit saved-key removal, fixed
   endpoint disclosure, session unlock state, and a connection Test action. Browser preview keeps all mutation and test
   controls disabled.
3. `test_web_search_connection` accepts only the `brave` identity, prefers an unsaved trimmed draft key for that one
   test, otherwise reads the saved vault entry under the existing session biometric policy, and constructs only the
   production fixed-endpoint adapter.
4. The connection test sends the constant `Bottie connection test` query with a one-result ceiling, runs through the
   adapter's existing no-redirect, timeout, response-size, URL, and redaction policy, then discards normalized results.
   IPC receives only provider identity, elapsed milliseconds, and a fixed success message.
5. Success/failure diagnostics remain secret-redacted. This slice adds no persisted endpoint, model-visible tool,
   dispatcher registration, provider wire mapping, database migration, second provider, or generic HTTP capability.

### Acceptance criteria

- A Brave key can be added, replaced, retained, or removed through the existing OS-vault boundary without being read
  back into JavaScript.
- Draft keys can be tested without saving; an empty draft falls back to the saved key and a missing key fails before
  provider I/O.
- Connection testing can reach only the fixed HTTPS Brave endpoint and returns no query, credential, provider body, or
  search result to the WebView.
- Credential rejection, rate limiting, timeout, unavailability, and malformed responses retain fixed redacted errors
  and secret-redacted diagnostic records.
- No search reaches a model, provider tool request, dispatcher, durable audit row, conversation, or database.

### Verification completed

Focused TDD first failed because the `brave` vault identity, fixed connection-probe request, and Settings presentation
were absent. Focused Rust coverage verifies native credential allowlisting, the constant one-result probe, and redacted
adapter-error mapping. One server-rendered frontend test verifies the fixed-route disclosure and absence of a
model-tool claim.

The standard frontend format check, `svelte-check`, all 75 frontend tests, and the production build pass. Cargo
formatting, Cargo check, and the default Rust suite pass with 243 tests and nine opt-in network tests skipped. The two
isolated Brave loopback tests also pass with host-local socket access, covering the complete header-authenticated
request/JSON path and stable rate-limit redaction.

The browser preview was visually checked at the desktop viewport and the 720 x 620 native minimum. The Brave card,
fixed endpoint, vault-only disclosure, disabled browser controls, and connection-test explanation remain contained
without horizontal overflow or console warnings. The native app compiled, development-signed, and launched against the
existing store with the new command registered. No actual credential was entered, saved, removed, unlocked, or sent;
therefore the real OS-vault mutation, Touch ID prompt, and live Brave endpoint remain manually unverified. The fixed
wire path is covered by the isolated loopback tests.

## Prior completed product slice: Pluggable native web-search provider boundary

The preceding slice established the provider-neutral query/result/error contract and fixed-endpoint Brave adapter.
Six socket-free protocol tests plus two opt-in loopback tests cover request bounds, header-only authentication,
no-redirect fixed routing, safe result normalization, response ceilings, and redacted provider failures.

## Prior completed product slice: Structured tool audit and expandable activity

### Goal

Turn provider tool calls into calm, durable, provider-neutral audit records that explain what Bottie allowed and what
happened without exposing opaque call identities or forcing users to scan raw JSON.

### Implemented shape

1. Schema version 21 adds one immutable execution-policy classification to each existing tool invocation plus a stable
   terminal outcome and native execution duration to its append-only result. New writes enforce agreement between the
   result error flag and outcome; historical success/error rows migrate as honest `legacy` audit records with no
   invented duration.
2. Mapped Ollama, OpenAI-compatible, and Anthropic-compatible execution snapshots the native policy classification
   before dispatch and maps only the bounded provider-neutral result categories after dispatch. Unregistered provider
   names are retained as unregistered/rejected audit state while the result remains redacted.
3. Reopened and just-completed selected responses now show a compact Tool activity summary. Each call expands into its
   requested/finished times, policy, outcome, and native-work duration; argument and result JSON remain inert text
   behind separate nested disclosures.
4. Known memory tools receive calm product labels while the exact registered name remains visible. Pure presentation
   helpers own policy/outcome/duration labels, and a focused `ToolActivity.svelte` component keeps the conversation
   renderer below its practical size limit.
5. Selected and batch JSON exports advance to version 4 and retain the structured audit object without native run or
   provider call IDs. Markdown exports add the same policy, outcome, and optional duration ahead of existing inert
   argument/result payloads.

### Acceptance criteria

- Existing schema-20 stores migrate without rewriting tool arguments/results; legacy records never claim a policy or
  duration that Bottie did not record at execution time.
- Every new accepted call records `safe`, `approval_required`, or `unregistered` before dispatch and one stable
  success/rejection/failure category plus native duration when a result is appended.
- Unknown and malformed provider calls remain bounded and redacted while their audit state explains the rejection;
  result status and audit outcome cannot disagree at the storage boundary.
- The activity summary identifies calls needing attention without opening raw payloads, and each call is independently
  keyboard-expandable with structured audit fields before arguments/results.
- IPC and portable exports still omit provider call IDs, native run IDs, paths, hashes, embeddings, scores, and storage
  details. No web tool, provider wire change, approval UI, or approval-required execution is added.

### Verification completed

Focused TDD first failed on the absent audit types/schema and absent presentation helper. Five storage tests cover
schema-20 legacy backfill, ordered pending/success/error reconstruction, mismatch rejection, linkage, duplicates, and
terminal-run constraints. Mapped-generation coverage confirms safe success metadata and unregistered-call rejection
without reflecting provider-controlled names or path-shaped arguments. Export tests cover version-4 JSON plus Markdown
audit summaries without opaque identities. Five frontend tests cover tool naming, policy/outcome/status/duration
presentation, attention counts, and server-rendered disclosure structure.

Prettier, `svelte-check`, all 74 frontend tests, the production build, Cargo formatting, Cargo check, and all 235
default Rust tests pass; seven opt-in network tests remain ignored by the default suite. The first full Rust pass
exposed older-version migration fixtures that retained schema-21 columns while rewinding `user_version`; those fixtures
now remove only the audit columns before exercising their historical migration paths, and the clean full rerun passed.

The browser preview was visually checked at 1320 x 820 and the 720 x 620 native minimum with both the activity group
and its call record expanded. The audit grid remained inside its container at both sizes; an explicit responsive DOM
check found document width equal to the 720-pixel viewport and no activity element with horizontal overflow. The
browser console contained no warnings or errors. The disclosures use native `details`/`summary` semantics and the
server-rendered accessibility structure is covered, but automated keyboard presses did not change disclosure state in
the browser harness, so this run does not claim a separate keyboard-interaction confirmation.

The native app compiled and launched against the existing store without terminal errors. Immutable read-only
inspection after launch returned schema version 21, `quick_check=ok`, nine conversations, one tool invocation, and one
matching result. That historical call migrated with `execution_policy=legacy`, as required; no real provider tool call
was generated during this verification, so fresh non-legacy native UI presentation remains covered by mapped-provider
path tests and the browser fixture rather than a live model response.

## Prior completed product slice: Safe versus approval-required native tool policy

### Goal

Establish one provider-independent execution policy before Bottie adds tools that can affect external or sensitive
state, while preserving the existing explicitly enabled read-only memory-tool behavior.

### Implemented shape

1. `src-tauri/src/tool_policy.rs` owns the only native classification table. Every currently advertised tool must map
   explicitly to `Safe` or `ApprovalRequired`; unknown names fail closed before argument validation, storage access,
   embedding work, or any other tool side effect.
2. `search_memory`, `open_memory`, and `search_attached_files` are explicitly safe because they are bounded read-only
   retrieval, are advertised only after the user enables Memory for a tool-capable request, and already recheck native
   profile, lifecycle, association, and exclusion policy. This does not make arbitrary future read operations safe.
3. The common dispatcher now accepts the complete provider-neutral call plus an optional Rust-owned approval grant and
   requires policy authorization before the existing closed-schema validation and typed execution boundary. All
   Ollama, OpenAI-compatible, and Anthropic-compatible generation paths continue through that same dispatcher.
4. A non-cloneable, consumed approval grant captures the exact provider call ID, tool name, and structured arguments.
   Supplying a grant after any of those provider-controlled values changes returns the same fixed `approval_required`
   error as a missing grant; neither the name nor arguments are reflected in the error.
5. Current provider orchestration supplies no approval grant. That is sufficient for the three safe memory tools and
   deliberately prevents a future approval-required tool from running until a trusted native approval flow passes the
   exact grant. No WebView boolean, provider argument, persisted preference, or implicit approval path was added.
6. Policy denials use the existing bounded provider-neutral error envelope and append-only invocation/result
   checkpoint path. This slice adds no schema, IPC, settings, provider wire shape, tool UI, web access, or side effect.

### Acceptance criteria

- Every advertised native tool has an explicit execution classification; an unregistered name is unsupported rather
  than inheriting a permissive default.
- The three current memory tools execute without a per-call prompt only inside the existing explicit Memory-enabled
  request path, and their validation, result limits, cancellation, and durable checkpoint behavior remain unchanged.
- Approval-required policy never executes without a matching native grant, and a changed call ID, name, or argument
  payload invalidates an earlier grant.
- Policy failures are bounded, redacted, machine-readable `unsupported_tool` or `approval_required` envelopes and do
  not expose provider-controlled input or native details.
- No approval surface, approval-required tool, audit-schema/UI expansion, oMLX mapping, automatic retrieval injection,
  document opening, web tool, model-cache deletion, or attachment retry behavior is added.

### Verification completed

Focused TDD first failed because the policy module and mandatory dispatch boundary were absent. Three policy tests
cover the complete current safe-tool registry, fail-closed unknown tools, missing grants, successful exact grants, and
grant rejection after call-ID, name, or argument changes. Existing dispatcher and mapped-generation tests confirm
that all three safe memory tools still run through bounded envelopes and that durable Ollama checkpoints are unchanged.

Prettier, `svelte-check`, all 69 frontend tests, the production build, Cargo formatting, Cargo check, and all 233
default Rust tests pass; seven opt-in network tests remain ignored by the default suite. The three local Ollama,
OpenAI-compatible, and Anthropic-compatible multi-round fixture tests initially could not bind inside the sandbox, then
all passed with host-local loopback access. The remaining four ignored tests require live oMLX or Ollama services and
were not required because this slice does not change provider networking or wire formats.

No browser review was required because the slice adds no frontend state or presentation. The native app was not
launched because there is no migration, IPC, startup, or UI change; avoiding a launch also avoids applying an unrelated
saved Trash-retention policy to the user's live store. Immutable inspection returned schema version 19,
`quick_check=ok`, nine conversations, one tool invocation, and one matching tool result. This slice changed none of
those values and requires no schema bump.

## Prior completed product slice: Opt-in time-based Trash retention

### Goal

Add one durable, explicit policy for forgetting conversations after time in Trash without applying age-based deletion
to active or Archived conversations or changing the established attachment, backup, export, and model-cache boundary.

### Implemented shape

1. Schema version 20 adds one optional `conversation_retention_policies` row for the built-in local profile. No row
   means keep Trash until manual forget; the only stored alternatives are 30 days, 90 days, or one year.
2. Settings exposes those four exact choices through a separate native control. Saving a period changes policy only;
   it does not delete content in the current process. Copy states that already-old Trash may expire on the next healthy
   app launch and that external exports/backups plus the application-owned embedding-model cache remain unchanged.
3. Healthy startup applies the saved period after migration and interrupted-run recovery but before attachment garbage
   collection. The inclusive cutoff uses `deleted_at_ms`, so restore and a later move back to Trash restart the clock.
4. One immediate transaction deletes only built-in-profile conversations already in Trash at or before the cutoff.
   Active and Archived conversations are ineligible, and the query defensively skips any conversation that still owns
   a running provider response.
5. Expired deletion uses the same foreign-key and derived-memory cascades as explicit forget. Shared attachments remain;
   newly unreferenced files follow the existing 24-hour safety window and strict startup garbage collector. Restore
   staging uses migration-only initialization, so validating a backup cannot trigger its saved retention policy.
6. IPC accepts and returns only the bounded period enum. No deletion time, source text, path, hash, score, embedding,
   chunk/vector identity, database detail, backup path, or model-cache detail is added to the WebView contract.

### Acceptance criteria

- Manual retention is the migration and product default, survives restart, and can be restored after any timed period.
- Thirty-day, ninety-day, and one-year policies use the conversation's durable Trash timestamp and an inclusive cutoff.
- A healthy app startup forgets only expired Trash; active, Archived, recent Trash, recovery-restricted stores, and
  defensive active-run rows are not deleted by the policy.
- The Settings copy distinguishes a saved future-startup policy from immediate deletion and names already-old Trash,
  the attachment safety window, unchanged external copies, and the retained model cache.
- Automatic forget removes the same live source/derived data as manual forget and does not add provider behavior,
  automatic retrieval injection, document opening, web tools, model-cache deletion, or attachment retry controls.

### Verification completed

Focused TDD first failed on the absent Rust policy/types and frontend modules. Three path-backed Rust tests cover the
manual default, every supported persisted period, disabling the policy again, exact inclusive cutoff behavior,
active/Archived/recent-Trash preservation, and healthy-startup enforcement. Frontend module and server-rendered
component tests cover the closed option set, native-only disabled preview, and destructive/external-copy disclosure.

Prettier, `svelte-check`, all 69 frontend tests, the production build, Cargo formatting, Cargo check, and all 230
default Rust tests pass; seven opt-in live provider/loopback tests remain ignored because provider behavior did not
change. The first full Rust pass exposed old-version fixtures that still retained the new table while rewinding
`user_version`; those fixtures now remove schema-20 state and assert the current version before the final clean run.

An interactive browser preview was attempted, but the sandbox denied the local port bind and the follow-up host-local
approval could not be granted, so this slice makes no desktop or responsive visual-interaction claim beyond clean
server-rendered component coverage, `svelte-check`, and the production build. The native app was not launched against
the real store. Immutable read-only inspection found the real store still at schema 19 with `quick_check=ok`, nine
conversations including one Trash item, no memory-exclusion rows, and semantic progress `ready` at 86 of 86 chunks.
No live policy was saved and no real conversation was deleted; migration, persistence, and destructive startup behavior
are verified against isolated path-backed stores.

## Prior completed product slice: Explicit per-conversation forget

### Goal

Add one explicit irreversible workflow that removes a trashed conversation's live source and derived memory while
stating exactly which deduplicated attachment bytes and external copies are retained.

### Implemented shape

1. Trash exposes `Forget permanently` separately from `Restore`. A second inline confirmation names message-memory and
   file-link deletion, the 24-hour unshared-file safety window, and unchanged exports/backups before `Forget forever`
   can invoke native storage.
2. The narrow `forget_conversation` command accepts only one opaque conversation ID. Rust requires the built-in local
   profile, an existing trashed lifecycle state, and no running provider response; active/Archived/missing/active-run
   targets fail with stable path- and database-redacted errors.
3. One immediate transaction deletes the conversation row. Existing foreign-key ownership and cleanup triggers remove
   branches, messages and reasoning, provider runs/usage, tool calls/results, ratings, attachment associations, the
   memory preference, lexical message rows, deterministic message chunks, and cascaded vector mappings. No migration
   or WebView source/derived identity was added.
4. Content-addressed attachments are global rather than conversation-owned. Shared files and derived rows remain
   available through their other references. Files made unreferenced by forgetting retain the established 24-hour
   cross-process safety window; startup garbage collection then removes their catalog, extraction/normalization rows,
   attachment chunks/vectors, original bytes, and derivatives.
5. Forget changes the live store only. Existing Markdown/JSON/ZIP exports, manual backups, pre-restore safety copies,
   automatic recovery snapshots, and copies outside Bottie's managed data remain unchanged and must be removed or
   rotated separately. The shared embedding-model cache is application runtime data and is not deleted.

### Acceptance criteria

- Only a trashed, locally owned conversation without an active response can be permanently forgotten.
- After success the conversation is absent from navigation, cannot load or restore after restart, and contributes no
  message source, lexical row, deterministic chunk, vector mapping, provider/tool audit, rating, or attachment link.
- Shared attachment content survives. Newly unreferenced content follows the already documented 24-hour safety window
  and established strict managed-file garbage collection rather than unsafe in-command filesystem deletion.
- The confirmation and documentation distinguish live-store deletion from retained external exports/backups and the
  non-conversation embedding-model cache.
- IPC remains an opaque conversation ID with an empty success response; source text, paths, hashes, scores, embedding,
  chunk/vector identities, backup paths, and model-cache details do not cross into the WebView.
- No time-based retention, oMLX mapping, automatic retrieval injection, model-cache deletion, document opening, web
  tool, or attachment retry behavior is added.

### Verification completed

Focused TDD first failed on the absent native mutation and frontend policy module. Two new path-backed Rust tests cover
active/Archived/missing rejection, active-run exclusion, permanent deletion after Trash, provider-run cascade, restart
absence, lexical and chunk removal, immediate association removal, the attachment safety window, shared-file
preservation, and strict later orphan collection. Frontend coverage checks the exact irreversible action and disclosure
copy.

Prettier, `svelte-check`, all 66 frontend tests, the production build, Cargo formatting, Cargo check, and all 227
default Rust tests pass; seven opt-in live provider/loopback tests remain ignored by default because provider protocol
behavior did not change. The browser-only Trash fixture was inspected and then removed at 1320 x 820 and 900 x 800.
The first pass exposed fixed-menu overflow and the compact pass exposed a left-edge clip; both were corrected, and the
final confirmation has complete visible copy/actions with no console warnings or errors at either viewport. The native
app launched successfully against the real store. Immutable SQLite inspection reported schema 19, `quick_check=ok`,
nine conversations including one Trash item, no memory-preference rows, and semantic progress `ready` at 86 of 86
chunks. No live conversation was forgotten: the destructive mutation is verified only against isolated persistent
stores, so no claim is made that the real Trash item was manually changed.

## Prior completed product slice: Durable per-conversation memory exclusion

### Goal

Add one reversible active/Archived conversation control that prevents native long-term memory retrieval without
deleting the conversation, attachments, exports, selected branch, provider records, tool audit, or model cache.

### Implemented shape

1. Schema version 19 adds `conversation_memory_preferences`, keyed by the native conversation identity. Navigation
   summaries serialize only the path-free boolean `memoryExcluded`; the new narrow command rejects missing, foreign,
   trashed, or actively generating targets and never accepts SQL, paths, source text, or derived identities from the
   WebView.
2. `set_conversation_memory_excluded` updates the preference and refreshes every message source in one immediate
   transaction. Excluding removes the conversation's deterministic message chunks and cascaded vector mappings;
   re-including rebuilds the same versioned chunks and wakes the existing bounded semantic worker.
3. Lexical and semantic retrieval exclude message sources owned by an excluded conversation. Document retrieval
   ignores its conversation- and message-scoped associations; a content-deduplicated file remains eligible when at
   least one other active or Archived non-excluded conversation still references it.
4. `search_memory` and `search_attached_files` recheck provenance after hybrid ranking, and `open_memory` refuses stale
   exact provenance after its conversation becomes excluded. This defense remains native even if indexes or tool
   calls were created before the preference changed.
5. Active and Archived conversation menus expose `Exclude from memory` or `Include in memory`. Excluded summaries show
   a compact `Memory off` label in navigation. Trash has no control; delete/restore preserves an existing preference
   because neither action claims to forget retained content.

### Acceptance criteria

- The preference survives restart and is reversible without source-content loss or branch/lifecycle changes.
- Excluded message text produces no lexical, semantic, hybrid, `search_memory`, or `open_memory` result; its chunks and
  vectors are absent until re-inclusion rebuilds them.
- Documents associated only through the excluded conversation produce no lexical, semantic, hybrid, or
  `search_attached_files` result. A shared document remains eligible only through a separate non-excluded association.
- The WebView receives only conversation summaries and the requested boolean; no source text, path, hash, score,
  embedding, chunk/vector identity, database detail, or model-cache detail is added to IPC.
- No source deletion, time-based retention, provider mapping, automatic retrieval injection, model-cache deletion,
  document opening, web tool, or attachment retry behavior is added.

### Verification completed

Focused TDD first failed on the absent native mutation/summary field and missing conversation-menu presentation. New
path-backed Rust tests cover durable exclude/reopen/re-include behavior, source preservation, message-chunk removal and
rebuild, lexical and semantic suppression, `search_memory`, stale `open_memory`, conversation-scoped file suppression,
and shared-file eligibility through a second conversation. Server-rendered frontend coverage checks the compact state
label and exact reversible action copy.

Prettier, `svelte-check`, all 65 frontend tests, the production build, Cargo formatting, Cargo check, and all 225
default Rust tests pass; seven opt-in live provider/loopback tests remain ignored by default because provider protocol
behavior did not change. The browser preview was inspected at 1320 x 820 and 900 x 800 with no console warnings or
errors. The native app launched successfully against the real store; immutable SQLite inspection reported schema 19,
`quick_check=ok`, an empty preference table before user action, and semantic progress `ready` at 86 of 86 chunks.
macOS coordinate automation was not reliable enough to toggle a live conversation safely, so no manual live-data
mutation is claimed; the reversible native path is covered by isolated persistent-store tests.

## Prior completed product slice: Visible removable native memory provenance

### Goal

Replace the Context panel's scored memory fixtures with visible, removable, path-free provenance derived only from
successful native memory-tool activity, without changing SQLite, IPC, provider requests, append-only tool audit data,
or source-retention policy.

### Implemented shape

1. `memoryCitationsForMessages` reads only selected-lineage assistant tool records already reconstructed by the native
   conversation load. It accepts exact successful dispatcher envelopes for `search_memory`, `open_memory`, and
   `search_attached_files`; failed, pending, unsupported, or malformed activity produces no card.
2. Conversation results show the bounded excerpt, durable conversation title, and creation date. Retained-document
   results show the bounded excerpt, native-sanitized display name, and creation date. Raw tool arguments, opaque
   identities, ranks, scores, hashes, paths, embeddings, full documents, and unknown result fields are never rendered.
3. Repeated search/open results deduplicate by native message or attachment provenance. The newest selected-lineage
   assistant activity is considered first while provider order remains stable within each response.
4. Each card has an accessible remove control and the empty state explains that successful native memory tools add
   citations. Removal is deliberately session-local and presentation-only: it cannot rewrite append-only provider-run
   tool records or imply that the source has been forgotten.
5. A focused `MemoryContextState` owns the existing explicit Memory toggle plus dismissed citation identities, keeping
   the central page controller at its 500-line practical limit. The browser-preview tool fixture now uses the exact
   common success envelope for layout review.

### Acceptance criteria

- Only non-error results with `{ ok: true, result: ... }` from the three supported native memory tools produce cards.
- Conversation and retained-file cards expose useful excerpt/title/date attribution without paths, scores, raw
  arguments, hashes, embeddings, full source text, or opaque identities in rendered markup.
- Duplicate provenance appears once and removal immediately replaces the last card with an explicit empty state.
- Selected branch changes and conversation reopen naturally rebuild cards from that visible lineage's durable tool
  records; session-local dismissals do not mutate storage or claim durable forget behavior.
- No migration, Tauri command, provider mapping, automatic injection, oMLX tool support, source deletion, retention
  policy, document opening, web tool, model-cache deletion, or attachment retry is added.

### Verification completed

Focused TDD first failed on the absent provenance adapter and the three scored Context-panel fixtures. New pure tests
cover exact conversation and attached-file results, `open_memory` matched-turn fallback, source deduplication,
dismissals, and rejection of failed, unsupported, or malformed activity. Server-rendered component coverage confirms
the accessible remove label, empty fixture-score removal, and absence of opaque identities from Context-panel markup.
Prettier, `svelte-check`, all 64 frontend tests, the production build, Cargo formatting, Cargo check, and all 222
default Rust tests pass; seven opt-in provider/loopback tests remain ignored by default because provider and native
protocol behavior did not change.

The browser preview was inspected at the default desktop viewport and at 900 x 800. The citation-card layout remained
contained in both the fixed panel and responsive overlay, its remove control changed `Memories 1` to the explicit
empty state, and the browser console reported no errors. No schema, IPC, Rust, provider, credential, filesystem, or
native-window behavior changed, so this slice did not claim a fresh provider call or native persistence interaction.
Citation dismissals reset on frontend reload by design; conversation exclusion is now durable, while explicit forget
and time-based retention remain future work.

## Prior completed product slice: Explicit Anthropic Messages native memory-tool loop

### Goal

Map Bottie's three closed native memory tools into Anthropic-compatible Messages and run repeated explicitly enabled
calls through the existing bounded dispatcher, state machine, durable tool records, provider stream, and shared
cancellation path, without adding oMLX mapping, automatic retrieval injection, citation cards, context-panel
replacement, or a schema migration.

### Implemented shape

1. The session-only Memory toggle is now available for Ollama, OpenAI-compatible, or Anthropic-compatible models that
   explicitly advertise tool capability. Native generation rechecks the selected model through provider discovery
   before definitions are sent; the flag remains off by default and clears for an unmapped or non-tool-capable model.
2. Anthropic request mapping preserves the exact three native names, descriptions, and closed `input_schema` objects.
   Streamed `tool_use` blocks retain provider order and reconstruct bounded object arguments from indexed
   `input_json_delta` fragments only when each content block closes.
3. Follow-up requests append the exact ordered assistant content blocks, including unmodified `thinking` signatures
   and opaque `redacted_thinking` data, then an immediately following `role: user` message containing correlated
   `tool_result` blocks. Native structured failures additionally set Anthropic's `is_error` signal.
4. Anthropic's provider call identity becomes the provider-neutral loop identity and durable correlation key. It
   remains Rust/SQLite-only and is returned unchanged only on the provider wire; reopened UI state and exports continue
   to omit native and provider call identities.
5. Every accepted invocation and exact bounded success/error envelope commits before provider reuse. Anthropic rounds
   use the same blocking native dispatcher boundary, process-lifetime query embedder, four-round/eight-call/five-minute
   policy, aggregate-output ceiling, HTTP abort handle, and native cancellation signal as the other mapped providers.
6. Input tokens, output tokens, and optional provider-reported cost are accumulated across every Messages request in
   one logical generation and checkpointed through the existing usage path.

### Acceptance criteria

- Memory tools remain off by default and unavailable for oMLX or any model that does not advertise tools.
- The first Anthropic request contains exactly the three closed definitions; a follow-up contains the exact assistant
  tool-use block sequence and an immediately following user message with correlated tool results.
- Thinking and redacted-thinking blocks required by Anthropic's multi-round protocol are retained unchanged without
  entering provider-neutral reasoning text, durable tool output, exports, or WebView-visible call identity.
- Calls and exact structured success/error results survive reopen through the existing append-only provider-run
  records before any result is reused by the provider.
- Multiple rounds preserve cumulative usage/cost and shared cancellation while retaining the existing four-round,
  eight-call, 64 KiB per-result, 256 KiB aggregate-output, and five-minute ceilings.
- oMLX tool mapping, automatic injection, citation/context-panel replacement, persistent memory controls, document
  opening, web tools, and attachment retry remain outside this slice.

### Verification completed

Focused TDD first failed on missing Anthropic definition/call/result mapping, streamed block reconstruction, durable
round execution, and composer availability. Pure tests cover explicit capability discovery, closed definition mapping,
bounded fragmented object arguments, malformed non-object rejection, exact result correlation, signed thinking-block
preservation, reasoning/multimodal request preservation, and provider gating. A real schema-18 store test confirms that
the Anthropic provider identity and exact result persist before reuse. A signed host-loopback two-request SSE fixture
confirms that the second request contains the durable correlated result and produces the final answer with cumulative
token and cost usage. Prettier, `svelte-check`, all 60 frontend tests, the production build, Cargo formatting, Cargo
check, and all 222 default Rust tests pass; seven opt-in tests remain ignored by default. The native development build
compiled, reached the AppKit event loop, remained active, and stopped on request. Immutable live-store inspection
reported schema version 18, `quick_check=ok`, and no running provider records. No visual structure changed, so a
separate responsive layout review was not required. No live third-party Anthropic-compatible account or API key was
exercised, so the real credential/model/tool-call UI flow remains unverified.

## Prior completed product slice: Explicit OpenAI Chat Completions native memory-tool loop

### Goal

Map Bottie's three closed native memory tools into OpenAI-compatible Chat Completions and run repeated explicitly
enabled calls through the existing bounded dispatcher, state machine, durable tool records, provider stream, and shared
cancellation path, without adding Anthropic-shaped or oMLX mapping, automatic retrieval injection, citation cards,
context-panel replacement, or a schema migration.

### Implemented shape

1. The session-only Memory toggle is now available for Ollama or OpenAI-compatible models that explicitly advertise
   tool capability. Native generation rechecks the selected model through provider discovery before definitions are
   sent; the flag remains off by default and clears when the selected provider/model is not mapped and tool-capable.
2. OpenAI request mapping preserves the exact three native names, descriptions, and closed JSON schemas. Streamed
   function calls are reconstructed by provider index across argument fragments, require bounded non-empty call/name
   identities, the exact `function` discriminator, and complete JSON-object arguments.
3. Follow-up requests append the accumulated assistant reasoning/text and complete `tool_calls`, then one `role: tool`
   result per call with the exact matching `tool_call_id`. Missing, conflicting, malformed, non-object, or mismatched
   call data terminates the provider run without reflecting raw arguments in the stable error message.
4. OpenAI's provider call identity becomes the provider-neutral loop identity and durable correlation key. It remains
   Rust/SQLite-only and is returned unchanged only on the provider wire; reopened UI state and exports continue to omit
   native and provider call identities.
5. Every accepted invocation and exact bounded success/error envelope commits before provider reuse. OpenAI rounds use
   the same blocking native dispatcher boundary, process-lifetime query embedder, four-round/eight-call/five-minute
   policy, aggregate-output ceiling, HTTP abort handle, and native cancellation signal as Ollama.
6. Input tokens, output tokens, and optional provider-reported cost are accumulated across every Chat Completions
   request in one logical generation and checkpointed through the existing usage path.

### Acceptance criteria

- Memory tools remain off by default and unavailable for Anthropic, oMLX, or any model that does not advertise tools.
- The first OpenAI request contains exactly the three closed definitions; a follow-up contains ordered assistant calls
  and correlated `tool_call_id` results without native paths, hashes, embeddings, or WebView-visible call identities.
- Calls and exact structured success/error results survive reopen through the existing append-only provider-run
  records before any result is reused by the provider.
- Multiple rounds preserve cumulative usage/cost and shared cancellation while retaining the existing four-round,
  eight-call, 64 KiB per-result, 256 KiB aggregate-output, and five-minute ceilings.
- Anthropic-compatible and oMLX tool mapping, automatic injection, citation/context-panel replacement, persistent
  memory controls, document opening, web tools, and attachment retry remain outside this slice.

### Verification completed

Focused TDD first failed on missing Chat Completions definition/call/result mapping, streamed fragment reconstruction,
OpenAI durable round execution, and the composer's OpenAI availability contract. Pure tests cover explicit capability
discovery, closed definition mapping, bounded/reasoning/multimodal request preservation, fragmented call assembly,
malformed argument rejection, identity mismatch rejection, and provider gating. A real schema-18 store test confirms
that the OpenAI provider identity and exact result persist before reuse. A host-loopback two-request SSE fixture
confirms the second request contains the durable correlated result and produces the final answer with cumulative token
and cost usage. Prettier, `svelte-check`, all 60 frontend tests, the production build, Cargo formatting, Cargo check,
and all 219 default Rust tests pass; six opt-in tests remain ignored by default. The native development build compiled,
launched one `bottie` window, remained active, and stopped cleanly on request. macOS window inspection confirmed the
native window existed, but it could not be raised above the foreground Chrome window for a visual content review. No
live third-party OpenAI-compatible account or API key was exercised, so the real credential/model/tool-call UI flow
remains unverified.

## Prior completed product slice: Explicit Ollama native memory-tool loop

### Goal

Map Bottie's three closed native memory tools into Ollama's function wire format and run repeated explicitly enabled
Ollama calls through the existing bounded dispatcher, state machine, durable tool records, provider stream, and shared
cancellation path, without adding OpenAI-shaped or Anthropic-shaped mapping, automatic retrieval injection, citation
cards, context-panel replacement, or a schema migration.

### Implemented shape

1. The composer exposes a session-only Memory toggle only for Ollama models that advertise tool capability. The typed
   request defaults the flag off for older callers, and native policy requires the explicit flag, the Ollama route,
   and discovered model capability before any definition is sent.
2. Ollama request mapping preserves the exact three native names, descriptions, and closed JSON schemas. Streaming
   accumulates reasoning, answer text, and ordered complete function calls, then appends the accumulated assistant
   message plus one ordered `role: tool` result per call for the next round.
3. Ollama's native call shape has no stable provider call identity, so Bottie creates one opaque UUID per accepted call
   for state-machine correlation and durable storage. Provider order and tool names remain exact on the Ollama wire;
   native/provider call identities never reach the WebView or exports.
4. Every accepted invocation commits before dispatch, and its exact bounded common success/error envelope commits
   before it is serialized into Ollama's inert tool-result content. A checkpoint failure terminates the loop and
   withholds an unretained result from the provider.
5. One process-lifetime EmbeddingGemma owner now services query embeddings over a native worker channel, so tool
   retrieval reuses the application-owned model runtime rather than loading a second model per generation. Provider
   request rounds stay async while synchronous storage/embedding work runs on a blocking worker.
6. Ollama request usage is accumulated across every round. Cancellation raises both the HTTP abort handle and the
   native tool-loop signal; call, round, aggregate-output, and five-minute loop ceilings remain unchanged.

### Acceptance criteria

- Memory tools remain off by default and unavailable for non-Ollama or non-tool-capable models.
- The first Ollama request contains exactly the three closed native definitions; a follow-up contains the accumulated
  assistant call plus ordered tool-name/result messages without native IDs or paths.
- Calls and exact structured success/error results survive reopen through the existing append-only provider-run
  records before any result is reused by Ollama.
- Multiple rounds preserve cumulative usage and shared cancellation while retaining the existing four-round,
  eight-call, 64 KiB per-result, 256 KiB aggregate-output, and five-minute ceilings.
- OpenAI-compatible, Anthropic-compatible, and oMLX tool mapping, automatic injection, citation/context-panel
  replacement, persistent memory controls, document opening, web tools, and attachment retry remain outside this
  slice.

### Verification completed

Focused TDD first failed on missing Ollama tool request/call/result mapping, missing durable generation orchestration,
and the composer's unavailable Memory contract. Pure protocol tests now cover closed definition mapping, streamed
parallel calls, accumulated assistant history, ordered tool results, malformed calls, and explicit request gating. A
real schema-18 store test covers call/result persistence before provider reuse. A host-loopback two-request fixture
confirms the second Ollama request contains the durable result and produces the final answer with cumulative usage.
Prettier, `svelte-check`, all 59 frontend tests, the production build, Cargo formatting, Cargo check, and all 214
default Rust tests pass; five opt-in tests remain ignored by default. The host-loopback Ollama fixture also passes when
run explicitly, proving a two-request call/result/final-answer exchange through the real HTTP stream parser.

The browser preview was inspected at its default desktop viewport: the unavailable Memory control remains labelled,
contained, and disabled with its tool-capability explanation. A fresh `npm run tauri dev` build signed and launched the
native app. With Ollama and `qwen3:1.7b-q8_0` selected, macOS exposed the Memory control as enabled and pressed. One
local-only prompt completed a real `search_memory` call, rendered one Tool activity record, and returned a final answer.
Immutable read-only inspection of the live store reported schema 18, `quick_check=ok`, and a matching ordinal-zero,
non-error `search_memory` result on a completed provider run. The previous oMLX provider selection was restored before
the development runner was stopped. No schema migration, remote-provider call, or automatic memory injection occurred.

## Prior completed product slice: Provider-neutral bounded tool-loop state machine

### Goal

Execute repeated provider-neutral native memory-tool batches through the existing strict dispatcher while enforcing
one shared call-count, recursion, aggregate-output, deadline, cancellation, and terminal-state policy, without adding
provider wire mapping, generation integration, automatic retrieval injection, UI/IPC exposure, or a schema migration.

### Implemented shape

1. `ToolLoopState` accepts ordered batches of provider-neutral `NativeToolCall` values, preserves each opaque call
   identity on its correlated `NativeToolResult`, and routes the raw name/arguments through the existing strict
   dispatcher. One state instance spans every provider-to-tool recursion round in a future generation.
2. One loop accepts at most eight total calls and four non-empty tool rounds. A round that would exceed either ceiling
   fails before any call in that round executes; completed, cancelled, timed-out, and policy-failed state never reopens.
3. Complete correlated result serialization is accumulated under a 256 KiB generation-wide ceiling in addition to
   the dispatcher's existing 64 KiB per-result ceiling. The result that would exceed the aggregate limit is not
   returned to the future provider adapter, and later calls do not execute.
4. One five-minute deadline covers the state-machine lifetime. A cloneable native cancellation signal and the deadline
   are checked before and after every dispatcher call and before normal completion, so cancellation or expiry prevents
   subsequent tool work and becomes terminal.
5. Loop failures use fixed redacted `call_limit_exceeded`, `recursion_limit_exceeded`,
   `aggregate_output_exceeded`, `timed_out`, `cancelled`, or `invalid_state` categories. Provider-controlled names,
   arguments, identities, queries, outputs, embedding details, storage failures, and paths are never reflected.

### Acceptance criteria

- Repeated calls execute in order through the existing validated dispatcher and retain exact opaque call correlation.
- Total calls, recursion rounds, aggregate correlated output, and overall elapsed time have named native ceilings.
- Cancellation and deadline checks occur on both sides of every native call and before completion; any exceptional
  terminal state rejects later work.
- Calls or rounds rejected by a pre-execution limit perform no dispatcher, storage, or embedding work.
- No provider adapter mapping, generation-loop entry point, Tauri command, WebView state, persistence/schema change,
  automatic retrieval injection, citation UI, document opening, retention control, or attachment retry is added.

### Verification completed

Focused TDD first failed because the `tool_loop` module did not exist. Six native tests now cover two real dispatcher
rounds, exact call/result correlation, call and recursion ceilings before excess execution, aggregate serialization,
deadline and cancellation boundaries, normal completion, and terminal-state closure. The full verification results and
native smoke evidence for this slice are recorded in the pull request handoff. Prettier, `svelte-check`, all 58
frontend tests, the production build, Cargo formatting, Cargo check, and all 209 non-live Rust tests pass; four opt-in
live-provider checks remain intentionally ignored because this slice changes no provider request or stream protocol.
The host-native development command built, signed, and kept Bottie running while immutable read-only inspection of the
unchanged schema-18 live store reported `quick_check=ok`, ready semantic progress at 84/84 chunks, and no running
provider records. The development runner was then stopped and port 1420 had no listener. This slice has no UI, IPC,
provider request, migration, or live generation-loop entry point, so no interactive tool behavior or layout change is
claimed.

## Prior completed product slice: Provider-neutral memory tool dispatcher

### Goal

Execute the three validated native memory-tool contracts through one bounded structured success/error envelope,
without adding a provider execution loop, adapter mapping, automatic retrieval injection, UI/IPC exposure, a schema
migration, or broad memory controls.

### Implemented shape

1. `dispatch_memory_tool` validates the raw name and JSON through the existing closed definition contract, then routes
   the typed variant directly to `execute_search_memory`, `execute_open_memory`, or
   `execute_search_attached_files`. Unsupported or malformed calls stop before storage or embedding work.
2. Every successful call returns `{ "ok": true, "result": ... }`; every failure returns
   `{ "ok": false, "error": { "code", "message" } }`. The two shapes are exclusive and provider-neutral so later
   adapters do not need to infer success from tool-specific payloads.
3. Complete serialized success envelopes are capped at 64 KiB. Oversized or unserializable output becomes a small
   `output_too_large` error instead of crossing the execution boundary.
4. Contract, unavailable-provenance, storage, and embedding failures map to stable `unsupported_tool`,
   `invalid_arguments`, `unavailable`, `execution_failed`, or `output_too_large` categories with fixed safe messages.
   Raw names, arguments, queries, model errors, database details, and filesystem paths are not forwarded.
5. The dispatcher accepts Bottie's existing `SemanticEmbedder` boundary and remains Rust-only. It does not load a
   second model, expose a Tauri command, map provider wire formats, persist tool activity, or run multiple calls.

### Acceptance criteria

- All three advertised names route through their exact typed native executor and retain the existing profile,
  lifecycle, branch, association, result-count, excerpt, and provenance policies.
- Unsupported names and invalid JSON fail before database or embedding work; unavailable provenance and execution
  failures use distinct, redacted structured categories.
- Successful envelopes contain only `ok` plus structured `result`; failures contain only `ok` plus structured `error`.
  Complete success serialization cannot exceed 64 KiB.
- No provider adapter mapping, provider execution loop, recursion/call-count/timeout/cancellation state machine,
  prompt injection, citations UI, document opening, retention control, cache deletion, or attachment retry is added.

### Verification completed

Focused TDD first failed against the absent dispatcher module. Four native tests now cover successful routing for all
three tools, exclusive common-envelope serialization, validation before storage/embedding, redacted unavailable and
embedding failure mapping, and the serialized output ceiling. The complete Rust suite reports 203 passed with four
opt-in live-provider checks intentionally ignored. Prettier, `svelte-check`, all 58 frontend tests, the production
build, Cargo formatting, Cargo check, and `git diff --check` pass without warnings.

The host-native development command built, signed, and started Bottie; macOS WebKit logs confirmed the page completed
loading. While it remained active, immutable read-only inspection of the unchanged schema-18 live store reported
`quick_check=ok`, ready semantic progress at 84/84 chunks, and no running provider records. The development runner was
then stopped and port 1420 had no listener. This slice has no UI, IPC, provider request, migration, or provider-loop
entry point, so no interactive tool behavior or layout change is claimed.

## Prior completed product slice: Provider-independent memory tool definitions

### Goal

Publish one provider-neutral native definition set and strictly validate raw JSON arguments for `search_memory`,
`open_memory`, and `search_attached_files`, without adding an executor, provider adapter mapping, provider tool loops,
automatic retrieval injection, UI/IPC exposure, a schema migration, or broad memory controls.

### Implemented shape

1. `memory_tool_definitions` returns the three stable native names, bounded model-facing descriptions, and provider-
   neutral closed object schemas. Search schemas require `query`; opening requires exact `conversationId` and
   `messageId` provenance. Optional conversation/date/result and surrounding-turn properties declare their exact JSON
   types and native bounds, and every schema sets `additionalProperties` to false.
2. `validate_memory_tool_arguments` accepts only one advertised name and a JSON object matching that definition. It
   rejects missing/extra fields, JSON null or type mismatches, blank or overlong query/identity strings, contradictory
   inclusive dates, search limits outside 1-10, and surrounding-turn counts outside 0-3 before producing a typed enum.
3. Errors distinguish unsupported names from invalid arguments while returning only stable redacted messages. Raw
   provider-controlled JSON is never echoed into errors or diagnostics.
4. The existing hybrid executors remain unchanged except that direct conversation filters now share the definition's
   128-Unicode-scalar identity ceiling. No definition is mapped into an Ollama, OpenAI-shaped, Anthropic-shaped, or
   oMLX request, and no tool is executed or exposed through Tauri/WebView state in this slice.

### Acceptance criteria

- Exactly the three native memory tools are advertised through provider-neutral definitions with closed schemas,
  required fields, declared JSON types, and the existing native query/result/window ceilings.
- Valid raw JSON becomes the matching `SearchMemoryArguments`, `OpenMemoryArguments`, or
  `SearchAttachedFilesArguments` variant without stringly typed downstream dispatch.
- Unsupported names, non-object payloads, missing/unknown fields, nulls, incorrect number/string types, blank or
  overlong strings, contradictory dates, and out-of-range counts fail before any database or embedding work.
- Validation failures do not repeat raw argument data; schemas and serialized definitions expose no filesystem,
  database, hash, embedding, score, credential, or automatic-injection capability.
- Provider adapter mapping, provider execution loops, tool execution, prompt/retrieval injection, citations UI,
  document opening, retention/forget controls, cache deletion, and attachment retry behavior remain outside the slice.

### Verification completed

Focused TDD first failed because the shared definition/validation module and storage re-exports did not exist. Four
native tests now cover the exact advertised set and schema shape, valid typed conversion for all three tools,
unsupported names, non-object/missing/unknown/null/wrong-type inputs, semantic string/date/count limits, schema/result
ceiling parity, redacted errors, and omission of sensitive or unimplemented capabilities. Existing search-memory and
file-search validation tests also cover the shared 128-character conversation-identity ceiling.

The complete Rust suite reports 199 passed with four opt-in live-provider checks intentionally ignored. Prettier,
`svelte-check`, all 58 frontend tests, the production build, Cargo formatting, Cargo check, and `git diff --check` pass
without warnings. The host-native development command built, signed, and started Bottie; macOS WebKit logs confirmed
the page completed loading. While it remained active, immutable read-only inspection of the unchanged schema-18 live
store reported `quick_check=ok`, ready semantic progress at 84/84 chunks, 84 current embedding mappings, 84 sqlite-vec
row identities, 768 dimensions, index generation 1, and no running provider records. The development runner was then
stopped and no Bottie development process remained. This slice has no UI, IPC, provider request, migration, or
executable tool entry point, so no interactive feature, provider-call, or layout behavior is claimed.

## Prior completed product slice: Native `search_attached_files` contract

### Goal

Expose one provider-independent Rust contract for ranked retained-document excerpts with inspectable path-free file
provenance, without adding provider tool loops, automatic retrieval injection, document opening, UI/IPC exposure, a
schema migration, or broad memory controls.

### Implemented shape

1. `SearchAttachedFilesArguments` accepts a natively normalized 200-character query plus optional conversation,
   inclusive attachment-creation-time, and result-limit filters. Unknown fields and zero limits fail before embedding
   work; empty queries return an empty result without loading the model.
2. `execute_search_attached_files` fixes the source category to ready extracted attachments, caps tool output at ten
   matches, and reuses the existing built-in-profile, active/Archived association, Trash exclusion, hybrid-query, and
   reciprocal-rank policy. Final message answers cannot appear.
3. Each result contains a one-based fused order and an excerpt capped at 1,200 Unicode scalars. Provenance includes
   only the opaque attachment ID, safe display name, sniffed MIME type, original byte count, extraction format and
   character/page counts, creation time, and optional exact semantic chunk ordinal/offsets. Lexical-only results omit
   chunk location rather than inventing it.
4. The serialized contract omits query text, full extracted text, association identities, content/derivative hashes,
   lexical/semantic ranks, fused scores, cosine distances, vectors, embeddings, and filesystem/database/model/cache
   paths. No Tauri command, WebView state, provider request, export field, tool loop, or schema change was added.

### Acceptance criteria

- Returned matches are ready extracted documents only, preserve fused order, remain capped at ten results and 1,200
  Unicode scalars per excerpt, and carry bounded safe metadata plus opaque path-free attachment provenance.
- Conversation and inclusive attachment-date filters share the native hybrid policy; active and Archived associations
  remain eligible, while unassociated files, Trash-only files, and message sources do not appear.
- Invalid query/filter/limit contracts fail before embedding; empty search remains a no-work empty result; unknown
  serialized arguments are rejected.
- Semantic matches retain exact current-generation chunk offsets, lexical fallback remains usable without indexed
  vectors and omits nonexistent offsets, and retrieval scores or implementation details never enter the result.
- Provider execution loops, adapter mapping, automatic prompt injection, document opening, citations UI, memory-card
  replacement, reindex/cache behavior, retention/forget controls, and attachment retry behavior remain outside the
  slice.

### Verification completed

Focused TDD first failed against the absent module and execution method. Five native tests now cover typed/path-free
serialization, exact semantic provenance, conversation/date filtering, attachment-only and association eligibility,
the narrower result and excerpt ceilings, invalid and unknown arguments before embedding, empty-query no-work
behavior, lexical fallback, and Archived/Trash lifecycle policy. The complete Rust suite reports 195 passed with four
opt-in live-provider checks intentionally ignored. Prettier, `svelte-check`, all 58 frontend tests, the production
build, Cargo formatting, Cargo check, and `git diff --check` pass without warnings.

The sandboxed native launch first failed at the loopback development-server bind with `EPERM`; the host retry built,
signed, and started the Tauri development app successfully. While it remained active, immutable read-only inspection
of the unchanged schema-18 live store reported `quick_check=ok`, ready semantic progress at 84/84 chunks, 84 current
mappings, 84 sqlite-vec shadow row identities, 768 dimensions, index generation 1, and no running provider records.
The development runner was then stopped. This slice has no UI or IPC entry point, so no interactive feature or layout
verification is claimed; migration and live-provider checks were not applicable.

## Prior completed product slice: Native `open_memory` contract

### Goal

Resolve exact `search_memory` conversation/message provenance into a small surrounding retained-turn window without
adding provider tool loops, automatic retrieval injection, document search/opening, UI/IPC exposure, a schema
migration, or broad memory controls.

### Implemented shape

1. `OpenMemoryArguments` accepts the exact conversation and message identities returned by `search_memory`, plus
   optional before/after turn counts. Blank or more-than-128-character identities fail before database work, unknown
   serialized fields are rejected, and each side is capped at three final text turns with two as the default.
2. `execute_open_memory` reconstructs the matched message's own immutable owning-branch lineage, rather than the
   conversation's currently selected branch, and never changes selection. Shared ancestors and branch-local
   descendants stay ordered while sibling branches remain absent.
3. Results carry stable message-source provenance, the bounded conversation title, exact conversation/message
   identities, and ordered message identity, role, creation time, match marker, and answer text. Each answer is capped
   at 2,000 Unicode scalars with a visible truncation marker.
4. Only final, non-empty message answer text is eligible. Archived conversations remain available; Trash, failed,
   cancelled, partial, missing, mismatched, cross-profile, and reasoning-only targets are unavailable. Separate
   reasoning, provider/model metadata, attachments, hashes, scores, vectors, and native paths do not serialize.

### Acceptance criteria

- Exact provenance opens a bounded ordered context window around one final text message without changing the selected
  branch or admitting an alternative sibling lineage.
- At most three retained turns appear on either side and each returned answer stays within 2,000 Unicode scalars;
  unknown fields and invalid identities fail under native policy.
- Archived conversation memory remains openable, while Trash and non-final message states do not appear as targets or
  surrounding turns.
- The result is path-free and answer-only: reasoning, provider/model data, attachments, native storage details, search
  scores, embeddings, and vector/chunk implementation metadata remain absent.
- Provider execution loops, automatic prompt injection, document search/opening, citations UI, memory-card
  replacement, reindex/cache behavior, retention/forget controls, and attachment behavior remain outside the slice.

### Verification completed

Focused TDD first failed against the absent module and execution method. Five native tests now cover exact path-free
serialization, Archived/Trash policy, final-only answer text, unknown and invalid arguments, Unicode/window bounds,
branch-owned lineage independent of current selection, sibling exclusion, and selection preservation. The complete
Rust suite reports 190 passed with four opt-in live-provider checks intentionally ignored. Prettier, `svelte-check`,
all 58 frontend tests, the production build, Cargo formatting, Cargo check, and `git diff --check` pass without
warnings.

Immutable read-only inspection of the unchanged schema-18 live store reported `quick_check=ok`, ready semantic
progress at 84/84 chunks, 84 current mappings, and no running provider records. A fresh native development build
started under the signed Tauri runner, remained active while the same store integrity/progress checks passed, and was
then stopped with the development runner interrupt. This slice has no UI or IPC entry point, so no interactive feature
or layout verification is claimed; migration and live-provider checks were not applicable.

## Prior completed product slice: Native `search_memory` contract

### Goal

Expose one provider-independent Rust contract for ranked conversation-memory excerpts with inspectable path-free
provenance, without adding provider tool loops, automatic retrieval injection, document search, UI/IPC exposure, a
schema migration, or broad memory controls.

### Implemented shape

1. `SearchMemoryArguments` accepts a natively normalized 200-character query plus optional conversation, inclusive
   creation-time, and result-limit filters. Unknown fields and zero limits fail before embedding work; empty queries
   return an empty result without loading the model.
2. `execute_search_memory` fixes the source category to final conversation messages, caps tool output at ten matches,
   and reuses the existing built-in-profile, Archived/Trash, filter, hybrid-query, and reciprocal-rank policy. Ready
   extracted documents remain reserved for the later `search_attached_files` contract.
3. Each result contains a one-based fused order and an excerpt capped at 1,200 Unicode scalars. Provenance includes
   only the durable conversation ID/title, message ID, author role, creation time, and optional exact semantic chunk
   ordinal/offsets. Lexical-only results omit chunk location rather than inventing it.
4. The serialized contract omits query text, lexical/semantic ranks, fused scores, cosine distances, vectors,
   embeddings, hashes, and filesystem/database/model/cache paths. No Tauri command, WebView state, provider request,
   export field, tool loop, or schema change was added.

### Acceptance criteria

- Returned matches are final message answers only, preserve fused order, remain capped at ten results and 1,200
  Unicode scalars per excerpt, and carry enough opaque path-free provenance for a later `open_memory` contract.
- Conversation and inclusive-date filters share the native hybrid policy; Archived remains eligible, while Trash and
  non-message sources do not appear.
- Invalid query/filter/limit contracts fail before embedding; empty search remains a no-work empty result; unknown
  serialized arguments are rejected.
- Semantic matches retain exact current-generation chunk offsets, lexical fallback remains usable without indexed
  vectors and omits nonexistent offsets, and retrieval scores or implementation details never enter the result.
- Provider execution loops, automatic prompt injection, document search, citations UI, memory-card replacement,
  reindex/cache behavior, retention/forget controls, and attachment behavior remain outside the slice.

### Verification completed

Focused TDD first failed against the absent module and execution method. Five native tests now cover typed/path-free
serialization, exact semantic provenance, conversation/date filtering, the narrower result and excerpt ceilings,
invalid and unknown arguments before embedding, empty-query no-work behavior, lexical fallback, and Archived/Trash
lifecycle policy. The complete Rust suite reports 185 passed with four opt-in live-provider checks intentionally
ignored. Prettier, `svelte-check`, all 58 frontend tests, the production build, Cargo formatting, Cargo check, and
`git diff --check` pass without warnings.

Immutable read-only inspection of the unchanged schema-18 live store reported `quick_check=ok`, ready semantic
progress at 84/84 chunks, 84 current-contract mappings, no contract mismatches, no mapping orphans, and no running
provider records. Two native-launch attempts did not reach process creation because the host approval review timed out;
therefore no fresh app-window interaction is claimed. No UI, IPC, schema, picker, export, or provider behavior changed,
so browser layout, migration, and live-provider checks were not applicable to this slice.

## Prior completed product slice: Explicit semantic reindex control

### Goal

Expose one user-controlled semantic rebuild with durable path-free progress and restore-safe native coordination,
without deleting sources or model files and without adding memory tools, retrieval injection, provider changes, a
schema migration, or attachment retry behavior.

### Implemented shape

1. `SemanticIndexProgress` is the narrow serialized contract: lifecycle state, completed chunks, total chunks, and a
   stable failure category. Chunks, vectors, source identities, embedding inputs, model paths, and cache paths remain
   native-only.
2. `reset_semantic_index` uses one immediate transaction to delete derived embedding records and their triggered
   sqlite-vec rows, recount eligible deterministic chunks, clear the prior failure, and persist either pending `0/N`
   or ready `0/0` progress. It does not change schema version 18, chunks, source content, or cached model files.
3. `reindex_semantic_memory` shares the storage-management mutex with restore, waits for the current bounded batch,
   pauses the single semantic worker through its RAII guard, commits the reset, resumes on every return path, and
   explicitly wakes the worker. Restore therefore cannot replace the store during the reset.
4. Settings polls durable progress only while pending, loading, or indexing; it renders a bounded progress bar, honest
   ready/loading/indexing/failed copy, and one disabled-while-active `Reindex memory` action. Browser preview keeps the
   native-only action visibly unavailable.

### Acceptance criteria

- Reindexing removes every derived mapping/vector while retaining deterministic chunks, source content, and the
  application-owned model cache; an empty index stays ready.
- Reset state and counts survive reopening the SQLite store, and the existing bounded worker resumes from `0/N`
  without a second worker or an in-memory-only progress claim.
- Reindex and restore serialize through the same native management boundary; reset waits for a current semantic batch
  and the RAII pause resumes even when reset fails.
- The WebView receives only state, two counts, and a stable path-free error category. No query, excerpt, opaque source
  identity, vector, embedding, model/cache path, database detail, or generic storage capability crosses IPC.
- No schema, provider request, export format, memory tool, retrieval injection, model-cache deletion, or attachment
  behavior changes.

### Verification completed

Focused TDD first failed against the absent storage reset and frontend contract. Two native tests now cover
derived-only reset, vector-trigger cleanup, durable reopen state, and the empty-index case; five frontend tests cover
state copy, active polling policy, percentages, stable failures, the retained-source/cache explanation, disabled
browser behavior, and path-free presentation. The complete Rust suite reports 180 passed with four opt-in
live-provider checks intentionally ignored. Prettier, `svelte-check`, all 58 frontend tests, the production build,
Cargo formatting, Cargo check, and `git diff --check` pass without warnings.

The browser preview was inspected at the default desktop viewport and a 600-pixel responsive viewport: the Settings
dialog, semantic card, native-only disabled action, diagnostics, and footer remain readable without overlap or
horizontal clipping. A fresh signed native launch reopened the unchanged schema-18 live store with `quick_check=ok`,
`ready` progress at 84/84 chunks, 84 current-contract mappings, no contract mismatches, no mapping orphans, and no
running provider records. The real native Settings tree exposed `Ready · 84 of 84 chunks` and an enabled reindex
action. Activating it produced durable `indexing` progress at 8/84, then 56/84, and returned both SQLite and the UI to
`Ready · 84 of 84 chunks`; final `quick_check` remained `ok` with zero mismatches and zero orphans. A restore was not
performed during verification because it would replace the user's live store; restore serialization is enforced by
the shared management lock and worker-pause boundary exercised by the reset path.

## Prior completed product slice: Native reciprocal-rank fusion

### Goal

Combine bounded lexical and semantic memory rankings under one native filter contract without adding a schema change,
reindex control, memory tool/UI, retrieval injection, provider change, or WebView exposure.

### Implemented shape

1. `MemorySearchFilters` now owns the shared 200-character query ceiling, 50-result ceiling, source kind, conversation,
   and inclusive date policy used by lexical, semantic, and hybrid retrieval. Invalid filters fail before embedding.
2. The hybrid query retrieves at most 50 candidates from each native engine, groups them by source kind plus opaque
   source identity, and sums one reciprocal contribution per result list using the named rank constant `k = 60`.
3. Multiple semantic chunks from one source cannot inflate its score. The strongest semantic chunk supplies the fused
   result's exact excerpt, ordinal, and Unicode offsets; lexical-only sources retain their bounded FTS5 snippet.
4. Fused results are capped at 50 and use fused score, strongest rank, source creation time, source kind, and opaque
   source identity for deterministic ordering. Query text, rankings, excerpts, and identities remain Rust-only.

### Acceptance criteria

- A source found by both engines gains both reciprocal contributions and ranks ahead of equivalent single-list hits.
- A source contributes no more than once per list even when semantic search returns several of its chunks.
- Both engines receive the same profile, lifecycle, attachment-association, source, conversation, and inclusive-date
  policy; Archived remains eligible, Trash and unassociated documents do not.
- Empty and zero-limit queries avoid embedding, malformed filters and overlong queries fail before model work, and
  fused results never exceed 50.
- Fusion adds no migration, command, IPC type, Svelte state, provider request, export field, memory tool, or cache
  behavior.

### Verification completed

Focused TDD coverage first failed against the absent shared-filter and hybrid-query modules. Four fusion tests now
cover overlap scoring, duplicate semantic chunks, exact semantic provenance, lexical fallback excerpts, deterministic
ties, the 50-result ceiling, shared filtering, Trash exclusion, and pre-embedding validation. The adjacent 21-test
memory suite passes. The complete Rust suite reports 178 passed with four opt-in live-provider checks intentionally
ignored. Prettier, `svelte-check`, all 53 frontend tests, the production build, Cargo formatting, Cargo check, and
`git diff --check` pass without warnings. A fresh signed native launch reopened the unchanged schema-18 live store and
remained available until stopped after verification. Immutable read-only inspection reported `quick_check=ok`,
`ready` semantic progress at 84/84 chunks, 84 unique current-generation mappings, 84 sqlite-vec shadow row identities,
768 dimensions, index generation 1, no mapping orphans, and no running provider records. The standalone SQLite CLI
cannot load Bottie's statically linked `vec0` module, so verification counted its durable row-identity shadow table
instead of querying the virtual table directly. No schema, UI, IPC, or provider behavior changed, so migration,
browser layout, picker/provider interaction, and live-provider tests were not applicable.

## Prior completed product slice: Filtered semantic KNN retrieval

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
