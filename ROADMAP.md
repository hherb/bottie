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
- [x] API keys stored in the operating system credential vault, with one Touch ID unlock at macOS app start and
  process-memory caching for the session;
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
- [x] Rust-owned zero-argument UTC `current_time` tool with closed schema, safe policy, durable audit, and mappings for
  every explicitly tool-capable route;
- [x] bounded oMLX endpoint-capability discovery plus native clock, Memory, and Web tool-loop mapping without provider
  MCP or arbitrary server-tool execution;
- [x] current Anthropic Models API structured-capability decoding with legacy compatible-response preservation;
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
- [x] user-configurable HTTPS, allowlisted-domain, and blocklisted-domain policy applied to Web search results,
  fetches, and every redirect without weakening the fixed public-address baseline.

MCP interoperability can follow after bottie's own tool contract and policy model are stable.

### Sandboxed Python execution

Status: core feasibility, macOS/Windows/Linux native containment proofs, opt-in development-bundle runner injection,
durable provider-neutral audit, all four mapped-provider integrations, and selected-lineage result presentation
complete; shipping evidence remains pending.

- [x] standalone Rust helper with a bounded stdin/stdout JSON contract and no shell interpolation;
- [x] CPython/WASI execution through Wasmtime's interpreter-only Pulley target;
- [x] read-only explicit mounts, isolated environment, denied networking/subprocesses, and bounded random requests;
- [x] fixed wall-time, linear-memory, table, stdout, and stderr ceilings with path-free result classifications;
- [x] opt-in runtime denial tests using one checksum-pinned development runtime;
- [x] transient, separately signed macOS App-Sandboxed XPC proof with private-pipe execution, cancellation,
  kill-on-client-exit, exact nested entitlements/signatures, and direct host-fixture denial;
- [x] transient Windows zero-capability AppContainer proof with a restricted token, private pipes, one-process
  memory/CPU Job Object limits, cancellation, kill-on-controller-close, and direct host-fixture denial;
- [x] built-in Linux Landlock/seccomp/rlimits containment with private pipes, cancellation, parent-close cleanup, and
  host-fixture denial, without requiring Bubblewrap or Flatpak;
- [x] reproducible official-source CPython/WASI build provenance plus exact unsigned development bundling, licence,
  dependency inventory, and cross-platform package inspection;
- [x] credential-free installed Linux development-DEB identity, Landlock/seccomp denial, private-pipe execution,
  cancellation, and parent-exit cleanup smoke;
- [x] credential-free installed Windows development-MSI controller/helper/runtime identity, AppContainer token and
  host-fixture denial, private-pipe execution, cancellation, and controller-exit cleanup smoke;
- [ ] exact shipping-helper/runtime containment, signing, release-candidate binding, and installed-package inspection;
- [x] approval-required native Python tool contract and user-visible inert source/purpose review without helper launch
  or provider advertisement;
- [x] process-local one-use approve/deny decisions bound to the unchanged complete call, with an opaque-token modal and
  no provider identity exposed to the WebView;
- [x] provider-neutral approval wait/resume for one exact call, with denial, shared cancellation, and aborted-waiter
  cleanup as terminal non-execution paths;
- [x] bounded generation-time approval event publication with startup-race-safe WebView subscription and cancellation
  removal;
- [x] provider-neutral exact-grant execution orchestration, bounded private-pipe helper protocol, and Linux's built-in
  Landlock/seccomp/rlimit launch path without provider mapping;
- [x] macOS XPC product transport behind the provider-neutral runner interface, with bounded private pipes, shared
  cancellation, connection-invalidation cleanup, and fixed opt-in development-bundle resolution;
- [x] Windows AppContainer product transport behind the provider-neutral runner interface, with a process-scoped
  profile, bounded private pipes, shared cancellation, controller-close Job Object cleanup, and fixed opt-in
  development-bundle resolution plus owned profile provisioning/cleanup;
- [x] fail-closed Tauri injection of marked platform resources while default and protected package configs remain
  unchanged;
