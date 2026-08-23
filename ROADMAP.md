# bottie roadmap

This roadmap favors complete vertical slices over building every backend subsystem in isolation. Each milestone should leave the desktop application usable and preserve the Rust/WebView security boundary.

## Product principles

- **Local first, not local only.** Local inference and storage are first-class, while cloud providers remain explicit choices.
- **Visible context.** Users can inspect and remove attachments, memories, web sources, and tool results supplied to a model.
- **Provider capability awareness.** Vision, tools, embeddings, structured output, reasoning, and context limits are detected rather than assumed.
- **Rust owns trust.** Secrets, database access, filesystem access, provider traffic, and tools stay behind narrow Tauri commands.
- **Durable provenance.** Stored messages retain model, provider, source, tool, and branching metadata.
- **Calm transparency.** Activity is visible and auditable without turning normal conversation into a developer console.
- **Accessible motion and interaction.** Keyboard operation, contrast, reduced motion, and responsive layouts remain release requirements.

## Milestone 0 — Product shell

Status: complete

- Tauri 2, Rust, SvelteKit, and TypeScript scaffold
- bottie visual language and responsive three-pane layout
- conversation, composer, attachment, memory, model, and privacy surfaces
- simulated activity and token streaming
- working simulated cancellation
- typed `app_info` Rust command
- minimal Tauri permissions and CSP
- compile, build, native-launch, and interaction verification

## Milestone 1 — Real inference

Outcome: bottie can conduct reliable text conversations through real local and remote providers.

### 1.1 Provider-neutral stream and oMLX

Status: complete

- [x] normalized provider capabilities, content blocks, requests, events, usage, and errors;
- [x] Rust-owned oMLX model discovery;
- [x] streaming chat completions through a typed Tauri IPC channel;
- [x] end-to-end cancellation;
- [x] offline, malformed-response, timeout, and retry UX;
- [x] fixture-driven protocol tests and opt-in live oMLX tests.

See `HANDOVER.md` for verification evidence.

### 1.2 Ollama

Status: complete

- [x] native Ollama discovery and chat adapter;
- [x] capability mapping for tools, vision, embeddings, and context size;
- [x] explicit loaded/on-demand model state from the running-model API;
- [x] fixture and opt-in live parity tests against the normalized stream contract.

### 1.3 Provider configuration

Status: complete

- [x] provider settings UI;
- [x] persisted Rust-owned local endpoint settings;
- [x] loopback-only endpoint validation and connection testing;
- [x] model picker populated from actual discovery;
- [x] separate provider/model selectors with provider-specific model refresh;
- [x] last successful provider/model selection remembered across restart;
- [x] provider-qualified model choice applied to the active conversation and recorded on responses;
- [x] sensible connect, discovery, stream-idle, and reconnect behavior;
- [x] structured in-memory diagnostic logging with secret redaction.

The native provider/model selection, provider-specific refresh, and remembered-selection experience were manually reviewed and confirmed on 2026-08-18; see `HANDOVER.md`.

### 1.3a Bounded reasoning controls

Status: complete

- [x] explicit per-request reasoning control with off as the safe default and low as the enabled level;
- [x] native oMLX and Ollama request mapping instead of model-name heuristics in the WebView;
- [x] separate normalized reasoning deltas across the Rust/Tauri boundary;
- [x] collapsed, user-expandable reasoning presentation kept separate from answer text;
- [x] a 4,096-token default completion ceiling to prevent unbounded provider defaults;
- [x] fixture coverage for request mapping and stream decoding plus a bounded live oMLX check.

### 1.4 Remote OpenAI and Anthropic APIs

Status: complete

- [x] native adapters instead of forcing both APIs through one request shape;
- [x] generic OpenAI-compatible and Anthropic-compatible endpoint profiles;
- [x] API keys stored in the operating system credential vault, with Touch ID session unlock on macOS;
- [x] explicit local/cloud routing indicator before sending;
- [x] usage and cost metadata where providers return it.

## Milestone 2 — Durable conversations

Outcome: conversations survive restart and can be searched, branched, exported, and recovered.

Status: complete

### Design gate

Decision: Bottie starts with one built-in local profile scoped to the current OS account. The schema retains explicit
profile ownership so optional local profiles can be added later without application authentication or a conversation
ownership rewrite.

### Storage foundation

- [x] bundled SQLite owned exclusively by Rust;
- [x] ordered, transactional migrations with schema-version history;
- [x] WAL mode, foreign keys, busy handling, and startup integrity checks;
- [x] built-in local profile, conversations, main branches, ordered messages, and text/reasoning blocks;
- [x] append-oriented provider runs and provider-reported usage records;
- [x] append-oriented tool invocations and results;
- [x] crash-safe draft/partial message persistence and interrupted-run recovery;
- [x] manual SQLite backup creation through a Rust-owned Save dialog and SQLite's online backup API;
- [x] confirmed manual restore from a validated Bottie backup with a pre-restore safety copy;
- [x] automatic backup rotation with one snapshot per 24 hours and seven-snapshot retention;
- [x] corruption detection and guided automatic/manual recovery with damaged-store preservation;
- [x] soft deletion and retention metadata.

