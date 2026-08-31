# Bottie handover

Last verified: 2026-09-01

## Start here

PR #126 is merged. Draft PR #127 contains the implemented and reviewed bounded input-device-selection slice on
`codex/input-device-selection`, based on synchronized `origin/main` merge commit `8293c16`. Milestones 0–6 are
complete; Milestone 7 is the active roadmap work.

The next session should implement only the bounded explicit transcript-to-text fallback slice below. Do not replay
completed slices or use this handover as a historical record; Git history, `ROADMAP.md`, and `README.md` retain that
context.

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
- lazy bounded input-device discovery with sanitized labels, process-local opaque tokens, and an exact-device capture
  boundary that never exposes or persists native identity.

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

The input-device slice adds this exact contract:

- **Choose microphone** and **Refresh** are the only discovery actions; startup and ordinary status polling do not
  enumerate devices, open a stream, or request microphone permission;
- discovery returns at most 64 sanitized 160-byte display labels, process-local opaque tokens, and a distinct
  **System default** choice. Native CPAL `DeviceId`, host API, path, and hardware details remain Rust-only;
- selection remains in one process-memory registry and is never written to settings, SQLite, diagnostics, provider or
  tool records, exports, or backups;
- System default resolves the operating system's current default at Record. A concrete token resolves only to its exact
  current native input; disappearance produces `selected_device_unavailable` and never falls back to another device;
- discovery, selection, empty/stale states, and reduced-motion/narrow presentation stay keyboard and screen-reader
  operable without changing capture, VAD, transcription, retention, provider-delivery, cancellation, or latency rules.

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

## Next bounded slice: explicit transcript-to-text fallback

### Goal

Let a user explicitly move a completed local transcript into the text composer as an editable draft, without silently
sending, persisting, retaining, or discarding either the transcript or captured audio.

### Acceptance boundary

1. Offer **Use transcript as text** only for the current final ready transcript and only after an explicit user action.
   Partial, preparing, failed, empty, or stale transcript generations must not populate the composer.
2. Derive the draft from the already bounded visible final turns, preserving their order and corrected text. Do not
   add a new native content-return command or expose PCM, paths, model/cache details, native IDs, or hidden transcript
   state.
3. Keep the result an ordinary editable unsent composer draft. Do not automatically submit it, create a conversation,
   select provider audio delivery, retain a WAV, write telemetry, or persist the session transcript.
4. Preserve an existing non-empty draft with an explicit, deterministic append boundary rather than overwriting it.
   Apply the composer's existing request limits and fail visibly if the combined text cannot be represented safely.
5. Keep captured audio and the native transcript available for correction, provider delivery, retention, or Discard
   after copying. Only the existing accepted delivery/retention actions may consume session capture state.
6. Add calm keyboard and screen-reader copy, focus behavior, and focused tests for unavailable, empty, corrected,
   existing-draft, repeated-action, and boundary-limit cases. Review desktop and narrow presentation.

### Explicit exclusions

Do not add automatic transcript insertion or submission, new native transcript IPC, output-device selection, acoustic
echo cancellation or feedback processing, wake words, automatic listening or playback, audio response blocks,
generated-audio retention, persisted telemetry, analytics, provider-reported latency, performance optimization, a
schema migration, release/update publication, protected workflow dispatch, or Microsoft Store work.

Microsoft Store certification and publication remain deferred until fresh release-owner notice. The earlier rejected
submission is not certification or publication evidence.

## Current evidence and limitations

The input-device slice has focused coverage for enumeration caps, Unicode-safe label sanitization, opaque-token
serialization, non-reuse of stale tokens, exact native resolution, unavailable-device failure, empty/one/stale UI
states, selector copy, and reduced-motion presentation. The standard checks completed on 2026-09-01:

- `npm run format:check`
- `npm run check` with no errors or warnings
- `npm test`: 224 passed, 3 skipped across 53 files
- `npm run build`
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml --no-run`: both Rust test executables compiled

The `?voice=input-devices` browser fixture was reviewed at 1320×820 and 720×620. At 720 px the selected label,
Refresh, and session-only note remained contained with no page-level horizontal overflow. A 390 px review confirmed
the selector itself wrapped visibly; the broader pre-existing shell still clips its composer at that width and was
not changed in this voice slice. These checks verify deterministic presentation only.

`npm run tauri dev` compiled and started the real arm64 Bottie process; a host process check confirmed the target was
running before the temporary development session was stopped. Device discovery, selection, Record, recognition, and
playback were not exercised in that native window. `codesign --verify --strict` returned `CSSMERR_TP_NOT_TRUSTED`, so
this is native-process evidence only and not successful signature-trust verification.

The ordinary full `cargo test` command and the development-signed runner both reached the freshly linked library test
executable, but macOS held it before the test harness started. Each attempt was stopped after 60 seconds with no test
output. Compilation passed, but final Rust test execution is not claimed. No microphone hardware, real device switch,
stale-device disconnection, Windows/Linux native interaction, provider request, package, release, updater, protected
workflow, or Store action was run. Treat those as unverified, not implied by compilation or browser/native launch.

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
