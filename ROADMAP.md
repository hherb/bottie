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

- native Ollama discovery and chat adapter;
- capability mapping for tools, vision, embeddings, and context size;
- explicit model loading/unloading state where available;
- parity tests against the normalized stream contract.

### 1.3 Provider configuration

- provider settings UI;
- endpoint validation and connection testing;
- model picker populated from actual discovery;
- per-conversation provider/model choice;
- sensible timeouts and reconnect behavior;
- diagnostic logging with secret redaction.

### 1.4 Remote OpenAI and Anthropic APIs

- native adapters instead of forcing both APIs through one request shape;
- generic OpenAI-compatible and Anthropic-compatible endpoint profiles;
- API keys stored in the operating system credential vault;
- explicit local/cloud routing indicator before sending;
- usage and cost metadata where providers return it.

## Milestone 2 — Durable conversations

Outcome: conversations survive restart and can be searched, branched, exported, and recovered.

### Design gate

Decide whether “per user” means local profiles inside one OS account or authenticated multi-user access. For an ordinary desktop application, local profiles are the simpler default. Make this decision before finalizing ownership and key-management columns.

### Storage foundation

- bundled SQLite owned exclusively by Rust;
- ordered, transactional migrations;
- WAL mode, foreign keys, integrity checks, and backup strategy;
- profiles, conversations, branches, messages, and content blocks;
- append-oriented provider runs, tool invocations, and usage records;
- draft/partial/final/cancelled message states;
- soft deletion and retention metadata.

### Conversation experience

- create, rename, archive, delete, and restore conversations;
- edit-and-regenerate branches using parent message IDs;
- reopen the exact conversation after restart;
- conversation search and date grouping based on real data;
- Markdown rendering with sanitization;
- copy, retry, rating, and export actions;
- JSON or Markdown export and backup restore.

## Milestone 3 — Attachments and multimodal context

Outcome: users can safely add images and documents and see exactly what is sent to a model.

- content-addressed attachment storage in the application data directory;
- MIME sniffing, hashes, size limits, duplicate detection, and safe filenames;
- extraction pipeline for plain text, Markdown, PDF, and selected office formats;
- image normalization and metadata removal policy;
- background extraction and indexing states;
- capability-aware image delivery to vision models;
- clear behavior for text-only models;
- per-message and per-conversation attachment scope;
- attachment removal, garbage collection, and export behavior;
- previews and extraction-error UX.

Raw local filesystem paths must never be forwarded to a provider.

## Milestone 4 — Persistent memory

Outcome: models can retrieve relevant past conversations with visible provenance and user control.

### Search foundation

- SQLite FTS5 index and BM25 lexical search;
- statically linked `sqlite-vec` semantic index;
- chunking for messages and extracted documents;
- embedding provider abstraction;
- embedding model, dimensions, chunking version, and index-generation metadata;
- resumable background indexing and complete reindex support;
- reciprocal-rank fusion of lexical and vector results;
- profile, source, conversation, and date filters.

### Memory tools

- `search_memory` for ranked excerpts;
- `open_memory` for surrounding turns and provenance;
- `search_attached_files` for indexed document chunks;
- structured tool results with conversation/message identifiers;
- visible and removable memory citations in the context panel;
- exclude-from-memory, forget, retention, and reindex controls.

Do not silently inject arbitrary long-term memories into every prompt. Recent conversation context may be automatic; long-term recall should be explicit and inspectable.

## Milestone 5 — Tool runtime and internet access

Outcome: bottie can use host-managed tools consistently across providers.

### Tool orchestration

- provider-independent tool definitions and execution loop;
- JSON-schema argument validation;
- recursion, call-count, output-size, and timeout limits;
- cancellation propagation;
- safe versus approval-required tool policy;
- structured audit records and expandable activity UI;
- protection against malformed or unsupported tool calls.

### Web tools

- pluggable search provider interface;
- `web_search` with freshness and domain filters;
- `web_fetch` with redirects, size limits, content-type checks, and timeouts;
- extraction to inert text with source URL, title, and publication metadata;
- citations connected to claims and retained with the conversation;
- prompt-injection labeling for untrusted retrieved content;
- domain and network policy controls.

MCP interoperability can follow after bottie's own tool contract and policy model are stable.

## Milestone 6 — Reliability, privacy, and desktop beta

Outcome: bottie is safe and comfortable enough for sustained daily use.

- first-run provider and privacy setup;
- database backup, restore, migration rollback planning, and corruption recovery;
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
