# Bottie handover

Last verified: 2026-11-26 (branch `fix/issues-258-259-260`).

## Start here

1. Check the PR list first. As of 2026-11-26, PRs up to #257 are merged; the #258/#259/#260 fixes are in review
   (#262). Land all work through PRs.
2. **Next items.** None of the four open `FIX_BOTTIE.md` items can be done on this Mac alone:
   - **9** (Linux `main.py` left in /tmp): confirm on Linux first, then let the parent own and delete the workspace.
   - **10** (MOSS CUDA hash lock): generate `--require-hashes` requirements on the DGX and prove the install there.
   - **14** (answer links): click a link in the running app first; if dead, add a scoped opener.
   - **19** (batch export memory): low priority; stream conversations into the ZIP writer.

   Doable here: #240 (voice test gaps), #247, #261 (split `page-state`/`conversation-state`), and the follow-up
   audit in `FIX_BOTTIE.md` "Not audited yet".
   #226's last bullet (spawners outside `SPAWN_LOCK`) is only worth doing if it shows up in practice.

3. **Hands-free voice conversation mode:** phase 0 is merged (#241). Still owed: a real-device run, especially with
   Bluetooth input, to check endpoint timing, word loss under a slow first `speak`, and the new Silero gate. #237
   (stream recovery reports success) needs that device too.
4. **Kastellan:** `@kastellan:` runs through `kastellan-cli session` (#236). It has not been clicked through in the
   running app yet; do that before relying on the UI.
5. Owed hardware re-checks (cannot run in a normal session):
   - Re-run the MOSS MLX (Mac) and CUDA (DGX) proofs to confirm item 5's minimal worker environment strips nothing a
     runtime needs (`docs/moss-image-local.md`).
   - Item 4 changed `mlx_worker.py`, but both shipped builds of it are frozen from the old bytes. Until each is rebuilt
     and re-pinned, the fix reaches no user:
     - the Mac managed Qwen-Image-2512 worker, a PyInstaller bundle pinned by hash in `model_package.rs`
       (`docs/local-image-model-package.md`);
     - the Linux clean-runtime image (`sha256:740816cb…`) and its closure record. Their source pins now name the new
       bytes, so rerun both DGX rebuilds and closures before relying on that evidence.

## Audit fixes done

2026-11-26 (#258, #259, #260):

- **#259:** while no retained automatic snapshot is complete, an expired snapshot whose completeness check fails is
  kept, not pruned. `hold_back_from_expiry` keeps the newest complete one plus every newer unreadable one.
  `AutomaticBackupRotation.check_failed` counts them and raises the report to `warn`. The inline unit tests moved
  verbatim to `storage/automatic_backup/tests.rs`.
- **#258:** backup failures keep a path-free cause. `StorageError::backup_sqlite` keeps only SQLite's code text
  ("Error code 13: database or disk is full") and drops its free text, which can hold a path. `backup_because`
  carries an `io::ErrorKind`. Covered: the snapshot copy, payload embedding, verification, reading a file into the
  payload, the manual rename and the rotation's clock read.
- **#260:** a Settings restore whose reload fails clears the stale view (`PageState.restoreBackup`). The pre-restore
  conversation, branch, attachments and list are dropped, so nothing acts on a conversation the restored data may
  lack.

2026-11-26 (#254, #255, #256):

- **#255:** a failed automatic backup names its step and cause in diagnostics
  (`StorageError::automatic_backup_during`). Folder creation, folder listing and the rename into place carry the
  `io::ErrorKind`. A failed snapshot write carries the inner (path-free) `StorageError` message. That message is still
  generic, because `write_verified_backup` maps its SQLite errors away; this is shared with manual backup (#258).
- **#254:** when a restore replaces the data but reloading it fails, `ConversationState.restoreReloadFailed` is set and
  the note adds "Relaunch Bottie to check again". `RecoveryState.restore` then sets `recheckFailed`, which locks both
  restore buttons, as #253 did for a failed re-check. The normal-mode restore still keeps the stale view (#260).
- **#256:** the flake was not a missed cancel window. `Transport` comes only from `send()` failing, so the fixture
  server had stopped listening: its 3 s accept timeout ran out during fsync-heavy setup under load. Reproduced with a
  3.2 s setup delay. The accept timeout is now 30 s (a hang guard only). The one test whose client may never connect
  calls `download_fixtures::release_unused_server`, and the `local_image_worker` tests went from 3.8 s to 1.4 s.

2026-11-25 (#235): every automatic-backup rotation now prunes expired snapshots, not only one that creates a
snapshot, so a snapshot an earlier launch failed to remove is retried and reported (`warn`) at the next launch. The
report names the first failure's `io::ErrorKind` (`AutomaticBackupRotation::report`). On the recovery screen, a failed
status re-read after a restore no longer rejects unhandled: it keeps the screen, adds an amber note, and turns restore
off until relaunch (`RecoveryState.recheckFailed` → `StorageRecovery` `locked`). `StorageRecovery` takes the `warning`
prop. Follow-ups: #254 (the same lock for a failed reload after restore) and #255 (snapshot-creation errors drop their
`io::ErrorKind`).

2026-11-25 (#249): the feature-gated `bottie-local-image-model-fetch` and `-runtime-proof` bins compile again. The
`local_image_worker` tree they include by `#[path]` no longer reaches crate-root modules the bins lack:
`QWEN_IMAGE_PROVIDER_ID` now lives in `local_image_worker/backend.rs` (re-exported by `image_generation`), the
download fixtures moved to `local_image_worker/download_fixtures.rs`, and the MOSS request-validation assertions moved
to `image_generation/tests.rs`. The bins declare `worker_env`. The Linux packaging smoke workflow now runs
`cargo check --all-targets --features local-image-runtime-proof`, so the bins cannot drift unnoticed again.

2026-11-25 (#232, #234): the local image execution boundary keeps its causes. `ModelUnavailable`,
`GenerationFailed` and `ShutdownFailed` carry the cache or transport error, and `InvalidLayout` and `InvalidOutput`
carry the shutdown failure when the worker then refuses to stop. A cancel that did not end cleanly still reads as
cancelled, but it is logged at `warn` with its cause (`CancelledAfterFault`). A failed cancel never leaves a live
worker behind: the transport tears down and the session is replaced. `runtime.shutdown()` no longer reports a
false `ShutdownFailed` for a worker that already stopped.

2026-11-25 (#246): `CaptureState` and its impl moved verbatim to `microphone/capture_state.rs`, with `pub(super)` on
its fields and methods. `microphone.rs` went from 713 to 326 lines; the test count is unchanged (919).

2026-11-25 (issues #238, #239; #242 closed as already fixed by #236):

- **#238:** `vad_silero::LoadPerCapture` retries a failed Silero load once per new capture, both in the recognizer
  (`WhisperTranscriber::prepare_vad`, lazily on the first job) and in the release gate. Each worker keeps its own last
  failure. `MicrophoneStatus.speechDetectionErrorCode` reports the gate's failure (conversation mode only), or else
  the recognizer's. Push-to-talk recording feedback adds "Silence filter unavailable", and the conversation listening
  line adds "silence filter unavailable". A cached model that fails its digest is deleted
  (`pinned_assets::verify_cached_file`), so the retry downloads it again; hf-hub never re-fetches a cached file. A
  gate inference error keeps the last verdict instead of reading as silence. Not yet reported to the user: inference
  failures after a successful load, and dropped VAD frames (#247).
- **#239:** `VoiceActivityDetector` counts the frames it drops from a callback over 64 frames (1.28 s).
  `take_frame_activities` returns that count, and the capture path adds it to the frame clock.
- The pinned-model gate moved verbatim to `microphone/pinned_assets.rs` (`transcription.rs` went from 558 to 434
  lines).
- #242 (`interpret_output` parking-line forgery) no longer applies: #236 removed that path, and a parked task now
  arrives as the typed `task.awaiting_operator` notification.

2026-11-07 (review of `feat/voice-mode-phase0`, merged as #241): streaming speech no longer drops words or hangs,
conversation state cannot be overwritten by a late reply, both endpointer inputs use the audio frame clock, and the
frontend recovers from failed polls and microphones. Details are in #241 and `docs/voice-mode-*`; open items are #240.

Earlier (2026-10-29 to 10-31): FIX_BOTTIE items 1-8, 11-13, 15-18, 20-22 and issues #214, #216-#218, #220,
#221, #223-#226 (bullets 1-2), #228, #230, #231. Their details are in `FIX_BOTTIE.md` status lines, the PRs (#213-#233)
and git history.

## Current state (summary; details in docs and git history)

- **Chat/providers:** oMLX, Ollama, OpenAI- and Anthropic-compatible remotes. Generation limits are in Settings (default
  12 rounds / 24 calls / 8,192 output tokens). `@kastellan:` requests go to a Kastellan agent (`docs/kastellan.md`).
- **Credentials:** one macOS keychain item, with session unlock by Touch ID or password. Localmail takes `lmk_` API
  keys, and paged PDF text works.
- **Composer:** Chat and Image tabs. Image has size tiers × five aspect ratios, and drafts survive tab switches.
- **Images:** hosted Qwen-Image-2.0; local Qwen-Image-2.1 through MLX Serve (`docs/qwen-image-2.1-local.md`); optional
  MOSS-Image on Apple MLX and NVIDIA CUDA (`docs/moss-image-local.md`). Base variants, Windows, lower-memory offload,
  and native Tauri UI clicks are unqualified.
- **Speech:** OS voices plus optional MOSS-TTS-Nano Ava/Adam (`docs/moss-nano-integration.md`).
- **Packaging:** development-signed contained-Python arm64 DMGs only. No notarization or production release.
- **Linux image runtime (frozen, Roadmap 8.4):** two offline ARM64 rebuilds agree. The closure is complete
  (`docs/local-image-linux-clean-runtime-closure-review.json`). 66 licence-only blockers remain, and `licenseReviewed`,
  `assemblyEligible`, and `distributionReviewed` are false. No bundle assembly, profile, signing, or release exists.

## Validation (2026-11-26)

- Rust: `cargo test` passes 928 library tests (42 ignored; the feature build was not rerun) plus the
  updater, adapter and transport suites. `cargo fmt` is clean. `cargo clippy --all-targets` prints 71 `warning:` lines,
  the same as `main` (style lints, none new).
- Over 500 lines: 16 Rust non-test files, all over the cap before this work. The largest are `lib.rs` (1226),
  `speech/streaming.rs` (983) and `microphone/endpointing.rs` (670). Frontend: `page-state.svelte.ts` (1222),
  `conversation-state.svelte.ts` (589), `storage.ts` (574), `ProviderSettingsDialog.svelte` (539) and `+page.svelte`
  (514). Split each one when you next touch it. The two state classes are #261.
- Frontend: `npm test` passes 543 tests; `npm run check` is clean.
- Python: `local-image-worker` was not touched (121 tests passed on 2026-10-29).
- Prettier reports pre-existing unformatted files (docs JSON, website config, `FIX_BOTTIE.md`).

## Linux runtime next slice (blocked on external input)

Complete independent expression review for the exact clean closure without assembling a bundle:

1. Obtain a complete independent review record for all 62 component identities, including reviewed expressions for
   Jinja2, safetensors, tokenizers, and Triton. Do not infer expressions from package metadata.
2. TDD a clean-profile-only two-trace review intake bound to the exact image, lock, traces, source archives,
   licence/notice bytes, and per-component review evidence. Keep the NGC review route intact.
3. Rerun both closures. Keep the output only if every expression and review byte agrees and no licence blocker
   remains; then `licenseReviewed` and `assemblyEligible` become true, and `distributionReviewed` stays false.

Stop if the review record is unavailable. Do not author review conclusions, assemble or publish bytes, accept a
Linux profile, or do signing, release, Store, updater, provider, or legal-terms work.
