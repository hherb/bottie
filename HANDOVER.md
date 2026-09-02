# Bottie handover

Last verified: 2026-09-02

## Start here

PR #128 is merged at `6ae2e0d`. Draft PR #129 contains the bounded repeated native capture fix on
`codex/native-voice-acceptance`. Milestones 0–7 remain complete except for evidence-gated acoustic feedback
processing, which remains deferred.

The next session should perform only the remaining native voice acceptance below. Do not replay completed slices or
broaden this into a new voice, persistence, release, updater, or Store feature.

Read, in order:

1. `HANDOVER.md`
2. the Milestone 7 section of `ROADMAP.md`
3. the Local voice capture section of `README.md`
4. `CONTRIBUTING.md`
5. `src-tauri/src/microphone.rs`, `src-tauri/src/microphone/tests.rs`, and the nearby capture/transcription modules

## Completed boundary and evidence

Milestone 7 keeps PCM, encoded audio, model/cache details, filesystem paths, hashes, provider payloads, cancellation,
and timing policy in Rust. The WebView receives bounded typed path-free status only. Capture, correction, transcript
copying, delivery, retention, playback, and interruption remain explicit actions; session voice state is not persisted.

Native macOS acceptance on 2026-09-02 established:

- lazy discovery exposed two bounded microphone choices and an explicitly selected MacBook Pro microphone;
- selection changes worked, and the selected input produced a stopped final local transcript with path-free native
  timing and retained-native-memory feedback;
- **Use transcript as text**, explicit Send, and the blank-line append boundary worked;
- the first same-input **Record again** attempt was ignored until the microphone selection changed;
- a focused regression test demonstrated that stopped, idle, and error capture owners must be replaceable even before
  their worker handle reports finished, while starting and recording owners remain protected;
- Rust now joins that already-inactive capture worker before opening the replacement stream;
- after the fix, the user confirmed two consecutive same-input recordings each reached a distinct final transcript
  without changing or refreshing the selected microphone.

This is narrow macOS hardware evidence. It does not establish transcription accuracy, acoustic latency, provider
cancellation completion, feedback suppression, or cross-platform behavior.

## Next bounded slice: remaining native voice acceptance

### Goal

Close only the remaining manual evidence gaps for the existing Milestone 7 workflow. Add code only if one of these
checks exposes a concrete product defect; use a focused regression test and the smallest fix.

### Acceptance boundary

1. Confirm **System default** capture if it was not part of the earlier input-switch check, plus visible live
   speech/silence state, one final-turn correction, and **Discard**.
2. Confirm transcript copying does not automatically send or create a conversation and leaves correction,
   delivery/retention, repeated copy, and Discard available until an explicit consuming action.
3. Exercise **Play response aloud**, **Stop local playback**, and one **Interrupt & record** attempt. Record only the
   endpoints actually observed; do not infer audible or provider completion from command acceptance.
4. Check the transcript action and feedback with ordinary keyboard navigation and VoiceOver if available. Keep user
   confirmation distinct from automated, screenshot, or visual evidence.

### Explicit exclusions

Do not add output-device selection, acoustic echo cancellation or feedback DSP, wake words, automatic listening or
playback, audio response blocks, generated-audio retention, persisted telemetry, analytics, provider-reported latency,
schema changes, release/update publication, protected workflow dispatch, or Microsoft Store work.

Microsoft Store certification and publication remain deferred until fresh release-owner notice. The earlier rejected
submission is not certification or publication evidence.

## Validation and worktree boundary

The completed branch checks on 2026-09-02 are:

- `npm run format:check`;
- `npm run check` with no errors or warnings;
- `npm test`: 236 passed and 3 intentionally skipped across 54 files;
- `npm run build`;
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`;
- `cargo check --manifest-path src-tauri/Cargo.toml`;
- `cargo test --manifest-path src-tauri/Cargo.toml`: 443 library tests and 1 updater-evidence test passed,
  33 intentionally ignored, and doc tests passed.

Run the standard checks before handing off another slice:

```sh
npm run format:check
npm run check
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

Bottie is not a Cargo workspace; run Cargo commands serially. A development-signed app launch or a compiled Rust test
binary is not proof that the Rust tests executed. The existing `block 0.1.6` future-incompatibility notice is unrelated.

The worktree contains unrelated untracked logo-kit, screenshot, and Linux signing-public-key files. Preserve them and
stage only reviewed paths.
