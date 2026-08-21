# bottie

Bottie is a local-first desktop chatbot built with Tauri 2, Rust, Svelte, and TypeScript. It is designed to connect to oMLX, Ollama, Anthropic-compatible, and OpenAI-compatible inference providers while keeping application secrets, files, tools, and persistent memory behind the Rust boundary.

The current developer preview pairs the interactive product shell with real, text-only inference through oMLX,
Ollama, OpenAI-compatible, and Anthropic-compatible providers. The Rust core validates provider endpoints, discovers
models, tests connections, streams normalized answer and reasoning events over a typed Tauri IPC channel, and owns
end-to-end cancellation. Conversations and their ordered text/reasoning messages persist in a Rust-owned bundled
SQLite database and reopen after restart. Accepted provider runs retain their model, generation settings, terminal
state, elapsed time, provider-reported token/cost usage, and checkpointed partial output. If Bottie exits during a
generation, its next launch marks that run interrupted and reopens the response with the text and reasoning already
saved. Users can edit earlier prompts or regenerate responses into preserved, switchable conversation branches, and
search active or archived histories by title or visible message text. Search results open the preserved branch that
contains the match. Assistant answers render sanitized Markdown for readable headings, lists, tables, links, and code;
raw HTML is escaped and remote Markdown images are reduced to inert labels. Each non-empty assistant answer can be
copied as Markdown without generated HTML or response metadata. When separate reasoning is present, the clipboard
document includes labelled Reasoning and Response sections. Interrupted, cancelled, and transiently failed responses
can be retried on a preserved alternative branch without overwriting the original attempt. Durable Good and Poor
ratings can be set, changed, or cleared on each preserved assistant response and survive restart and branch switching.
Native provider runs can also retain ordered structured tool calls and one append-only result per call. Reopened tool
activity is visible in a calm expandable panel, even though provider tool loops and execution remain intentionally
deferred.
The selected visible branch can be saved as either human-readable Markdown or versioned, machine-readable JSON. Both
formats retain separate reasoning, response status, provider/model provenance, local ratings, and retained tool
activity. Bottie also restores
the exact last-open conversation after restart and preserves an intentional blank new-chat view. Native backup
controls can create a verified SQLite
snapshot or restore a validated Bottie backup after explicit confirmation and an automatic pre-restore safety copy.
After a successful native startup, an app-private background rotation also creates at most one verified snapshot every
24 hours and retains the seven newest automatic snapshots without pruning manual or pre-restore backups. If SQLite
reports corruption at startup, Bottie opens a restricted recovery screen instead of mutating the damaged store. Users
can restore the newest verified automatic snapshot or choose a manual backup; Rust preserves the damaged database
bundle in app-private storage before replacement.
Remote API keys stay in the operating-system credential vault and are never returned to the WebView. On macOS,
Touch ID gates the first read of each saved cloud credential
per Bottie session; successful unlocks are cached only in process memory. The native attachment picker now streams up
to eight selected files into application-private, SHA-256-addressed storage with a 25 MiB per-file ceiling,
content-based MIME detection, safe display names, and duplicate reuse. The WebView receives no filesystem path,
labels each retained item as local-only, and can atomically associate the current selection with the submitted user
message. Associations reopen on the selected branch, survive edit/regenerate forks, and can be removed without deleting
the retained content blob. Plain-text, Markdown, PDF, and DOCX text up to 2 MiB is extracted into durable native-only
state. PDF parsing is additionally limited to 500 pages and 8 MiB of decompressed content per page. DOCX parsing
validates the package manifest and bounds archive entries, total declared expansion, main-document XML, XML events,
and XML depth. JPEG and PNG images are decoded within dimension, pixel, allocation, and output ceilings, have EXIF
orientation applied to their pixels, and are re-encoded without source metadata into application-private,
content-addressed derivatives. Ingestion commits durable pending state and returns before document parsing or image
decoding; one native background worker resumes pending work after startup, selection, or restore. The WebView receives
path-free live extraction or normalization updates, dimensions, counts, and sizes but never extracted text, derivative
identities, bytes, or paths. Only prompt text reaches the provider:
attachment delivery, other office formats, memory
retrieval, provider tool loops, approvals, and tool execution are not implemented yet; those surfaces remain disabled
or explicitly labelled.

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

With oMLX or Ollama running on its default loopback port, the native app discovers available models automatically. Provider and model use separate selectors; changing providers refreshes that provider's models, and the last successful pair is restored after restart. Settings can change either endpoint and test it before saving. Rust rejects non-loopback hosts, embedded credentials, paths, query strings, and fragments; redirects are disabled, and no HTTP capability is exposed to the WebView. Ollama discovery also normalizes model capabilities, context size, and loaded/on-demand state.

