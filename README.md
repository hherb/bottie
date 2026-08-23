# bottie

Bottie is a local-first desktop chatbot built with Tauri 2, Rust, Svelte, and TypeScript. It is designed to connect to oMLX, Ollama, Anthropic-compatible, and OpenAI-compatible inference providers while keeping application secrets, files, tools, and persistent memory behind the Rust boundary.

The current developer preview pairs the interactive product shell with real text and capability-gated image inference
through oMLX, Ollama, OpenAI-compatible, and Anthropic-compatible providers. The Rust core validates provider
endpoints, discovers models, tests connections, streams normalized answer and reasoning events over a typed Tauri IPC
channel, and owns end-to-end cancellation. Conversations and their ordered text/reasoning messages persist in a
Rust-owned bundled SQLite database and reopen after restart. Accepted provider runs retain their model, generation
settings, terminal state, elapsed time, provider-reported token/cost usage, and checkpointed partial output. If Bottie
exits during a
generation, its next launch marks that run interrupted and reopens the response with the text and reasoning already
saved. Users can edit earlier prompts or regenerate responses into preserved, switchable conversation branches, and
search active or archived histories by title or visible message text. Search results open the preserved branch that
contains the match. Assistant answers render sanitized Markdown for readable headings, lists, tables, links, and code;
raw HTML is escaped and remote Markdown images are reduced to inert labels. Each non-empty assistant answer can be
copied as Markdown without generated HTML or response metadata. When separate reasoning is present, the clipboard
document includes labelled Reasoning and Response sections. Interrupted, cancelled, and transiently failed responses
can be retried on a preserved alternative branch without overwriting the original attempt. Durable Good and Poor
ratings can be set, changed, or cleared on each preserved assistant response and survive restart and branch switching.
Native provider runs retain ordered structured tool calls and one append-only result per call. Each call records its
native execution classification, stable outcome, and native-work duration; historical calls remain labelled legacy.
Reopened and just-completed tool activity is visible in a calm expandable audit panel, with raw payloads nested below
the audit summary. A Rust-only provider-neutral state machine can now execute repeated
native tool batches through the strict dispatcher while enforcing an eight-call, four-round, 256 KiB aggregate output,
30-second deadline, and shared cancellation policy across oMLX, Ollama, OpenAI-compatible, and Anthropic-compatible wire
formats. Every explicitly tool-capable route also receives a closed zero-argument `current_time` tool backed only by
Rust's UTC system clock; it exposes no timezone, locale, hostname, path, or platform detail.
A separate Rust-only web-search boundary now normalizes bounded queries and inert result metadata behind a pluggable
provider contract. Fixed Brave Search and Exa Search adapters use provider-owned HTTPS endpoints, disable redirects,
keep API keys in sensitive request headers, cap response bytes, and accept only absolute HTTP(S) result URLs. Settings
stores the two keys independently through the operating-system credential vault, can test either fixed route with one
bounded native probe, and persists one secret-free active search-engine choice without returning results or keys to the
WebView. A separate closed
provider-independent `web_search` definition now validates day/week/month/year freshness, bounded include/exclude DNS
filters, and result limits before a safe native dispatcher can execute the selected provider. Brave maps freshness and
domain policy to its query API; Exa maps domains to native arrays and freshness to an absolute publication-date lower
bound. Both adapters recheck returned hosts. A session-only Web toggle is available for tool-capable oMLX, Ollama,
OpenAI-compatible, and Anthropic-compatible models. When enabled, each mapped provider can call the closed definition
through Bottie's existing bounded native loop; every call and exact result is checkpointed before provider reuse.
A separate closed `web_fetch` foundation validates one absolute public HTTP(S) URL, blocks IP literals, special-use
names, non-default ports, and any non-public DNS answer, pins accepted addresses per hop, and revalidates at most three
redirects under one 15-second deadline. It accepts at most 48 KiB of valid UTF-8 HTML, XHTML, or plain page source,
parses HTML/XHTML into at most 24 KiB of inert visible text, and returns the final source URL with a bounded optional
title and publication value. Raw markup, executable/embedded element content, and media type are omitted, while the
result remains explicitly untrusted inside the existing 64 KiB dispatcher envelope. Explicitly enabled tool-capable
oMLX, Ollama, OpenAI-compatible, and Anthropic-compatible requests advertise it after `web_search`, execute it through
the same durable bounded loop, and checkpoint the exact result before provider reuse.
Successful selected-lineage search and fetch results also create deduplicated, removable Web source cards in the
Context panel. Cards expose only normalized public HTTP(S) source metadata and inert excerpts; a later fetched page
supersedes an earlier search result for the same URL, while removal stays session-local and leaves the durable audit
unchanged. Fetched-page cards label the native result as untrusted and calmly explain that external page text may
contain misleading instructions. The same notice appears before an explicitly untrusted result in the expandable tool
audit; failed, malformed, search, and unmarked legacy results cannot acquire that label.
When native capability checks enable Web for a mapped provider, Bottie adds fixed guidance asking the model to consult
`current_time` before interpreting relative or current dates and to cite Web-grounded claims with inline Markdown links
to exact result URLs. The stored answer retains those links unchanged.
On completion and reopen, Bottie marks a link as a Web citation only when its normalized destination matches a
successful Web result retained on that same response; the matching Context card is labelled `Cited in response`.
Unmatched model-authored links remain ordinary safe external links, and copied or exported Markdown preserves the
same claim-level link without adding opaque tool identities.
Settings now persist a secret-free Web destination policy behind the Rust boundary. HTTPS-only access is the default;
users may save up to 32 combined allowed and blocked public DNS names, with parent-domain matching and blocked-domain
precedence. Rust filters normalized search results before the common tool envelope and applies the same immutable
per-generation policy before a fetch plus after every redirect. Disabling HTTPS-only permits public HTTP without ever
permitting IP literals, loopback, private, special-use, mixed public/private DNS answers, non-default ports, ambient
proxies, or automatic redirects.
The selected visible branch can be saved as either human-readable Markdown or versioned, machine-readable JSON. Both
formats retain separate reasoning, response status, provider/model provenance, local ratings, retained tool activity,
and path-free attachment metadata. When the selected lineage or conversation scope references retained files, Rust
writes a ZIP containing the document plus one deduplicated original file per content hash; exports without attachments
remain plain Markdown or JSON. Bottie also restores
the exact last-open conversation after restart and preserves an intentional blank new-chat view. Native backup
controls can create a verified SQLite snapshot with original attachment blobs and ready normalized derivatives embedded
in backup-only tables, or restore a validated Bottie backup after explicit confirmation and an automatic pre-restore
safety copy.
After a successful native startup, an app-private background rotation also creates at most one verified snapshot every
24 hours and retains the seven newest automatic snapshots without pruning manual or pre-restore backups. If SQLite
reports corruption at startup, Bottie opens a restricted recovery screen instead of mutating the damaged store. Users
can restore the newest verified automatic snapshot or choose a manual backup; Rust preserves the damaged database
bundle in app-private storage before replacement.
The implemented [migration rollback contract](MIGRATION-ROLLBACK.md) keeps schema changes forward-only. Supported older
stores are preflighted read-only, copied into an isolated same-volume candidate, migrated and validated there, and
promoted only after a separate verified source-version recovery point and native-only marker exist. Startup reconciles
an interrupted promotion before ordinary corruption classification by accepting a valid target or restoring the
verified source. The two newest strict migration recovery points remain separate from automatic backup rotation;
attachments and the embedding-model cache are never changed by schema migration.
Remote inference, Brave Search, and Exa Search API keys stay in the operating-system credential vault and never return to
the WebView. On macOS, Touch ID gates the first read of each saved cloud credential
per Bottie session; successful unlocks are cached only in process memory. The native attachment picker now streams up
to eight selected files into application-private, SHA-256-addressed storage with a 25 MiB per-file ceiling,
content-based MIME detection, safe display names, and duplicate reuse. The WebView receives no filesystem path,
labels each retained item as local-only, and can atomically associate the current selection with the submitted user
message. Associations reopen on the selected branch, survive edit/regenerate forks, and can be removed without deleting
the retained content blob. Retained draft items can also be promoted into an existing conversation's durable,
branch-independent context, bounded to eight distinct files and removable without deleting content. Plain-text,
Markdown, PDF, and DOCX text up to 2 MiB is extracted into durable native-only state. PDF parsing is additionally
limited to 500 pages and 8 MiB of decompressed content per page. DOCX parsing
validates the package manifest and bounds archive entries, total declared expansion, main-document XML, XML events,
and XML depth. JPEG and PNG images are decoded within dimension, pixel, allocation, and output ceilings, have EXIF
orientation applied to their pixels, and are re-encoded without source metadata into application-private,
content-addressed derivatives. Ingestion commits durable pending state and returns before document parsing or image
decoding; one native background worker resumes pending work after startup, selection, or restore. The WebView receives
path-free live extraction, indexing-readiness, or normalization updates, dimensions, counts, and sizes but never
extracted text, derivative identities, bytes, or paths. Extracted text becomes explicitly indexable in the same
resumable worker; unsupported and failed extraction become terminal unsupported or blocked readiness. A second native
worker now derives resumable local EmbeddingGemma vectors from eligible message and document chunks without exposing
FTS/chunk/vector state or embeddings. A current
normalized JPEG or PNG can be sent only after native
discovery confirms that the
selected model advertises vision support. Text-only selections block a current image with an explicit explanation and
omit older image associations; documents remain excluded from automatic provider context. Native delivery reconstructs
the selected durable
lineage, reads at most eight normalized images and 50 MiB per request, and emits provider-native Ollama, OpenAI-shaped,
or Anthropic-shaped image blocks without exposing bytes to JavaScript. An explicit session-only Memory toggle is
available for oMLX, Ollama, OpenAI-compatible, and Anthropic-compatible models that advertise tools. Those requests may
execute bounded native conversation or retained-document retrieval and return path-free excerpts through the
provider's native tool shape; no document is injected automatically. The native dispatcher classifies every
registered tool as safe or approval-required before validation or execution. The bounded read-only memory and Web
contracts have explicit safe entries; unknown tools fail closed, and any future approval-required
call must consume a Rust-owned grant over its exact provider call ID, name, and arguments. No approval UI or
approval-required tool is registered yet. oMLX-owned MCP execution, other office formats, and direct document delivery
remain unimplemented. A separate Web toggle is off by default and available for tool-capable oMLX, Ollama,
OpenAI-compatible, and Anthropic-compatible models. It requires the selected search engine's credential from the native
vault and sends only
model-selected bounded search queries and filters to that fixed route; Ollama prompts stay on loopback, while
cloud-model prompts continue over their already-visible provider route. Privacy and activity surfaces identify the
selected Brave Search or Exa Search hop whenever Web is enabled.

