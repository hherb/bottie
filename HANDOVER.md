# Bottie handover

Last verified: 2026-09-02

## Start here

PR #129 is merged at `daf09d6`. The current branch is `codex/remaining-native-voice-acceptance`; its draft PR is to be
opened after final validation. Milestones 0–7 remain complete. Acoustic feedback processing, release publication, and
Microsoft Store certification/publication remain deferred until fresh release-owner direction.

Read, in order:

1. `HANDOVER.md`
2. the Milestone 7 section of `ROADMAP.md`
3. the Local voice capture section of `README.md`
4. `CONTRIBUTING.md`

## Completed slice

The user completed the remaining native macOS voice acceptance on 2026-09-02 and reported that the checked workflow
worked: System default capture, visible speech/silence state, final-turn correction, transcript copying without an
automatic send or conversation creation, repeated copy, Discard, local playback/stop, Interrupt & record, keyboard
navigation, and VoiceOver feedback. Treat this as narrow user-observed macOS evidence, not transcription-accuracy,
acoustic-latency, feedback-suppression, provider-cancellation-completion, or cross-platform evidence.

The follow-up preference and Settings slice now:

- retains the existing durable provider/model restore and deterministic unavailable-model fallback;
- keeps WebView microphone and speech selections process-local while Rust derives separate stable opaque preference
  keys and native identities remain outside IPC;
- stores only those Rust-owned opaque local-audio preference keys in native `local-audio.json` configuration;
- discovers microphones at startup without opening an input or requesting permission, restores the last exact choice
  when available, and persists System default when the saved device is unavailable;
- restores the last speech voice when available and persists the default available local voice when it is not;
- moves the speech-voice selector from the conversation into Settings while leaving playback status and explicit
  Play/Stop actions with the response.

The current native development restart preserved the same settled opaque microphone and speech choice file. The
1280×720 browser fixture review showed the voice selector in Settings with its on-device, automatic-save, and fallback
disclosures; the conversation no longer exposes a duplicate selector.

## Validation

Focused validation passed:

- 17 frontend Settings, conversation, and microphone-control tests;
- 12 speech tests passed and one host-engine test was intentionally ignored;
- all three microphone-device tests;
- the local-audio preference save/reopen and malformed-token fallback test;
- `npm run check` with no errors or warnings;
- native development launch and one settled restart with unchanged opaque choice-file SHA-256.

The full standard suite passed:

```sh
npm run format:check
npm run check
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

Specifically, `npm test` passed 236 tests with three intentional skips across 54 files. Rust passed 446 library
tests with 33 intentional ignores, the updater-evidence test, and doc tests. `npm run check` reported no errors or
warnings. Formatting, the production build, and `cargo check` also passed.

Bottie is not a Cargo workspace; run Cargo commands serially. A development-signed app launch is not proof of audible
playback, microphone behavior, or executed Rust tests. The existing `block 0.1.6` future-incompatibility notice is
unrelated.

## Next bounded action

For this draft PR, inspect exact-head CI and address only concrete review findings with focused regression coverage.
If it has merged, do not invent another voice, persistence, release, updater, DSP, or Store slice: wait for explicit
product/release-owner direction.

Microsoft Store certification and publication remain deferred until fresh release-owner notice. The earlier rejected
submission is not certification or publication evidence.

The worktree contains unrelated untracked logo-kit, screenshot, and Linux signing-public-key files. Preserve them and
stage only reviewed paths.