- [x] append-only durable Python invocation, approve/deny decision, and bounded terminal-outcome audit through a
  provider-neutral orchestration seam, with approval committed before helper launch;
- [x] explicit tool-capable oMLX mapping through approval, contained execution, bounded provider reuse, and durable
  audit;
- [x] explicit tool-capable Ollama mapping through the same approval, containment, audit, cancellation, bounded-result,
  and ordered provider-correlation boundaries;
- [x] explicit tool-capable OpenAI-compatible mapping through the same approval, containment, audit, cancellation,
  bounded-result, and exact Chat Completions call-identity boundaries;
- [x] explicit tool-capable Anthropic-compatible mapping through the same approval, containment, audit, cancellation,
  bounded-result, preserved thinking-block, and exact Messages `tool_use`/`tool_result` identity boundaries;
- [x] selected-lineage answer presentation that labels approved source, bounded stdout/stderr, stable errors, helper
  outcome/duration, and contained-runtime execution provenance from the durable path-free audit.

See `docs/python-sandbox.md` for the verified boundary, platform options, and exclusions.

## Milestone 6 — Reliability, privacy, and desktop beta

Outcome: bottie is safe and comfortable enough for sustained daily use.

- [x] first-run provider and privacy setup;
- [x] automatic backup rotation, corruption recovery, and staged migration promotion rollback;
- [x] crash-safe partial messages and interrupted indexing recovery;
- [x] structured local diagnostics with redaction and opt-in export;
- [x] first-party Localmail HTTPS origin, explicit certificate trust, bounded connection testing, and vault-held bearer
  authentication foundation;
- [x] first-party Localmail `search_email` contract with closed filters, explicit current `sort`/`sort_order` mapping,
  newest-first date order by default, one pinned authenticated search call, bounded inert path-free summaries, and no
  email-body or attachment-content exposure;
- [x] first-party Localmail `open_email` contract over exact search-result identities with one pinned authenticated
  detail call, external images disabled, bounded inert header/body text, and no HTML or attachment-byte exposure;
- [x] first-party Localmail extracted attachment-text reading through exact message-local attachment numbers, with
  hashes resolved only in Rust, one fixed pinned `/text` request, bounded untrusted text, and no raw-byte fallback;
- [x] provider-independent closed Localmail tool definitions, strict raw conversion into the existing connector
  requests, safe read-only policy entries, and bounded redacted dispatch;
- [x] remembered Memory, Web, and Email preferences that restore only when current provider/model capability and
  connector-readiness gates permit them;
- [x] explicitly tool-capable Ollama Email enablement with configured native trust and credential gating, bounded
  multi-round execution, durable audit, and explicit loopback/Localmail disclosure;
- [x] explicit OpenAI-compatible Email mapping with exact Chat Completions call/result correlation, the same configured
  native trust and loop bounds, and cloud-provider/Localmail delivery disclosure;
- [x] explicit Anthropic-compatible Email mapping with exact Messages `tool_use`/`tool_result` block correlation,
  preserved thinking state, the same configured native trust and loop bounds, and cloud-provider/Localmail delivery
  disclosure;
- [x] explicit oMLX Email mapping through its discovered Chat Completions tool route, with exact call/result
  correlation, the same configured native trust and loop bounds, and loopback-provider/Localmail delivery disclosure;
- [x] CSP and Tauri capability review;
- [x] secret-vault and filesystem-boundary tests;
- [x] dependency and licence review with locked macOS/Windows/Linux Rust plus npm graphs, generated distributable
  notices, pinned native/model runtime assets, and explicit release gates;
- [x] keyboard shortcuts and command palette;
- [x] themes and density options;
- [x] refined empty/offline/error states;
- [x] accessibility audit and reduced-motion verification;
- [x] performance tests for long conversations and large histories;
- [x] macOS packaging and smoke test;
- [x] current Windows 0.9.0 packaging, inspection, and isolated smoke evidence;
- [x] current Linux 0.9.0 packaging, inspection, and isolated smoke evidence;
- [x] custom bottie application icon with deterministic WebView and platform package assets;
- [x] credential-free macOS Developer ID signing, hardened-runtime, notarization, stapling, and Gatekeeper contract,
  plus current 0.9.0 host evidence;
