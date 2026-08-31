# Bottie handover

Last verified: 2026-08-31

## Start here

PR #125 is merged. `main` was synchronized with `origin/main` at merge commit `688edba` before this handover was
pruned. Milestones 0–6 are complete; Milestone 7 is the active roadmap work.

The next session should implement only the bounded voice-latency slice below. Do not replay completed slices or use
this handover as a historical record; Git history, `ROADMAP.md`, and `README.md` retain that context.

Read, in order:

1. `HANDOVER.md`
2. the Milestone 7 section of `ROADMAP.md`
3. the Local voice capture section of `README.md`
4. `CONTRIBUTING.md`
5. the relevant implementation and tests:
   - `src-tauri/src/microphone.rs` and `src-tauri/src/microphone/`
   - `src-tauri/src/speech.rs`
   - `src-tauri/src/lib.rs`
   - `src/lib/microphone.ts`
   - `src/lib/MicrophoneControl.svelte`
   - `src/routes/microphone-state.svelte.ts`

## Current voice boundary

Milestone 7 currently provides:

- explicit Rust-owned default-input capture with a 60-second and 32-MiB session-memory ceiling;
- native 20-ms energy-based speech/silence detection and path-free timing;
- local streaming transcription through the pinned multilingual Whisper tiny Q5 model, with final turn correction;
- explicit local text-to-speech, process-lifetime opaque voice selection, and no generated-audio retention;
- barge-in that stops Bottie's playback, cancels provider/tool work, and serializes capture against generation;
- separate, off-by-default stopped-capture choices for provider delivery and app-private WAV retention.

PR #125 established the following contract:

- only an explicitly audio-capable oMLX or OpenAI-compatible model may receive captured audio;
- Rust converts the stopped mono PCM to bounded PCM16 WAV and places it in one native-only content block;
- audio is sent on the initial provider request only and is removed before every tool follow-up;
- Ollama and Anthropic-compatible routes remain audio-disabled;
- local retention is independent of provider capability and uses the existing attachment backup, export, branch,
  removal, forget, and garbage-collection policy;
- accepting either delivery or retention consumes the session capture and transcript; a capability, encoding, or
  retention failure leaves them available for correction or retry.

Preserve these boundaries:

- Rust owns PCM, encoded audio, speech engines, model/cache details, filesystem paths, hashes, provider payloads,
  persistence, cancellation, and timing policy.
- The WebView receives only bounded typed path-free status. It must not receive audio bytes, wall-clock timestamps,
  device identities, engine identities, native/provider call identities, paths, or hashes.
- Voice capture, provider delivery, retention, playback, and interruption remain explicit user actions. Capture must
  remain fail-closed if Bottie's playback cannot be positively stopped.
- Capture, transcript corrections, voice choice, and any latency observations remain process/session state unless a
  later handover explicitly authorizes persistence.
- Provider and tool cancellation must continue through the existing durable run boundary.

`CONTRIBUTING.md` is authoritative: use TDD, mandatory docstrings, pure helpers, named constants, practical 500-line
and 120-character limits, and update relevant documentation at slice completion. Bottie is not a Cargo workspace; use
`--manifest-path src-tauri/Cargo.toml` and run Cargo commands serially.

## Next bounded slice: local voice-latency status

### Goal

Add bounded native measurements for latency that Bottie's existing capture, transcription, and playback state
machines can observe honestly, then present them as calm, accessible, path-free session status.

### Acceptance boundary

1. Define each reported interval from exact existing native lifecycle events before adding presentation. Use monotonic
   elapsed time, optional bounded integer milliseconds, and saturating conversion. Do not expose absolute or wall-clock
   timestamps.
2. Report only intervals whose endpoints Rust can positively observe. Label them by their actual semantics; polling or
   a speech-engine callback must not be described as physical first-sample, first-token, or audible-output latency
   unless that endpoint is genuinely observed.
3. Keep the data session-only and reset it when its capture or playback action is discarded, replaced, consumed, or
   made stale. Never write latency data to SQLite, diagnostics, provider requests, tool records, exports, or backups.
4. Extend the existing typed microphone/speech status with only the small path-free summary needed by the UI. Keep
   device, model, engine, content, provider, and native call identity out of IPC.
5. Show available measurements without turning the composer or response actions into a telemetry dashboard. Missing
   or in-progress values need calm copy and must not render as zero. Preserve keyboard operation, screen-reader status,
   narrow viewport containment, and reduced-motion behavior.
6. Start with focused failing tests for interval definitions, bounds, stale/reset behavior, serialization, and UI copy.
   Manually exercise the meaningful native flow where the host permits it, and state exactly which endpoints were
   observed. Do not infer device-wide or acoustic performance from unit tests, polling, compilation, or app launch.

### Explicit exclusions

Do not add device selection, acoustic echo cancellation or feedback processing, wake words, automatic listening or
playback, transcript insertion into the composer, full text fallback, audio response blocks, generated-audio
retention, persisted telemetry, analytics, provider-reported latency, performance optimization, a schema migration,
release/update publication, protected workflow dispatch, or Microsoft Store work.

Microsoft Store certification and publication remain deferred until fresh release-owner notice. The earlier rejected
submission is not certification or publication evidence.

## Base evidence and limitations

The merged PR #125 base passed on 2026-08-31:

- `npm run format:check`
- `npm run check` with no errors or warnings
- `npm test`: 219 passed, 3 skipped across 53 files
- `npm run build`
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`: 437 passed, 33 ignored opt-in/live tests

The `?voice=audio-content` browser fixture was reviewed at 1320×820 and 720×620. No credential-backed live audio
request, microphone hardware capture, Windows/Linux native interaction, package, signing, release, updater, protected
workflow, or Store action was run. Treat those as unverified, not implied by the automated evidence.

Run the standard checks before handing off the next slice:

```sh
npm run format:check
npm run check
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

The worktree currently contains unrelated untracked logo-kit, screenshot, and Linux signing-public-key files. Preserve
them and stage only reviewed paths.
