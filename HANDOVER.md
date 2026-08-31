# Bottie handover

Last verified: 2026-08-31

## Start here

PR #125 is merged. The bounded local voice-latency status slice is implemented and reviewed on
`codex/voice-latency-status` from synchronized `origin/main` commit `8aa571c`. Milestones 0–6 are complete;
Milestone 7 is the active roadmap work.

The next session should implement only the bounded input-device-selection slice below. Do not replay completed slices
or use this handover as a historical record; Git history, `ROADMAP.md`, and `README.md` retain that context.

Read, in order:

1. `HANDOVER.md`
2. the Milestone 7 section of `ROADMAP.md`
3. the Local voice capture section of `README.md`
4. `CONTRIBUTING.md`
5. the relevant implementation and tests:
   - `src-tauri/src/microphone.rs` and `src-tauri/src/microphone/`
   - `src-tauri/src/speech.rs` and `src-tauri/src/speech/tests.rs`
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
- separate, off-by-default stopped-capture choices for provider delivery and app-private WAV retention;
- bounded session-only native timing for input-stream readiness, first/final transcript availability, and speech-engine
  acceptance, with each interval labelled by the endpoint Rust actually observes.

PR #125 established the following contract:

- only an explicitly audio-capable oMLX or OpenAI-compatible model may receive captured audio;
- Rust converts the stopped mono PCM to bounded PCM16 WAV and places it in one native-only content block;
- audio is sent on the initial provider request only and is removed before every tool follow-up;
- Ollama and Anthropic-compatible routes remain audio-disabled;
- local retention is independent of provider capability and uses the existing attachment backup, export, branch,
  removal, forget, and garbage-collection policy;
- accepting either delivery or retention consumes the session capture and transcript; a capability, encoding, or
  retention failure leaves them available for correction or retry.

The latency slice adds this exact contract:

- Record-request timing begins only when the native microphone controller accepts a new action; input readiness ends
  only after `stream.play()` succeeds and does not claim that a first sample arrived;
- first-transcript timing ends when the first non-empty bounded local transcript is applied for the current capture ID
  and generation, while stale results are ignored;
- final-transcript timing begins when the active Stop command is accepted, or when limit-driven capture finalization is
  scheduled, and ends only when a successful final result (including an empty transcript) is applied;
- playback timing begins inside the native Play action and ends when the local engine's `speak` call succeeds; it is
  engine-acceptance timing, not an audible-output callback;
- every interval uses `Instant`, saturates into optional `u32` milliseconds, remains session-only, and clears on the
  relevant replacement, discard, accepted audio consumption, stop, natural completion, failure, or stale result;
- IPC adds only nested optional integer timing summaries. It contains no wall-clock timestamp, device/model/engine
  identity, content, provider/native call identity, path, hash, PCM, or encoded audio.

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

## Next bounded slice: explicit local input-device selection

### Goal

Let a user explicitly choose one currently available microphone without exposing native device identity or weakening
the existing default-input, permission, and session-only capture boundary.

### Acceptance boundary

1. Enumerate input devices lazily only after an explicit user action. Keep native identifiers in Rust; expose at most a
   bounded sanitized display label and process-local opaque selection token, plus a distinct system-default choice.
2. Keep selection session-only. Never write a device name or token to SQLite, settings, diagnostics, provider requests,
   tool records, exports, or backups, and never expose a device ID, host API, path, or hardware address.
3. Resolve the opaque token back to the exact native device only at capture start. Fail closed with an existing or new
   stable path-free error if it disappeared; do not silently capture from a different microphone after explicit choice.
4. Preserve the existing permission-on-Record rule, capture ceilings, VAD/transcription pipeline, barge-in guard,
   discard/consumption behavior, latency definitions, and provider/retention boundaries.
5. Add a calm keyboard and screen-reader operable selector near Record without crowding the composer. Handle no-device,
   one-device, stale-selection, narrow viewport, and reduced-motion states explicitly.
6. Start with focused failing tests for enumeration bounds/sanitization, opaque-token resolution, stale selection,
   path-free serialization, and UI copy. Exercise real selection/capture only where the host permits, and scope the
   claim.

### Explicit exclusions

Do not add output-device selection, acoustic echo cancellation or feedback processing, wake words, automatic listening
or playback, transcript insertion into the composer, full text fallback, audio response blocks, generated-audio
retention, persisted telemetry, analytics, provider-reported latency, performance optimization, a schema migration,
release/update publication, protected workflow dispatch, or Microsoft Store work.

Microsoft Store certification and publication remain deferred until fresh release-owner notice. The earlier rejected
submission is not certification or publication evidence.

## Current evidence and limitations

The latency slice has focused passing coverage for monotonic interval definitions, millisecond saturation, stale/reset
behavior, path-free nested serialization, playback stop/natural-completion reset, UI wording, and responsive rendering.
The standard checks passed on 2026-08-31:

- `npm run format:check`
- `npm run check` with no errors or warnings
- `npm test`: 221 passed, 3 skipped across 53 files
- `npm run build`
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`: 441 passed, 33 ignored opt-in/live tests

The `?voice=final-transcript` and `?voice=local-playback` browser fixtures were reviewed at 1320×820 and 720×620. At
720 px, the unobstructed composer had no page-level horizontal overflow, and both capture and playback timing remained
contained. This verifies deterministic presentation only. No microphone hardware, real recognizer timing, audible
playback timing, credential-backed audio request, Windows/Linux native interaction, package, signing, release, updater,
protected workflow, or Store action was run. Treat those as unverified, not implied by tests or browser review.

`npm run tauri dev` compiled and started the real arm64 Bottie process with the updated native status contracts. No
Record, recognition, or playback control was exercised. A separate `codesign --verify --strict` check returned
`CSSMERR_TP_NOT_TRUSTED`, so this run is native-launch evidence only and not a successful signature-trust verification.

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