- [x] protected manual Windows Authenticode contract for an independently signed, timestamped, and verified MSI plus
  installed executable, with identity-free package and isolated-smoke evidence; retained only as an unconfigured
  direct-download alternative;
- [x] credential-free, identity-parameterized Microsoft Store x64 MSIX packaging, inspection, and manual Windows App
  Certification Kit workflow contract;
- [x] Individual Microsoft Store developer registration and Bottie product-name/identity reservation;
- [x] current Windows Store MSIX runner build, independent inspection, and Windows App Certification Kit pass;
- [x] exact reviewed Microsoft Store package submitted for certification;
- [ ] Microsoft Store certification and publication (deferred after rejection until further release-owner notice);
- [x] protected manual Linux embedded-OpenPGP signing contract with a published public certificate and independent
  canonical-payload, policy, and keyring verification;
- [x] current credentialed Linux distribution-signature evidence;
- [x] versioned 0.9.0 beta release notes plus a deterministic path-free release-candidate gate manifest;
- [x] credential-free signed-update manifest and path-free publication-evidence contract;
- [x] recoverably backed-up production updater trust key plus Rust-owned, user-controlled update checks and installs;
- [x] native final-byte minisign verification plus current protected Linux x64 updater-artifact evidence;
- [x] protected current-main three-platform GitHub updater-publication workflow with exact intent, legal, artifact,
  draft, digest, latest-full-release, and path-free evidence gates;
- [ ] current protected macOS and Windows updater-artifact evidence (platform credentials remain unconfigured);
- [ ] signed updater release publication outside the Store.

## Milestone 7 — Local voice conversations

Outcome: users can hold interruptible, private voice conversations without requiring a cloud speech service.

Status: complete; evidence-gated acoustic feedback processing remains deferred.

- [x] Rust-owned default-input capture behind an explicit Record voice action, with operating-system permission,
  bounded session-only PCM retention, path-free status, Stop/Discard controls, and no provider delivery;
- [x] bounded native voice activity detection with path-free speech/silence timing and calm live/captured state;
- [x] local streaming speech-to-text with a pinned multilingual Whisper tiny Q5 model, bounded partial/final transcript
  ranges, visible timing, app-owned cache verification, and session-only audio/text state;
- [x] session-only transcript correction and visible numbered turn boundaries;
- [x] bounded local text-to-speech with explicit assistant-response playback, a Settings-owned durable system-voice
  choice, opaque native identities, unavailable-choice fallback, and no generated-audio retention;
- [x] explicit barge-in that stops Bottie's local playback, cancels active provider/tool work through the existing
  durable cancellation boundary, and serializes new generation registration against native capture;
- [x] provider-neutral native-only audio content blocks plus separate off-by-default provider delivery and app-private
  WAV retention choices;
- [x] bounded session-only native latency status for input readiness, first/final local transcript availability, and
  local speech-engine acceptance, labelled by observable endpoints rather than acoustic claims;
- [x] explicit durable input-device selection with bounded display labels, process-local public tokens, Rust-owned
  stable opaque preference keys, startup fallback to System default when unavailable, exact-device resolution at Record,
  and fail-closed playback/capture separation;
- [x] explicit transcript-to-text fallback that copies only current visible final turns into a bounded editable unsent
  draft, preserves existing text, keeps capture state intact, and provides keyboard, focus, and screen-reader feedback;
- [x] deterministic same-input repeated capture after a stopped worker's final unwind, with active capture overlap
  still rejected and native macOS regression acceptance;
- [ ] acoustic echo cancellation or general system-feedback processing, if later evidence justifies native DSP;

Voice should reuse the same provider, content-block, event, persistence, and cancellation models established in
earlier milestones.

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