### Conversation experience

- [x] create conversations on first send, list recent conversations, and reopen stored messages after restart;
- [x] rename, archive, delete, and restore conversations;
- [x] edit-and-regenerate branches using parent message IDs;
- [x] reopen the exact conversation after restart;
- [x] date grouping based on real conversation activity;
- [x] bounded native conversation search across titles and visible message text;
- [x] Markdown rendering with sanitization;
- [x] assistant-response and reasoning copying as labelled Markdown with accessible outcome feedback;
- [x] response retry for interrupted, cancelled, and transiently failed attempts on preserved branches;
- [x] durable Good/Poor response rating with replacement, clearing, and preserved-branch restoration;
- [x] single-conversation Markdown export through a Rust-owned native Save dialog;
- [x] single-conversation JSON export through the same selected-lineage and native Save policy;
- [x] batch JSON export of active and archived selected lineages through a Rust-owned native Save dialog.

## Milestone 3 — Attachments and multimodal context

Outcome: users can safely add images and documents and see exactly what is sent to a model.

Status: complete

- [x] content-addressed attachment storage in the application data directory;
- [x] MIME sniffing, hashes, size limits, duplicate detection, and safe display names;
- [x] bounded native extraction state and UTF-8 source retention for plain text and Markdown;
- [x] bounded page-aware PDF text extraction with path-free failure states;
- [x] bounded DOCX extraction as the first selected office format;
- [x] bounded JPEG/PNG normalization, EXIF orientation application, and metadata removal policy;
- [x] durable background extraction and image-normalization lifecycle;
- [x] durable background indexing-readiness states for extracted attachment text;
- [x] capability-aware image delivery to vision models;
- [x] clear behavior for text-only models;
- [x] per-message association with selected-branch reopen, fork inheritance, and association removal;
- [x] conversation-level attachment scope;
- [x] portable attachment backup and export behavior;
- [x] restart-boundary attachment garbage collection;
- [x] bounded ready-image previews and explicit extraction/normalization-error UX.

Raw local filesystem paths must never be forwarded to a provider.

## Milestone 4 — Persistent memory

Outcome: models can retrieve relevant past conversations with visible provenance and user control.

Status: complete

### Search foundation

- [x] native SQLite FTS5 whole-source index and bounded BM25 lexical search with built-in-profile enforcement plus
  source, conversation, and date filters;
- [x] versioned deterministic Unicode-safe chunk catalog for final message answers and ready extracted documents;
- [x] statically linked `sqlite-vec` semantic index;
- [x] Rust-owned FastEmbed runtime using Q4 EmbeddingGemma 300M as the single built-in embedding model;
- [x] application-owned model cache progress and versioned embedding metadata, without an embedding-provider picker;
- [x] embedding model, dimensions, chunking version, input contract, and index-generation metadata;
- [x] resumable bounded background indexing;
- [x] bounded current-generation semantic KNN retrieval with EmbeddingGemma query prompting and lifecycle-safe
  profile, source, conversation, association, and date filters;
- [x] explicit derived-only reindex control with durable path-free progress and restore-safe worker coordination;
- [x] bounded source-level reciprocal-rank fusion of lexical and vector results under one shared filter contract;

### Memory tools

- [x] Rust-owned `search_memory` contract for ranked message excerpts with bounded path-free provenance;
- [x] `open_memory` for bounded surrounding final turns on the matched message's immutable branch lineage;
- [x] `search_attached_files` for indexed document chunks with bounded path-free file provenance;
- [x] structured `search_memory` results with conversation/message identifiers and optional exact chunk offsets;
- [x] provider-independent definitions and strict closed argument schemas for the three native memory tools;
- [x] provider-neutral single-call dispatch with a bounded structured result/error envelope;
- [x] explicit Ollama native memory-tool definition/call/result mapping and bounded generation-loop integration;
- [x] explicit OpenAI Chat Completions definition/call/result mapping and bounded generation-loop integration;
- [x] explicit Anthropic Messages definition/call/result mapping and bounded generation-loop integration;
- [x] visible and removable memory citations in the context panel;
- [x] reversible per-conversation exclude-from-memory control enforced across native indexes and memory tools;
- [x] explicit per-conversation forget from Trash with documented source, derived-data, attachment, and backup policy;
- [x] opt-in 30-day, 90-day, or one-year Trash retention with manual retention as the default and healthy-startup
  enforcement through the explicit-forget data policy (explicit derived-only reindex is complete).

Do not silently inject arbitrary long-term memories into every prompt. Recent conversation context may be automatic; long-term recall should be explicit and inspectable.

