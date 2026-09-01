# Bottie handover

Last verified: 2026-09-01

## Start here

PR #127 is merged. Draft PR #128 contains the implemented and reviewed bounded explicit transcript-to-text fallback
on `codex/transcript-text-fallback`, based on synchronized `origin/main` merge commit `6be2a03`. Milestones 0–7 are
complete except for evidence-gated acoustic feedback processing, which remains deferred.

The next session should perform only the bounded native voice acceptance closure below. Do not replay completed
slices or use this handover as a historical record; Git history, `ROADMAP.md`, and `README.md` retain that context.

Read, in order:

1. `HANDOVER.md`
2. the Milestone 7 section of `ROADMAP.md`
3. the Local voice capture section of `README.md`
4. `CONTRIBUTING.md`
5. the relevant implementation and tests:
   - `src/lib/microphone.ts` and `src/lib/microphone.test.ts`
   - `src/lib/MicrophoneControl.svelte` and `src/lib/MicrophoneControl.test.ts`
   - `src/lib/Composer.svelte` and `src/lib/Composer.test.ts`
   - `src/routes/page-state.svelte.ts` and `src/routes/page-state.test.ts`
   - `src/routes/composer-interaction-state.ts` and its test
   - `src/routes/microphone-state.svelte.ts`

## Current voice boundary

Milestone 7 provides:

- explicit Rust-owned selected-input capture with a 60-second and 32-MiB session-memory ceiling;
- native 20-ms energy-based speech/silence detection and path-free timing;
- local streaming transcription through the pinned multilingual Whisper tiny Q5 model, with final turn correction;
- explicit local text-to-speech, process-lifetime opaque voice selection, and no generated-audio retention;
- barge-in that stops Bottie's playback, cancels provider/tool work, and serializes capture against generation;
- separate, off-by-default stopped-capture choices for provider delivery and app-private WAV retention;
- bounded session-only native timing for input readiness, first/final transcript availability, and speech-engine
  acceptance, labelled only by endpoints Rust observes;
- lazy bounded input discovery with sanitized labels, process-local opaque tokens, and exact-device capture;
- explicit transcript-to-text copying into the ordinary editable unsent composer draft.

The transcript-to-text slice adds this exact contract:

- **Use transcript as text** appears only for the current captured, ready, non-empty transcript when every visible
  turn is final. Partial, preparing, failed, empty, recording, and stale states do not expose the action;
- the frontend derives text only from the already bounded visible turns, in order, using corrected text already
  present in path-free status. No native command or hidden transcript-return path was added;
- an empty draft receives the transcript directly. Any existing draft is preserved exactly and followed by one blank
  line before the transcript. Repeating the explicit action appends through the same deterministic boundary;
- the combined draft must fit 32 KiB of UTF-8. Failure leaves both draft and capture unchanged and is announced as an
  alert. Success focuses the composer with the caret at the end and announces that nothing was sent;
- a final transcript or an existing non-empty local draft keeps the composer editable without provider/model
  readiness. Provider/model requirements still gate Send, so offline editing cannot submit or create a conversation;
  active native message persistence continues to lock editing so later draft changes cannot be cleared by completion;
- copying never submits, creates a conversation, persists transcript text, selects audio delivery or retention,
  writes telemetry, or consumes capture state. Correction, provider delivery, retention, repeated copy, and Discard
  remain available afterward.

Preserve these boundaries:

- Rust owns PCM, encoded audio, speech engines, model/cache details, filesystem paths, hashes, provider payloads,
  persistence, cancellation, and timing policy.
- The WebView receives only bounded typed path-free status. It must not receive audio bytes, wall-clock timestamps,
  device or engine identities, native/provider call identities, paths, or hashes.
- Voice capture, provider delivery, retention, playback, interruption, and transcript copying remain explicit user
  actions. Capture remains fail-closed if Bottie's playback cannot be positively stopped.
