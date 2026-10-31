# bottie roadmap

This roadmap favors complete vertical slices over building every backend subsystem in isolation. Each milestone should leave the desktop application usable and preserve the Rust/WebView security boundary.

## Current focus (2026-10-31)

- `FIX_BOTTIE.md` audit list: all items done except 9 (Linux check), 10 (DGX hash lock), 14 (runtime link check), and
  19 (low priority). Then run the follow-up audit it lists. Review follow-ups up to #228 are merged; #230 and #231
  are fixed on `fix/issues-230-231`. #226's last bullet (spawners outside `SPAWN_LOCK`) waits until it is seen in
  practice.
- `mlx_worker.py` changed (audit item 4). The frozen Mac Qwen-Image-2512 worker bundle (8.4) and the Linux clean-runtime
  image and closure evidence (8.4) predate it, so rebuild and re-pin both before relying on them.

## Product principles

- **Local first, not local only.** Local inference and storage are first-class, while cloud providers remain explicit choices.
- **Visible context.** Users can inspect and remove attachments, memories, web sources, and tool results supplied to a model.
- **Provider capability awareness.** Vision, tools, embeddings, structured output, reasoning, and context limits are detected rather than assumed.
- **Rust owns trust.** Secrets, database access, filesystem access, provider traffic, and tools stay behind narrow Tauri commands.
- **Durable provenance.** Stored messages retain model, provider, source, tool, and branching metadata.
- **Calm transparency.** Activity is visible and auditable without turning normal conversation into a developer console.
- **Accessible motion and interaction.** Keyboard operation, contrast, reduced motion, and responsive layouts remain release requirements.

## Completed milestones 0-4

Item-level checklists are in git history (before 2026-10-30) and the linked `docs/`.

- **0 — Product shell:** Tauri 2, Rust, SvelteKit, and TypeScript scaffold; three-pane layout; typed IPC; minimal
  permissions and CSP.
- **1 — Real inference:** provider-neutral stream, event, usage, and error contract; native oMLX, Ollama,
  OpenAI-compatible, and Anthropic-compatible adapters; capability detection; end-to-end cancellation; bounded reasoning
  controls; keychain-held API keys with one Touch ID unlock; local/cloud routing indicator; usage and cost metadata.
- **2 — Durable conversations:** one built-in local profile (the schema keeps profile ownership so local profiles can
  be added later); Rust-owned SQLite with ordered migrations, WAL, and integrity checks; crash-safe partials; manual and
  automatic backup, restore, and guided recovery; rename, archive, Trash, branches, search, retry, ratings, and
  Markdown/JSON/batch export.
- **3 — Attachments and multimodal context:** content-addressed storage; MIME sniffing, hashing, and size limits; text,
  Markdown, PDF, and DOCX extraction; JPEG/PNG normalization; capability-aware image delivery; per-message and
  conversation scope; portable backup and export; restart-boundary garbage collection. Raw local paths never reach a
  provider.
- **4 — Persistent memory:** FTS5 lexical and `sqlite-vec` semantic indexes with EmbeddingGemma (FastEmbed),
  resumable indexing, reciprocal-rank fusion, and the `search_memory`, `open_memory`, and `search_attached_files` tools
  across all providers; visible citations; per-conversation exclusion, forget, and Trash retention. Long-term recall
  stays explicit and inspectable, never silently injected.

## Milestone 5 — Tool runtime and internet access

Outcome: bottie can use host-managed tools consistently across providers.

### Tool orchestration and web tools

Status: complete (checklists in git history before 2026-10-30).

- Provider-neutral tool contract, strict argument validation, multi-call state machine, and recursion, call, output, and
  deadline limits (Settings default: 12 rounds, 24 calls, 8,192 output tokens), mapped for Ollama, oMLX, OpenAI, and
  Anthropic; `current_time`; safe vs. approval-required policy; structured audit and activity UI.