## Development

Prerequisites:

- A current Rust toolchain
- Node.js and npm
- The platform prerequisites required by Tauri 2

Install and verify:

```sh
npm install
npm run format:check
npm run check
npm test
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

See `CONTRIBUTING.md` for the mandatory documentation, file-size, line-length, pure-function, named-constant, TDD, and
slice-documentation rules.

Run the native desktop application:

```sh
npm run tauri dev
```

On macOS, this package script development-signs each freshly linked executable with the only usable Apple Development
identity in the active keychains before Cargo runs it. This prevents macOS execution-policy scans from holding a new
large debug binary for minutes. No certificate name or fingerprint is stored in the repository, and release signing
is unchanged. If more than one Apple Development identity is usable, set `BOTTIE_APPLE_SIGNING_IDENTITY` to the exact
certificate label or SHA-1 fingerprint for the intended identity. Other platforms and Tauri commands pass through
without development signing.

With oMLX or Ollama running on its default loopback port, the native app discovers available models automatically.
Provider and model use separate selectors; changing providers refreshes that provider's models, and the last successful
pair is restored after restart. Settings can change either endpoint and test it before saving. Rust rejects non-loopback
hosts, embedded credentials, paths, query strings, and fragments; redirects are disabled, and no HTTP capability is
exposed to the WebView. oMLX discovery reads explicit VLM/residency metadata from `/v1/models/status` and accepts tools
only when its bounded fixed OpenAPI request schema contains both `tools` and `tool_choice`. Ollama
discovery also normalizes model capabilities, context size, and loaded/on-demand state.

On a genuinely new native installation, Bottie pauses before the first conversation to show the discovered
provider/model and whether its route is local or cloud. The disclosure distinguishes provider-delivered prompts,
normalized images, and explicitly enabled tool results from local conversations, files, and derived memory; Memory and
Web remain session-only and off by default, and credentials remain in the OS vault. Setup cannot complete until one
usable provider/model pair has been remembered. Settings files written before this flow are treated as already
acknowledged, so upgrading does not replay first-run setup.

Settings also support HTTPS OpenAI-compatible and Anthropic-compatible profiles plus path-free Web destination
controls. API keys are written and removed through narrow Rust commands backed by the OS credential vault. Cloud
routes are visibly labelled before sending, redirects stay disabled, and remote response usage and provider-reported
cost metadata are preserved when available. Anthropic discovery accepts the current nullable structured capability
object and `max_input_tokens`, while compatible endpoints retain explicit legacy capability-array gating for Bottie's
client-executed tools. Bottie omits its provider-neutral sampling default from Anthropic Messages requests because
Claude Sonnet 5 rejects non-default sampling parameters.

Recent diagnostics in Settings can be exported explicitly as version 1 `bottie-local-diagnostics` JSON for the
current application session. Rust snapshots the existing 100-event bounded history in recorded order, reapplies
credential, path, and content-shaped redaction, and opens a date-normalized native Save dialog only when the history is
non-empty. The document declares that credentials, provider request/response bodies, raw tool arguments/results,
database or attachment content, and native paths are omitted. The WebView receives only saved/cancelled state and the
selected leaf filename; Bottie does not upload the document automatically.

Thinking/reasoning defaults to off and can be toggled to low effort for each request. Reasoning-capable providers stream
that material into a collapsed, user-expandable section rather than mixing it into the answer. Native generation also
applies a 4,096-token default ceiling when the interface does not provide a tighter limit.

The first submitted prompt creates a durable conversation for the built-in local profile. User messages commit before
provider inference begins. Rust creates an empty assistant checkpoint with each accepted run, appends streamed text,
reasoning, and usage before forwarding those events to the interface, and commits terminal state before the next
prompt can be sent. On startup, any run left active by a prior process becomes an interrupted partial response. The
sidebar groups real conversation activity by local calendar date. Conversations can be renamed inline, archived, moved
to recoverable trash, and restored without losing messages. The initial SQLite schema models conversations, main
branches, ordered messages, separate text/reasoning blocks, provider runs, append-only usage snapshots, ordered tool
invocations, single append-only tool results, and ordered user-message attachment associations. Provider-controlled tool
names, call identities, argument objects, and outputs are validated and bounded before insertion. Reopened assistant
responses recover provider-reported
token/cost metadata and structured tool activity without estimating or executing anything. The native store
owns the local profile's last-open selection; opening or creating a conversation records it, while New chat clears it.
Conversation search runs through a narrow Rust command, treats query characters literally, excludes Trash and separate
reasoning blocks, and returns at most 50 activity-ordered results with bounded snippets. That user-facing navigation
search remains a literal scan distinct from the native-only FTS5 memory foundation. Assistant answers pass through a
Markdown parser configured to reject raw HTML, non-HTTP(S)/email link destinations, relative navigation, and image
fetches. User prompts and provider reasoning remain plain text. Copying writes the exact stored assistant answer through
the WebView clipboard API when no reasoning is present. Responses with reasoning become one Markdown document with
labelled Reasoning and Response sections, and the action reports success or failure in place. Interrupted, cancelled,
timeout, unavailable-provider, provider-server, and malformed-response attempts expose a labelled retry action. Retry
forks the unchanged request and uses the provider/model/reasoning route currently visible in the toolbar, while the
original attempt remains available through branch switching and its failure copy stays outside the next provider
context. Good and Poor rating controls
write only the exact visible assistant response through a narrow native command. Selecting the active choice clears it;
ratings stay local, survive restart, and remain attached to their preserved branch response. The toolbar's Markdown
and JSON export actions reconstruct only the selected lineage and ask Rust to show a format-filtered native Save
dialog. JSON uses version 4 of the `bottie-conversation` contract and retains ordered text, separate reasoning, message
state, provider/model provenance, local rating, creation time, and provider-reported generation metadata without
opaque storage identifiers. Conversation- and message-scoped attachment entries include safe display metadata, hashes,
and archive-relative members. Retained tool arguments/results are included without native run or provider call IDs. A
separate global JSON action writes every active and archived conversation's selected lineage through version 4 of the
`bottie-conversation-batch` contract while excluding Trash and hidden branch siblings. Exports with referenced files
are ZIP bundles containing the UTF-8 document and one hash-deduplicated original blob; otherwise their historical plain
file shape is unchanged. The WebView receives only a saved/cancelled outcome and leaf filename, never the chosen
directory. A separate global backup action asks Rust to create and verify a complete SQLite snapshot with SQLite's
online backup API, including committed WAL content, original attachment blobs, and ready normalized derivatives in
backup-only tables, while likewise returning no local path. Restore opens and validates the selected database and every
embedded hash in Rust, migrates an isolated staging copy when supported, creates an application-private portable
snapshot of the current store, rehydrates attachment bytes through a private staging tree, and only then replaces the
live database and attachment root. The WebView receives leaf filenames rather than database paths. Native
startup rotation creates and verifies a new app-private snapshot when the newest is at least 24 hours old, retains the
seven newest managed snapshots, and reports its path-redacted result in Recent diagnostics. Before normal store
initialization, Rust opens existing data read-only and classifies explicit SQLite corruption or a non-`ok` integrity
result. Recovery mode blocks normal conversation connections and skips automatic rotation. Its WebView status contains
only verified automatic-snapshot count and newest timestamp. Restoring either that newest snapshot or a manually
selected backup stages, migrates, and verifies a replacement before preserving the damaged main database, present
WAL/shared-memory sidecars, and prior attachment tree in app-private storage. Successful replacement resumes normal
conversation access without returning a filesystem path. A separate native picker ingests files through a bounded
streaming copy into an
application-private attachment directory. SQLite stores only sanitized metadata and the SHA-256 content identity;
identical content reuses its existing blob across restarts. Ingestion leaves extraction and supported image
normalization durably pending, then wakes a single process-lifetime worker instead of parsing or decoding inside the
picker command. The worker handles one item at a time, resumes pending rows after startup or restore, and emits only
path-free terminal metadata to update visible draft and message attachments. Store replacement pauses the worker after
its current bounded item and resumes it afterward. Sending commits up to eight selected attachment identities
with the user message, then clears the draft. Reopened selected lineages reconstruct ordered path-free metadata; edited
and regenerated request branches inherit the source request's associations. Detaching is limited to visible user
messages while generation is idle and retains the catalog row and blob for deduplication. Conversation-scoped
associations apply on every branch and future request;
ready images are treated as current vision context and deduplicated when the same file is message-linked, while
documents remain excluded from automatic context and cloud routes. Before drafts or background processing begin on
each successful non-recovery startup,
Rust deletes catalog entries older than a 24-hour safety window with no message or conversation association, including
their extraction and normalization metadata. It then removes only equally old, strict hash-addressed
original/derivative files absent from the surviving catalog and clears old interrupted attachment temporary files.
Recoverable Trash references, recent cross-process drafts, and shared derivatives remain live; unexpected files are
left untouched. Cleanup commits catalog changes before file sweeping and holds a SQLite write lock during the sweep so
interruption can leave only harmless untracked bytes for the next pass. Recent diagnostics receives counts and
reclaimed bytes without paths or content identities. Current schema-version-21 stores retain up to 2 MiB of UTF-8
plain-text,
Markdown, derived PDF text, or derived DOCX text inside SQLite, resume pending work after migration or interruption,
and expose only path-free state to the interface. Each attachment also retains waiting-for-extraction, indexable,
unsupported, or blocked readiness; ready non-empty text now feeds the derived whole-source FTS5 index. PDF extraction
retains a page count, refuses files over 500 pages,
bounds each decompressed page stream to 8 MiB, and reports encrypted, malformed, text-free, extraction, and size
failures without parser details or paths. DOCX extraction recognizes the package by its manifest rather than its
filename, reads only bounded ZIP members in memory, rejects overlapping/encrypted/over-complex archives, and bounds
WordprocessingML size, events, depth, and output. JPEG and PNG normalization accepts at most 8,192 pixels on either
axis, 16 million total pixels, 128 MiB of decoded image allocation, and 25 MiB of encoded output. JPEG EXIF orientation
is applied before both formats are re-encoded through metadata-free encoders into application-private,
content-addressed derivatives. Portable manual, automatic, and pre-restore SQLite backups embed original blobs plus
ready normalized derivatives in verified backup-only tables. Conversation exports include selected-scope original
files, never extracted SQLite text or normalized derivatives. Ready JPEG/PNG derivatives are read only by Rust for
capability-confirmed vision requests, with
an eight-image and 50 MiB selected-lineage request ceiling; documents remain absent from automatic message content.
Other office formats are not extracted. Indexed document text stays native until an explicitly enabled
`search_attached_files` call returns a bounded path-free excerpt to a mapped oMLX, Ollama, OpenAI-compatible, or
Anthropic-compatible model.
Ready
images also receive a
metadata-free thumbnail capped to 320 pixels on either axis. The WebView requests that thumbnail through a private
`bottie-attachment` protocol using only the opaque attachment ID; Rust rejects non-GET, malformed, missing, pending,
unsupported, and failed requests without returning a path, hash, derivative identity, or native diagnostic. Failed
text extraction and image normalization remain linked locally and now show a specific, accessible consequence in the
composer, context panel, and retained message presentation. Preview generation does not add retry controls or change
provider delivery.

Schema version 16 begins the persistent-memory search foundation with a derived native SQLite FTS5 index. It aggregates
each final user or assistant answer into one source, excludes separate reasoning and non-final responses, and indexes
ready extracted documents as whole sources. A bounded Rust-only query layer provides BM25 ranking plus source,
conversation, and date filters while excluding Trash and unassociated drafts. This is not yet a user-facing memory
feature: no FTS query, excerpt, source identity, or extracted document content crosses IPC or enters a provider request.
Schema version 17 derives the same eligible text into deterministic, versioned, Unicode-safe chunks. Version 1 keeps
exact source offsets and stable SHA-256 identities, prefers whitespace boundaries at up to 1,200 characters, and uses
approximately 200 characters of overlap. Schema version 18 statically registers sqlite-vec and resumably maps those
chunks to 768-dimensional cosine vectors using FastEmbed 6 and Q4 EmbeddingGemma 300M. The single native worker owns
the application-data model cache, eight-chunk transactions, durable progress/failure state, and restore pause/resume.
A bounded Rust-only semantic query contract applies EmbeddingGemma's retrieval prompt, validates one 768-dimensional
query vector, and runs exact cosine KNN against only current-generation row identities that satisfy the same profile,
source, conversation, association, lifecycle, and inclusive-date policy. Queries and results are capped at 200
characters and 50 chunks. A Rust-only hybrid query applies that shared filter contract to lexical and semantic
candidates, groups them by source, and combines one rank per engine with reciprocal-rank fusion (`k = 60`). The best
semantic chunk supplies exact excerpt offsets while lexical-only sources retain bounded FTS5 snippets. Reindex
controls are now explicit in Settings: the WebView receives only durable state, completed/total counts, and a stable
failure category. Reindexing serializes with restore, pauses the worker, atomically removes only derived vectors,
retains chunks and the application-owned model cache, then resumes bounded background work. Native-only
`search_memory`, `open_memory`, and `search_attached_files` contracts provide bounded message excerpts, surrounding
final turns, and ready-document excerpts with path-free provenance for the mapped tool runtime. One
provider-independent definition set now advertises closed JSON schemas for those three tools and strictly validates
raw names and arguments
into their typed native contracts. A Rust-only provider-neutral dispatcher executes one validated call and returns an
exclusive structured success/error envelope capped at 64 KiB, with stable redacted failure categories. Provider
independent loop state now correlates repeated calls and results across at most four rounds, eight calls, 256 KiB of
aggregate serialized output, and 30 seconds while checking a shared cancellation signal before and after every native
call. Tool-capable oMLX, Ollama, OpenAI-compatible, and Anthropic-compatible models receive those definitions only after
the
user enables Memory. Rust accumulates streamed calls, checkpoints each call/result under the active provider run,
reuses the process-lifetime EmbeddingGemma worker for semantic queries, appends provider-native correlated tool-result
messages, and aggregates usage and optional cost across follow-up requests. Tool activity is inspectable and portable;
paths, hashes, scores, vectors, embeddings, cache details, and provider/native call identities remain excluded.
Successful selected-lineage tool results also produce deduplicated conversation, retained-file, or Web source cards in
the Context panel. Removing a card is session-local presentation state and does not delete the append-only audit record
or exclude the source from later retrieval. Automatic retrieval injection remains unimplemented.

Run the layout-only browser preview:

```sh
npm run dev
```

The browser preview deliberately disables sending and reports that native inference is unavailable.

Four ignored tests exercise streaming and cancellation against live local providers:

```sh
cargo test --manifest-path src-tauri/Cargo.toml live_omlx_stream -- --ignored --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml live_ollama_stream -- --ignored --test-threads=1
```

One additional ignored live oMLX check exercises `current_time` plus explicitly enabled `open_memory` through Bottie's
durable dispatcher and verifies the reopened results:

```sh
cargo test --manifest-path src-tauri/Cargo.toml live_omlx_clock_and_memory -- --ignored --test-threads=1
```

## Planned architecture

The WebView owns presentation state. The Rust application core will own:

- provider configuration, credentials, and streaming inference;
- conversation and attachment persistence;
- SQLite FTS5 and vector memory retrieval;
- file extraction and normalization;
- web search, fetch, and other model tools;
- privacy policy enforcement and audit data.

The provider layer normalizes OpenAI, Anthropic, Ollama, and oMLX responses into a capability-aware internal event
stream rather than assuming every compatible endpoint behaves identically.

The native memory subsystem uses FastEmbed 6 with Q4 EmbeddingGemma 300M as its single built-in embedding model.
Bottie owns the model cache, durable acquisition/index progress, semantic-query prompting, and embedding/index
versions; users do not configure a second inference provider merely to enable local memory indexing. A native-only
`search_memory` contract now returns at most ten hybrid-ranked final-message excerpts with path-free conversation and
message provenance for the mapped tool runtime. A matching native-only `open_memory` contract resolves exact provenance
into the matched message's immutable branch lineage, returning at most three final text turns on either side without
changing the selected branch. Native-only `search_attached_files` applies the same bounded hybrid policy to ready
extracted documents that retain an active or Archived association, returning safe file metadata and optional exact
chunk offsets without paths, hashes, scores, or full extracted text. Provider-independent definitions expose those
three memory tools plus `web_search` and `web_fetch` through closed schemas. Required and optional
fields, JSON types, Unicode-scalar identity/query/URL ceilings, result/window bounds, public-network policy, and
unknown-field rejection are enforced before typed dispatch. Memory and Web provider invocation is available only
through explicitly enabled, tool-capable oMLX, Ollama, OpenAI-compatible, and Anthropic-compatible requests. The common
dispatcher additionally applies an explicit native execution policy before argument validation: current bounded
read-only memory and Web tools are safe, unknown tools fail closed, and future
approval-required tools cannot run without an exact Rust-owned call grant. Providers and WebView arguments cannot
grant approval, and grants are consumed rather than reusable. Successful selected-lineage results appear as path-free
removable Context-panel
citations. Each active or Archived conversation also has a durable reversible exclude-from-memory action in its
navigation menu. Excluded conversations remain readable and exportable, but their messages and attachment
associations are unavailable to lexical, semantic, `search_memory`, `open_memory`, and `search_attached_files`
retrieval; a shared file remains eligible only through another non-excluded association. Trash adds a separately
confirmed `Forget permanently` action. Rust accepts only a trashed local-profile conversation with no active response,
then deletes its branches, messages and reasoning, provider/usage records, tool audit, ratings, memory preference,
attachment links, and message-derived lexical/chunk/vector rows in one transaction. Content-addressed attachments
shared elsewhere remain; newly unreferenced originals, extraction text, and derivatives keep the existing 24-hour
cross-process safety window before startup garbage collection removes them. Existing exports, manual backups, and
automatic recovery snapshots are not rewritten and must be managed separately. The application-owned embedding-model
cache is not conversation data and is retained. Automatic memory injection, document opening, and attachment retry
remain unavailable. Settings also exposes opt-in Trash
retention:
keep until manual forget (the default), 30 days, 90 days, or one year from the time a conversation enters Trash. Rust
stores only that
bounded period and permanently removes expired Trash on the next healthy app launch through the same live-store
cascades as explicit forget. Active and Archived conversations are never affected. Unshared attachments retain the
existing 24-hour safety window; exports, backup snapshots, and the application-owned embedding-model cache are not
rewritten or deleted by retention.