- Capture, corrections, device/voice choice, and latency observations remain process/session state.
- Provider and tool cancellation continues through the existing durable run boundary.

`CONTRIBUTING.md` is authoritative: use TDD, mandatory docstrings, pure helpers, named constants, practical 500-line
and 120-character limits, and update relevant documentation at slice completion. Bottie is not a Cargo workspace; use
`--manifest-path src-tauri/Cargo.toml` and run Cargo commands serially.

## Next bounded slice: native voice acceptance closure

### Goal

Close the remaining evidence gap for the completed Milestone 7 voice workflow on a real macOS microphone and local
speaker, without adding a new voice feature or broadening any persistence, provider, or release boundary.

### Acceptance boundary

1. In the development-signed native app, exercise System default and one explicitly selected input where available,
   Record, visible speech/silence state, Stop, final local transcription, one correction, **Use transcript as text**,
   editing the unsent draft, repeated copy, and Discard.
2. Confirm copying into both an empty and an existing non-empty draft, the blank-line append boundary, composer focus,
   no automatic send or conversation creation, and retained correction/delivery/retention/Discard controls.
3. Exercise **Play response aloud**, **Stop local playback**, and one **Interrupt & record** attempt. Record only the
   endpoints actually observed; do not infer acoustic latency, provider cancellation completion, or cross-platform
   behavior from a successful launch.
4. Check the transcript action and feedback with ordinary keyboard navigation and VoiceOver if available. Keep user
   confirmation distinct from automated or visual evidence.
5. If a product defect is found, add the smallest focused regression test and fix only that defect. Otherwise update
   documentation with the bounded manual evidence and remaining limitations.

### Explicit exclusions

Do not add output-device selection, acoustic echo cancellation or feedback DSP, wake words, automatic listening or
playback, audio response blocks, generated-audio retention, persisted telemetry, analytics, provider-reported latency,
schema changes, release/update publication, protected workflow dispatch, or Microsoft Store work.

Microsoft Store certification and publication remain deferred until fresh release-owner notice. The earlier rejected
submission is not certification or publication evidence.

## Current evidence and limitations

Focused transcript-transfer coverage verifies unavailable, empty, partial, failed, stale, corrected, existing-draft,
repeated-action, exact-boundary, over-limit, native-state-retention, visible-error, and focus/caret behavior. The
standard checks completed on 2026-09-01:

- `npm run format:check`
- `npm run check` with no errors or warnings
- `npm test`: 236 passed, 3 skipped across 54 files
- `npm run build`
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`: 442 library tests and 1 updater-evidence test passed, 33
  intentionally ignored, and doc tests passed

The `?voice=final-transcript` browser fixture was reviewed at 1320×820 and 720×620. The action copied all three visible
turns, moved focus and the caret to the draft end, announced that nothing was sent, and left the transcript visible.
At 720 px the composer remained within its 660 px bounds with no page-level horizontal overflow. At 390 px the new
action stacked correctly, while the broader pre-existing shell retained its documented 720 px layout width and
clipped; that shell was not changed in this slice. Browser fixtures do not prove native microphone, transcription,
audio delivery, retention, playback, or cancellation behavior.

The provider-unavailable review regression was also exercised at 720×620: without artificial provider readiness, the
composer accepted the transcript, remained focused and editable for added text, and kept Send disabled. Native
Discard is unavailable in the browser fixture; state coverage verifies that an existing offline draft remains
editable independently of transcript availability.

No native app, microphone hardware, selected-device change, local recognition, provider request, audio retention,
speaker playback, interruption, package, release, updater, protected workflow, or Store action was run in this slice.
The Rust build retains the existing future-incompatibility notice for `block 0.1.6`; it is not introduced by this
frontend-only change.

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

The worktree contains unrelated untracked logo-kit, screenshot, and Linux signing-public-key files. Preserve them and
stage only reviewed paths.