- `web_search` (Brave, Exa) and `web_fetch` (DNS pinning, explicit redirects, size and type limits, inert extraction)
  with source cards, retained citations, prompt-injection labelling, and user HTTPS/allow/block policy.

MCP interoperability can follow after bottie's own tool contract and policy model are stable.

### Sandboxed Python execution

Status: core feasibility, macOS/Windows/Linux native containment proofs, opt-in development-bundle runner injection,
durable provider-neutral audit, all four mapped-provider integrations, and selected-lineage result presentation
complete; an opt-in macOS protected-distribution composition and a credential-free Linux protected-DEB
inspection/installed-containment producer and Windows protected-MSI shipping-containment producer are also complete,
with a credential-free aggregate protected-platform comparison contract, while shipping evidence remains pending.

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
- [x] credential-free packaged macOS development-app client/service/helper/runtime identity, App Sandbox denial,
      private-pipe execution, cancellation, and client-exit cleanup smoke;
- [x] credential-free release-candidate binding of the exact macOS, Windows, and Linux development package/containment
      evidence to one shared source revision and CPython/WASI runtime core while retaining exact platform layouts;
- [x] credential-free protected-package comparison against the accepted development runtime identity, requiring a
      separate exact-inspection-bound platform-native shipping containment record;
- [x] credential-free opt-in macOS protected-package staging and exact unsigned app inspection against the accepted
      runtime identity, without changing the default or protected distribution configuration;
- [x] credential-free macOS shipping-containment producer for an already signed protected app, with exact reinspection,
      independent signature verification, App Sandbox denial, private-pipe execution, cancellation, and client-exit cleanup;
- [x] opt-in protected macOS distribution composition that accepts only same-revision successful provenance, recreates
      the inspected app before credentials, signs nested code inside out, notarizes/staples/Gatekeeper-verifies the outer
      app, and runs final shipping containment plus protected-package comparison without changing the default path;
- [x] credential-free Linux protected-DEB public-signature verification, candidate-bound extraction inspection, exact
      fixed-layout installed helper/runtime equality, and Landlock/seccomp/rlimit plus process-lifecycle containment;
- [x] optional same-revision protected Linux Python workflow composition through exact pre-credential inspection,
      existing signing, installed-package containment, and final candidate comparison without changing the default path;
- [x] credential-free Windows protected-MSI and extracted-executable verification, candidate-bound extraction,
      fixed-layout installed controller/helper/runtime equality, and AppContainer/process-lifecycle containment;
- [x] optional same-revision protected Windows Python workflow composition through exact pre-credential inspection,
      existing Authenticode/updater signing, installed-package containment, and final candidate comparison;
- [x] credential-free per-platform envelopes and aggregate binding of the exact Linux, macOS, and Windows
      protected-package comparisons to one source revision, accepted release candidate, shared runtime core, canonical
      inspection/containment identities, and the normalized outer distribution produced by each protected run;
- [x] manual read-only aggregation workflow that accepts only three explicit same-revision successful distribution
      runs, downloads dedicated path-free comparison/distribution envelopes, and uploads only the bounded aggregate record;
- [x] credential-free protected-Python release-eligibility binding of one closed ready ordinary candidate to the exact
      same-revision aggregate while requiring its three normalized outer distributions to match the protected-run
      envelopes and retaining canonical input digests and signed native identities;
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

### Kastellan agent requests

- [x] `@kastellan:` prefix routes a message to a Kastellan agent daemon instead of the selected model;
- [x] transport through `kastellan-cli ask`, run directly for loopback hosts and over batch-mode `ssh` otherwise, with
      the task as data (argv locally, stdin remotely) and never as shell text;
- [x] durable provider-run placement of the reply under its request, background completion, and a reload event that
      keeps the composer free;
- [x] Settings → Kastellan agent: host and optional CLI path, shared validation, atomic save, and a side-effect-free
      Test connection;