Settings also support HTTPS OpenAI-compatible and Anthropic-compatible profiles. API keys are written and removed
through narrow Rust commands backed by the OS credential vault. Cloud routes are visibly labelled before sending,
redirects stay disabled, and remote response usage and provider-reported cost metadata are preserved when available.

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
reasoning blocks, and returns at most 50 activity-ordered results with bounded snippets. It intentionally uses the
existing store rather than adding FTS5 before the later memory-search milestone. Assistant answers pass through a
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
dialog. JSON uses version 2 of the `bottie-conversation` contract and retains ordered text, separate reasoning, message
state, provider/model provenance, local rating, creation time, and provider-reported generation metadata without
opaque storage identifiers. Retained tool arguments/results are included without native run or provider call IDs. A
separate global JSON action writes every active and archived conversation's selected lineage through version 2 of the
`bottie-conversation-batch` contract while excluding Trash and hidden branch siblings.
Rust writes each UTF-8 file; the WebView receives only a saved/cancelled outcome and leaf filename, never the chosen
directory. A separate global backup action asks Rust to create and verify a complete SQLite
snapshot with SQLite's online backup API, including
committed WAL content, while likewise returning no local path. Restore opens and validates the selected database in
Rust, migrates an isolated staging copy when supported, creates an application-private snapshot of the current store,
and only then replaces the live database. The WebView receives leaf filenames rather than database paths. Native
startup rotation creates and verifies a new app-private snapshot when the newest is at least 24 hours old, retains the
seven newest managed snapshots, and reports its path-redacted result in Recent diagnostics. Before normal store
initialization, Rust opens existing data read-only and classifies explicit SQLite corruption or a non-`ok` integrity
result. Recovery mode blocks normal conversation connections and skips automatic rotation. Its WebView status contains
only verified automatic-snapshot count and newest timestamp. Restoring either that newest snapshot or a manually
selected backup stages, migrates, and verifies a replacement before preserving the damaged main database and present
WAL/shared-memory sidecars in app-private storage. Successful replacement resumes normal conversation access without
returning a filesystem path. A separate native picker ingests files through a bounded streaming copy into an
application-private attachment directory. SQLite stores only sanitized metadata and the SHA-256 content identity;
identical content reuses its existing blob across restarts. Ingestion leaves extraction and supported image
normalization durably pending, then wakes a single process-lifetime worker instead of parsing or decoding inside the
picker command. The worker handles one item at a time, resumes pending rows after startup or restore, and emits only
path-free terminal metadata to update visible draft and message attachments. Store replacement pauses the worker after
its current bounded item and resumes it afterward. Sending commits up to eight selected attachment identities
with the user message, then clears the draft. Reopened selected lineages reconstruct ordered path-free metadata; edited
and regenerated request branches inherit the source request's associations. Detaching is limited to visible user
messages while generation is idle and retains the catalog row and blob for deduplication. Attachment bytes remain
outside SQLite-only backups and exports. Schema-version-13 records retain up to 2 MiB of UTF-8 plain-text,
Markdown, derived PDF text, or derived DOCX text inside SQLite, resume pending work after migration or interruption,
and expose only path-free state to the interface. PDF extraction retains a page count, refuses files over 500 pages,
bounds each decompressed page stream to 8 MiB, and reports encrypted, malformed, text-free, extraction, and size
failures without parser details or paths. DOCX extraction recognizes the package by its manifest rather than its
filename, reads only bounded ZIP members in memory, rejects overlapping/encrypted/over-complex archives, and bounds
WordprocessingML size, events, depth, and output. JPEG and PNG normalization accepts at most 8,192 pixels on either
axis, 16 million total pixels, 128 MiB of decoded image allocation, and 25 MiB of encoded output. JPEG EXIF orientation
is applied before both formats are re-encoded through metadata-free encoders into application-private,
content-addressed derivatives. SQLite backups include extracted text and path-free derivative metadata, but original
blobs and normalized derivatives remain outside those backups. Extracted text and images remain absent from
conversation exports and provider requests. Other office formats are not extracted; no attachment is indexed or
delivered to a provider.

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

For the planned memory subsystem, bottie will use FastEmbed inside Rust with the quantized EmbeddingGemma 300M model as its single built-in embedding default. The application will own model download/cache UX and embedding-version metadata; users will not need to configure a second inference provider merely to enable local memory search.