## Milestone 5 — Tool runtime and internet access

Outcome: bottie can use host-managed tools consistently across providers.

### Tool orchestration

- [x] provider-independent definitions for the native memory tool set;
- [x] provider-neutral single-call execution dispatcher with a 64 KiB serialized output ceiling;
- [x] provider-independent multi-call execution state machine;
- [x] strict JSON-schema argument validation for the native memory tool set;
- [x] recursion, call-count, aggregate-output, and overall-deadline limits;
- [x] provider-neutral cancellation signal with checks before and after every native call;
- [x] Ollama native definition/call/result mapping and generation-loop wiring;
- [x] OpenAI Chat Completions definition/call/result mapping and generation-loop wiring;
- [x] Anthropic Messages definition/call/result mapping and generation-loop wiring;
- [x] active-generation cancellation propagation through mapped providers and native tool work;
- [x] safe versus approval-required tool policy;
- [x] structured audit records and expandable activity UI;
- [x] protection against malformed or unsupported tool calls.

### Web tools

- [x] pluggable native search-provider interface with bounded fixed-endpoint Brave Search and Exa Search adapters;
- [x] independent native search-engine credentials, fixed-route connection tests, and a saved active-engine choice;
- [x] provider-independent `web_search` contract and native dispatcher with freshness and domain filters;
- [x] explicit Ollama definition/call/result mapping and generation-loop integration for `web_search`;
- [x] explicit OpenAI Chat Completions definition/call/result mapping and generation-loop integration for
  `web_search`;
- [x] explicit Anthropic Messages definition/call/result mapping and generation-loop integration for `web_search`;
- [x] provider-independent `web_fetch` contract and native public-network client/dispatcher with explicit redirects,
  DNS/address pinning, size limits, UTF-8 content-type checks, and one shared timeout;
- [x] explicit Ollama generation-loop mapping for `web_fetch`;
- [x] explicit OpenAI-compatible generation-loop mapping for `web_fetch`;
- [x] explicit Anthropic-compatible generation-loop mapping for `web_fetch`;
- [x] extraction to inert text with source URL, title, and publication metadata;
- [x] removable path-free Context-panel source cards derived from successful selected-lineage Web tool results;
- [x] citations connected to claims and retained with the conversation;
- [x] prompt-injection labeling for explicitly untrusted fetched-page content in source cards and durable tool audit;
- user-configurable domain and network policy controls.

MCP interoperability can follow after bottie's own tool contract and policy model are stable.

## Milestone 6 — Reliability, privacy, and desktop beta

Outcome: bottie is safe and comfortable enough for sustained daily use.

- first-run provider and privacy setup;
- automatic backup rotation and corruption recovery (complete), plus migration rollback planning;
- crash-safe partial messages and interrupted indexing recovery;
- structured local diagnostics with redaction and opt-in export;
- CSP and Tauri capability review;
- secret-vault and filesystem-boundary tests;
- dependency and license review;
- keyboard shortcuts and command palette;
- themes, density options, and refined empty/offline/error states;
- accessibility audit and reduced-motion verification;
- performance tests for long conversations and large histories;
- macOS, Windows, and Linux packaging and smoke tests;
- custom bottie application icon, signing, updates, and release notes.

## Milestone 7 — Local voice conversations

Outcome: users can hold interruptible, private voice conversations without requiring a cloud speech service.

- local audio capture with explicit microphone permission;
- voice activity detection;
- local streaming speech-to-text;
- transcript correction and visible turn boundaries;
- local text-to-speech with selectable voices;
- barge-in and end-to-end cancellation;
- audio content blocks and optional local audio retention;
- latency, device selection, and acoustic feedback controls;
- full text fallback and accessibility support.

Voice should reuse the same provider, content-block, event, persistence, and cancellation models established in earlier milestones.

## Cross-cutting definition of done

Every implementation slice should include, in proportion to its risk:

- unit tests for protocol, storage, parsing, and policy code;
- frontend type and accessibility checks;
- fixture tests for provider-specific behavior;
- migration tests once persistence exists;
- explicit cancellation and error-path coverage;
- secret-redaction and trust-boundary review;
- native manual verification of the meaningful user flow;
- updated `README.md`, `HANDOVER.md`, and roadmap status when behavior changes;
- clean results from the standard verification commands.

## Release-shaped checkpoints

### Developer preview

Current product shell plus a real oMLX text conversation.

### Local alpha

oMLX and Ollama, persistent conversations, branching, and reliable restart behavior.

### Memory alpha

Attachments, hybrid conversation search, embeddings, and inspectable memory tools.

### Connected beta

Remote OpenAI/Anthropic-compatible providers, secure credentials, web tools, citations, and import/export.

### Desktop 1.0

Recovery, privacy controls, accessibility, packaging, signing, updates, and multi-platform verification complete. Voice remains optional for a later release.