- [x] regenerate and edit-and-resend route Kastellan requests back to Kastellan;
- [ ] keep a pending task across a Bottie restart instead of marking it interrupted;
- [ ] cancel a pending task from Bottie;
- [ ] answer Kastellan operator questions inside the conversation;
- [ ] decide whether a waiting conversation should accept new messages while its Kastellan task runs;
- [ ] optional Matrix transport, reusing Kastellan's paired, end-to-end-encrypted channel instead of the operator CLI.

## Milestone 6 — Reliability, privacy, and desktop beta

Outcome: bottie is safe and comfortable enough for sustained daily use.

- [x] first-run provider and privacy setup;
- [x] automatic backup rotation, corruption recovery, and staged migration promotion rollback;
- [x] crash-safe partial messages and interrupted indexing recovery;
- [x] structured local diagnostics with redaction and opt-in export;
- [x] first-party Localmail HTTPS origin, explicit certificate trust, bounded connection testing, and vault-held
      administrator-issued API key authentication foundation;
- [x] first-party Localmail `search_email` contract with closed filters, explicit current `sort`/`sort_order` mapping,
      newest-first date order by default, one pinned authenticated search call, bounded inert path-free summaries, and no
      email-body or attachment-content exposure;
- [x] first-party Localmail `open_email` contract over exact search-result identities with one pinned authenticated
      detail call, external images disabled, bounded inert header/body text, and no HTML or attachment-byte exposure;
- [x] first-party Localmail extracted attachment-text reading through exact message-local attachment numbers, with
      hashes resolved only in Rust, one fixed pinned `/text` request, bounded untrusted text, and no raw-byte fallback;
- [x] Localmail attachment reader accepts the current paged text contract, bounds the requested window, and marks
      unread text as truncated while retaining compatibility with older text-only responses;
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

Status: complete. Done: Rust-owned capture with voice activity detection, local streaming Whisper transcription,
transcript correction and text fallback, OS text-to-speech, barge-in through the shared cancellation boundary,
off-by-default audio delivery and retention, latency status, and durable input-device selection.

- [ ] acoustic echo cancellation or general system-feedback processing, if later evidence justifies native DSP.

### Optional neural speech evaluation — MOSS-TTS-Nano

The owner requested Nano work on 2026-10-17. This is a separate opt-in evaluation after Milestone 7, with the shipped
OS speech backend retained. See `docs/moss-nano-evaluation.md` and `scripts/moss-nano-spike/README.md`.

- [x] immutable ONNX/code asset inventory, explicit verified acquisition, Torch-free CPU worker, bounded streaming PCM,
      English/German short/numeric/long fixtures, process cancellation/reaping, and fresh-worker recovery;
- [x] owner approval of the short English/German Ava samples on 2026-10-18; numeric and long-form audition remain open;
- [x] optional Rust-owned Nano engine with bounded playback, saved voice preferences, and microphone/stop synchronization;
- [x] explicit command-line setup of a dedicated Python 3.12 runtime and pinned model cache, with compatible reuse;
- [ ] bundled Nano runtime/model installation, dependency/voice distribution review, and Linux/Windows qualification.

Current setup, native host proof, and qualification limits are in `docs/moss-nano-integration.md`.

## Milestone 8 — Generated images

Outcome: users can explicitly generate and edit durable images with exact model provenance through hosted or local
execution chosen by capability, without exposing credentials, provider result URLs, or native paths to the WebView.

Status: active product priority. App-store certification/publication and other shipping work are paused until this
milestone is complete or the product owner explicitly reprioritizes them.

These model tracks remain visibly distinct:

- **[Qwen-Image-2.0](https://docs.modelstudio.console.alibabacloud.com/en/model-studio/qwen-image-2-0):**
  use the exact hosted `qwen-image-2.0-2026-03-03` snapshot now. Add local execution only when exact 2.0 weights are
  publicly released and independently verified.
- **[Qwen-Image-2512](https://huggingface.co/Qwen/Qwen-Image-2512):** use the currently available Apache-2.0
  `Qwen/Qwen-Image-2512` text-to-image weights for the managed legacy local route. Never label 2512 output as Qwen-Image-2.0;
  `2512` means the December 2025 checkpoint.
- **[Qwen-Image-2.1](https://huggingface.co/Qwen/Qwen-Image-2.1):** prefer the installed MLX Serve 8-bit pack for local
  generation, with explicit upstream research-license acceptance before an optional installation. Reuse existing
  model bytes rather than downloading or copying a second set. See `docs/qwen-image-2.1-local.md`.

### Optional MOSS-Image local backend

The owner requested Apple MLX / otherwise CUDA execution on 2026-10-18. See `docs/moss-image-local.md`.

- [x] closed Base/SFT MLX and CUDA package/revision inventories, explicit verified setup, and platform-specific workers;
- [x] native private-pipe execution, bounded layout prompts/PNG output, cancellation/reaping, and one-image size catalog;
- [x] existing composer, initial readiness, normalization, durable MOSS provenance, and exact-family retry integration;
- [x] real Apple M3 Max SFT layout/storage/active-cancellation/recovery proof and stable app-data installation;
- [x] live Linux ARM64 NVIDIA GB10 CUDA SFT worker generation, active cancellation, and fresh-worker recovery;
- [x] verified development-signed contained-Python DMG and checked-in synthetic output/provenance evidence;
- [ ] native Linux app/UI, Windows, Base variants, and broader platform/low-memory/output-quality measurements;
- [ ] bundled runtime/model distribution and native installer UI, if requested separately.

### 8.1-8.2 Hosted Qwen-Image-2.0 and durable generation

Status: complete (checklists in git history before 2026-10-30). A bounded generation contract; a DashScope adapter
pinned to `qwen-image-2.0-2026-03-03` (Singapore default, vault-held key, opt-in live fixture); `generated_assets`
storage; one Rust-owned generation controller with shared cancellation; an explicit composer action (no implicit cloud
fallback); in-Rust download with fixed limits, PNG decode, and content-addressed storage; durable pending, completed,
failed, and cancelled image messages with retry; and export, backup, restore, retention, branching, and GC coverage.

### 8.3 Local worker and model lifecycle

- [x] Define a versioned private worker protocol shared by every local backend: `hello/capabilities`, `load`,
      `generate`, `progress`, `cancel`, `result`, and `shutdown`, with closed schemas and bounded frames over private pipes
      rather than a public localhost API;
- [x] add a long-lived Rust-owned worker manager so large weights load once, with one generation at a time initially,
      cooperative step cancellation, forced teardown after a bounded grace period, and no shell interpretation. The
      ordered lifecycle/correlation policy and private child-process transport own bounded stdin/stdout/stderr, clear the
      inherited environment, enforce handshake/read/write/shutdown deadlines, and kill and reap on failure;
- [x] keep model acquisition separate and explicit: show exact model/runtime IDs, license, expected disk and memory use,
      source revision, download progress, and verified file hashes before activation. The native manifest, path-free status,
      explicit approval/progress phases, all-files size/SHA-256 activation gate, transactional app-owned cache, and
      load-boundary re-verification are complete. Strict approved-root/revision/path/validator source planning, resumable
      ranged download, source-bound partial restart, fixed byte/time ceilings, cancellation, and durable progress reporting
      are also complete. Hugging Face delivery now uses one manually validated 302/307 resolution envelope with automatic
      redirects still disabled. The exact q4 candidate now has measured acceptance and a selected package manifest;
      native path-free availability states, exact installed-worker/cache re-verification, the app-owned service, explicit
      Cloud/local composer presentation, and the single-slot product acquisition coordinator are complete. Its closed
      acknowledgement binds every displayed package fact, startup inspection is read-only, cancellation retains only
      exact synced partials, and atomic promotion refreshes readiness without exposing paths, hashes, URLs, or response
      detail;
- [x] store models in an app-owned cache, generate offline after installation, prohibit worker network access during
      generation, and expose only readiness/capability metadata to Svelte. App-owned resumable staging and atomic verified
      promotion, read-only readiness re-verification, and the real-runtime network-denied proof are complete;
      app-cache-only service resolution, explicit model and worker installation, fresh execution-time verification,
      network-denied native execution, and path-free presentation are complete. The worker is imported transactionally from
      a user-selected native folder because no automatic worker source or accepted archive digest exists;
- [ ] probe hardware and runtime support rather than inferring it from the operating system. An unavailable local route
      remains unavailable; there is no silent cloud fallback. The native macOS probe now combines OS-owned physical memory,
      compile-target architecture, the exact accepted Apple M3 Max 128 GiB profile, worker-bundle evidence, and cache
      integrity. Application-service wiring is complete; other hardware profiles remain unavailable;
- [x] normalize local results through the same Rust PNG validation, durable storage, provenance, cancellation, export,
      deletion, and exact-retry path as hosted results;
- [x] keep this worker distinct from Bottie's user-approved Python tool runtime: image generation is a product
      subsystem, not arbitrary Python execution and not a reason to broaden tool grants.

### 8.4 Qwen-Image-2512 local execution now

- [x] **macOS Apple silicon:** prove a pinned [MLX-Gen](https://github.com/lpalbou/mlx-gen) worker with
      `Qwen/Qwen-Image-2512`, then choose one supported MLX-Gen package tier from measured hardware evidence. Current
      published packages are roughly 16.2 GiB for mixed 4-bit and 27.5 GiB for 8-bit; do not auto-download either. The
      mixed q4/q8 `AbstractFramework/qwen-image-2512-4bit` candidate is pinned to an immutable 18-file revision and exact
      MLX-Gen 0.18.2 commit. Its exact worker bundle, decoded output, 29,526,129,448-byte lifetime peak footprint, visual
      quality, network denial, and 110 ms denoising-boundary cancellation are accepted on the named target hardware. The
      native availability gate fails closed for every other hardware profile and for missing or mismatched worker/cache
      bytes. A freshly verified installation can now run one 512x512 text-to-image request through the reusable private
      worker, shared durable PNG pipeline, exact local provenance and seed, cooperative cancellation, and exact retry;
- [x] **Linux NVIDIA:** the pinned PyTorch + Diffusers `QwenImagePipeline` proof passes Bottie's private protocol on a
      named DGX Spark GB10 target. The exact ARM64 runtime and full public model revision produced byte- and pixel-identical
      cold/warm PNGs, denied network access, cancelled at a denoising boundary in 100 ms, and recorded conservative unified
      host-memory measurements. CPU offload was disabled; DGX Spark exposes no separate aggregate VRAM counter. This proves
      feasibility only. The native catalog now binds this candidate's backend, runtime, model, proof target, and evidence
      document while leaving its accepted profile and importable executable absent. A proof-only inspector now binds the
      exact NGC/Diffusers inputs, emits a deterministic native-compatible regular-file/executable/bundle inventory, and
      requires complete file ownership plus included third-party licence bytes; every record remains explicitly unreviewed.
      A separate DGX gate now inventories all 248 Python and 434 Debian package-manager components with deterministic,
      path-free licence evidence and refuses assembly on 76 exact missing-byte or undeclared-licence blockers. Linux remains
      unavailable. A traced closure gate now measures 340 required files across 84 components, recursively closes Python
      requirements and ELF dependencies, records an explicit NVIDIA host-driver boundary, and assigns all observed bytes
      an exact Python, Debian, or marker-backed native identity. Two independent records prove the closure is complete. It
      still refuses assembly on 106 licence-only blockers: five missing byte records, seventeen undeclared licences, and
      84 unreviewed expressions. An exact-image/exact-trace review-manifest gate now preserves measured source and review
      bytes while rejecting partial component coverage and drift. The closure now binds exact in-image source evidence for
      cuSPARSELt, Open MPI, and UCX. A separate offline read-only source gate now recognizes exact authoritative PyPI
      archives for SentencePiece and tokenizers plus exact NVIDIA NVPL BLAS/LAPACK archives; the NVPL evidence additionally
      requires both installed runtime files to match archive members byte-for-byte. It never infers expressions and has not
      changed the checked-in DGX closure record. A second exact-source pass found no public OpenUCX object for the pinned
      UCC revision; NVIDIA's public v2.24 UCC notices name only `rdma-core`. The EULA-authorized target-matching 2.24.1
      ARM64 archive matches its published digest but contains no UCC licence/source member, and its regular UCC library
      differs from the retained image by eight bytes and SHA-256. The exact image UCC binary therefore still lacks
      authoritative licence bytes. A corrected replacement control with the checksum-pinned conventional ARM64 PyTorch
      2.10.0 CUDA 13.0 wheel completed the full offline worker proof under its distinct runtime identity while the UCC
      installation was masked and absent from live process maps. That image still contains masked UCC bytes. No review
      manifest exists and Linux remains unavailable. Bottie must either resolve the exact retained-image artifact or prove
      a clean runtime without the UCC runtime. The first clean Ubuntu-based image now passes the full proof with identical
      output and zero `libucc` mappings. Its complete 63-wheel/112-Debian input set is now byte-frozen in a canonical lock:
      175 authoritative archives totaling 3,016,341,507 bytes, with separate official-source provenance and independent
      verification. The gate binds the exact image inventory, top-level wheel metadata and compatibility tags, Debian
      control identities, sizes, hashes, ARM64 target, and lock digest. Renamed foreign wheels still fail closed; one
      upstream NVIDIA SBSA spelling mismatch is accepted only for the exact official cuSPARSELt filename, size, and PyPI
      digest. A closed rebuild plan and host gate now bind the exact base, input lock, and three worker-source files; they
      require two independently named BuildKit runs with networking, pulls, and cache disabled, exact installed-inventory
      agreement, and a normalized regular-file path/mode/ownership/size/hash comparison. Two fresh offline rebuilds now
      agree across exact package inventories and 27,035 normalized files; two independent full proofs against rebuilt
      bytes reproduce the reviewed output, deny network access, and retain fresh trace digests with no UCC mapping. No
      external `readelf` package was added: a bounded in-process AArch64 ELF reader now closes the rebuilt runtime without
      changing its frozen inputs. Two independent read-only, non-root, offline collections from the retained traces agree
      on 161 files totalling 4,039,257,278 bytes, including 114 ELF files and 62 exact components. The rebuilt closure is
      complete. A clean-profile-only source gate now requires exact authoritative SentencePiece 0.2.2 and tokenizers
      0.23.2 archives, including both archive and licence-member measurements, and both independent collections reproduce
      one updated path-free record. No missing licence-byte blocker remains; four expressions are undeclared and all 62
      expressions remain unreviewed, leaving 66 licence-only blockers. No licence-review manifest, bundle, accepted Linux
      profile, app wiring, or product availability exists. Bottie must independently review every exact expression,
      produce reviewed bundle bytes, and add native hardware gating plus app-owned execution.
      See
      `docs/local-image-linux-nvidia-proof.md` and
      `docs/local-image-linux-worker-bundle.md`;
- [ ] **Windows NVIDIA:** use the same pinned Diffusers worker and protocol after a native Windows CUDA/package proof;
      do not treat WSL-only success as Windows product evidence;
- [ ] **Linux AMD:** add a ROCm Diffusers route only after a named-GPU proof demonstrates correct decoded output and
      acceptable memory/cancellation behavior;
- [ ] **Windows AMD/Intel and lower-memory GPUs:** evaluate a pinned
      [stable-diffusion.cpp](https://github.com/leejet/stable-diffusion.cpp) Vulkan/GGUF worker as a later fallback. Require
      same-seed output review across supported quantizations before enabling it; do not claim DirectML support without a
      verified runtime;
- [ ] retain DiffSynth-Studio, vLLM-Omni, and ComfyUI as benchmark/reference integrations, not embedded product
      dependencies, unless measured evidence shows the selected workers cannot meet Bottie's lifecycle contract;
- [x] publish a support matrix from real hardware evidence. Initial availability may legitimately be Apple silicon and
      NVIDIA only; unsupported hardware must fail clearly before model download or generation.

The local 2512 track is text-to-image only. Do not silently combine it with the separate Qwen-Image-Edit-2511 weights.
Editing should first land through the unified hosted 2.0 model after generation persistence is stable.

### 8.5 Qwen-Image-2.0 editing and future local weights

- [x] add hosted 2.0 reference-image editing through the existing native attachment-byte consent boundary, supporting
      one to three validated inputs without forwarding local paths. The provider-neutral request and exact offline wire
      serializer, native provider execution, path-free controller command, bounded download lifecycle, cancellation, and
      exact edit retry are complete. Cloud Image mode accepts ready current-draft images and explicitly selected completed
      generated images from the visible ancestry, preserves ordered opaque source identities, and shows source-byte and
      charge disclosure before submission. Local editing remains unavailable without fallback;
- [x] persist edit lineage from every source asset to the generated assistant asset, including exact hosted model and
      execution provenance. Generated edits now present a compact accessible source count and ordered path-free source
      kind, dimensions, media type, and byte size after initial completion and durable reopen, without rendering opaque
      IDs; ordinary text-to-image results omit the lineage panel;
- [ ] monitor only official Qwen repositories/model registries for an exact 2.0 weight release. A 2026-09-16 recheck of
      Qwen's GitHub repository plus its Hugging Face and ModelScope inventories still found no exact 2.0 weights. A paper,
      API alias, community conversion, similarly named model, or 2512 package is not sufficient;
- [ ] when exact weights ship, freeze their revision, files, hashes, license, architecture metadata, and reference
      output before selecting MLX and CUDA/ROCm/Windows runtimes through the same worker conformance suite;
- [ ] expose local 2.0 only after generation and editing both pass native-byte, cancellation, provenance, and hardware
      acceptance. Keep hosted and local execution explicit even when their model identity matches.

### 8.6 Preferred installed Qwen-Image-2.1 local generation

- [x] separate Chat and Image composer tabs, preserve prompt drafts, collapse model details, and expose exact
      size/aspect presets for Cloud and Local 2.1; retain the legacy worker profile and retry dimensions;
- [x] prefer the exact loaded MLX Serve 8-bit pack with explicit image capability on loopback port 11234; keep the
      oMLX text service and managed 2512 route separate;
- [x] reuse installed files through the local image API, normalize bounded inline PNGs behind Rust, retain exact
      quantized-package/Local/seed provenance, and bind start/retry to the disclosed model without fallback;
- [x] prove native generation, durable completion, and disconnected-client cancellation on the Apple M3 Max host;
- [x] retain explicit revision-bound research-license consent for managed acquisition and document optional external
      server installation without automatic model download;
- [x] start the optional 2.1 installer with an exact installed-pack source inventory, revision/terms-bound consent,
      transactional native model download, source-bound resume/cancellation, and verified-cache reuse;
- [ ] qualify/install the managed MLX Serve runtime, own its lifecycle, and expose the optional 2.1 installation
      action with license acceptance, progress, cancellation, and readiness in the app. Model-file verification alone
      does not qualify generation readiness. See the managed-installer section of `docs/qwen-image-2.1-local.md`.

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
