# Bottie handover

Last verified: 2026-10-30 (issues #214, #218, #220, #221 in PR #222, including its review fixes).

## Start here

1. Check the PR list first. As of 2026-10-30, PRs #212, #213, #215 and #219 are merged; PR #222
   (`fix/issues-214-218-220-221`) holds the #214/#218/#220/#221 fixes. Review follow-ups are filed as #223-#226.
2. **Next items:** the four open `FIX_BOTTIE.md` items. Each needs something this Mac session could not provide:
   - **9** (Linux `main.py` left in /tmp): confirm on Linux first, then let the parent own and delete the workspace.
   - **10** (MOSS CUDA hash lock): generate `--require-hashes` requirements on the DGX and prove the install there.
   - **14** (answer links): click a link in the running app first; if dead, add a scoped opener.
   - **19** (batch export memory): low priority; stream conversations into the ZIP writer.
   - Follow-up from #214 (#226): other spawners (Python, Nano, Kastellan) can still inherit pipe ends on macOS (see
     `SPAWN_LOCK` in `local_image_worker/transport.rs`). Only worth a crate-wide spawn helper if it shows up.
   Then run the follow-up audit listed under "Not audited yet".
3. Owed hardware re-checks (cannot run in a normal session):
   - Re-run the MOSS MLX (Mac) and CUDA (DGX) proofs to confirm item 5's minimal worker environment strips nothing a
     runtime needs (`docs/moss-image-local.md`).
   - Item 4 changed `mlx_worker.py`, but both shipped forms of it are frozen builds of the old bytes. Until each is
     rebuilt and re-pinned, the fix reaches no user:
     - the Mac managed Qwen-Image-2512 worker, a PyInstaller bundle pinned by executable/bundle hash in
       `model_package.rs` (`docs/local-image-model-package.md`);
     - the Linux clean-runtime image (`sha256:740816cb…`) and its closure record. Their source pins now name the new
       bytes, so rerun both DGX rebuilds and closures before relying on that evidence.

## Audit fixes done

2026-10-30, third session (issues #214, #218, #220, #221):
- **#214:** on macOS, std sets close-on-exec only after `pipe()`. A worker spawned in that gap held a sibling's pipe
  ends, so EOF never came and teardown failed. Transport and MOSS spawns now take `SPAWN_LOCK`. 29/29 parallel runs
  passed (was 3/6 failing).
- **#221:** `run_cancellation::reserve_then_write` reserves, then runs the first durable write. `start_chat` uses it,
  and two tests pin the helper's order and unregister-on-failure path (a `start_chat` call-site test is #223).
- **#220:** new `worker_pause.rs` (moved out of `semantic_indexer.rs`). A dead attachment or semantic worker still
  counts as paused for restore, but `WorkerHealth` records an error diagnostic once ("… stopped; restart Bottie"),
  also when a `wake()` finds the worker gone. Reindex refuses to clear vectors when the semantic worker is stopped.
- **#218:** exports and manual backups sweep their own stale (>1 h) staging in the target folder, but only plain
  files named `<prefix>-<uuid><suffix>` (`remove_stale_staging_files`), so user files with a similar name survive.
  When rotation succeeds, it reports staging it could not delete (or inspect) as a `warn` diagnostic; other
  sweep-failure reporting is #225.

2026-10-30, second session (issues #216, #217):
- `start_chat` registers its run through `run_cancellation::reserve_run` before the first durable write. Early returns
  drop the registration. Restore holds both run registries (`lock_idle_generation`) from its last check through
  the swap, so a run starting in that window waits instead of writing to the old store. A run that voice capture
  cancels before its task spawns now fails with "Voice capture interrupted this message" instead of a silent
  Cancelled.
- Tests added for the gaps #217 listed: per-adapter round-text cap and message truncation, the attachment-processor
  pause timeout (`pause_within`) and disconnect, and speech poll epochs. OpenAI's inline tests moved to
  `inference/openai/tests.rs`.
- Deleting or archiving a conversation discards an in-flight load of that conversation only; a load of another
  conversation started meanwhile still opens (`close` checks `openingConversationId`).

2026-10-30 (items 6-8, 11-13, 15-18, 20-22; details in `FIX_BOTTIE.md` status lines):
- Provider streams are bounded by `src-tauri/src/inference/limits.rs` (64 KiB tool arguments, 1 MiB frames, 4 MiB text
  per round, 64 KiB error bodies, 1 MiB discovery, 300-character provider messages). SSE handles bare CR and strips one
  space after `data:`.
- Exports, restore pauses (20 s busy error), staging sweeps (`storage/staging_sweep.rs`; rollback dirs kept on
  purpose), restore guards (image runs; cancelled runs stay registered until their final write), and private-address
  refusal for generated-image downloads.
- Frontend: stale conversation loads and speech polls are discarded; `npm test` excludes `.claude/**`.

2026-10-29 (items 1-5):

- **1. Saved-key leak:** connection tests read the saved credential only when the normalized draft endpoint equals
  the persisted one (OpenAI/Anthropic/Qwen base URL; Localmail origin + certificate pin). Otherwise the user must
  type a key.
- **2. Backup staging:** manual backups build and verify in `.bottie-backup-<uuid>.part` beside the destination, then
  rename over it. Automatic rotation shares the writer (`write_verified_backup`).
- **3. Missing blobs:** backups skip catalog files absent from disk and report the count. Rotation diagnostics show it,
  and a manual backup logs a `warn` diagnostic. Payload validation accepts a verified subset; it still rejects extra rows
  and wrong bytes. Re-attaching identical bytes restores a missing blob.
- **4. Late cancel:** the worker drops cancels for no active request, raises the interrupt under the lock, and ignores
  stray idle interrupts. The Linux source pins were updated; the frozen Mac and Linux builds still need rebuilding
  (see owed checks).
- **5. Worker environment:** `src-tauri/src/worker_env.rs` clears the environment for MOSS and Nano workers and
  re-adds an allowlist (PATH, HOME, temp, locale, `LD_LIBRARY_PATH`, CUDA vars, Windows system root).

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

## Validation (2026-10-30)

- Rust: `cargo test --locked` passes 803 library tests (42 ignored) plus the updater, adapter, and transport suites.
  `cargo fmt` is clean. Clippy still reports ~50 pre-existing style warnings (none are bugs).
- Over 500 lines: `storage/backup.rs` (517), `semantic_indexer.rs` (549). Split them when next touched.
- Python: `local-image-worker` was not touched this session (121 tests passed on 2026-10-29).
- Frontend: not touched this session (`npm test` passed 464 tests earlier on 2026-10-30).
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
