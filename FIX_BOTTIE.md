# Fix Bottie: audit todo list

These items come from a full-repo audit on 2026-10-28 at commit `dae91bf` on `codex/moss-image-backend`. The list is
ordered by urgency. Line numbers refer to that commit, so re-locate the code before editing. Tick an item when its fix
and its test are in place.

Each item gives the problem, the smallest fix, and a check that proves it. Keep fixes small and reuse the helpers that
are already named. Do not add layers or config.

The audit's baseline:
- Rust suite: 712 library tests pass (41 ignored).
- Frontend suite: 433 tests pass once `.claude/**` is excluded (see item 11).
- `svelte-check`: clean.

## Must fix

- [x] **1. Test connection sends the saved API key to any HTTPS URL**
  - **Status:** Done 2026-10-29: also covers the Localmail test (origin + certificate pin must match).
  - **Where:** `src-tauri/src/lib.rs:614-658` (`test_provider_connection`) and `src-tauri/src/credentials.rs:44-55`
    (`draft_or_saved_credential`).
  - **Problem:** With an empty key box, the saved vault key is used for whatever `draft.base_url` the WebView sends.
    `validate_remote_base_url` only requires https. A typo or a new host receives the real key. The doc comment on
    `draft_or_saved_credential` says it never reads the saved credential, which is the opposite of what it does.
  - **Fix:** Fall back to the saved key only when the normalized draft URL equals the persisted base URL for that
    provider (the `settings.<provider>_base_url` values). Otherwise return the existing "Enter an ... API key to test."
    error. Correct the doc comment.
  - **Test:** Saved key plus a different draft URL gives an error, and no saved-key read happens. Saved key plus the
    same URL still works.

- [x] **2. Manual backup destroys the previous backup on failure**
  - **Status:** Done 2026-10-29.
  - **Where:** `src-tauri/src/storage/backup.rs:51-73` (`backup_to`), called from `storage_commands.rs:143`.
  - **Problem:**
    - `backup_to` writes straight onto the user-chosen path.
    - If `embed_portable_payload` fails (for example ENOSPC), it calls `remove_database_files(destination)` and the old
      backup is gone.
    - If `verify_backup` fails, a bad file is left under the old name.
  - **Fix:** Write to a staging file in the same directory (`.bottie-backup-<uuid>.part`), embed and verify there, then
    `fs::rename` over the destination. Remove the staging file on any error. Automatic rotation
    (`rotate_automatic_backups_at`) already stages, so make it share this path rather than double-staging.
    `restore_from`'s safety copy (`backup.rs:196`) goes to a fresh path and keeps working.
  - **Test:** An existing destination file survives byte-identical when the embed step is forced to fail.

- [x] **3. One missing attachment blob blocks every backup, and re-attaching does not repair it**
  - **Status:** Done 2026-10-29: payload validation now accepts a verified subset; manual backups log a `warn` diagnostic.
  - **Where:** `src-tauri/src/storage/portable_backup.rs:71-73`, `:83-90` and `:101-103` (`verified_file` failure
    aborts), and `src-tauri/src/storage/attachments.rs:125-130` (`commit_blob` duplicate branch).
  - **Problem:**
    - A blob can go missing: antivirus quarantine, or a legacy restore that keeps the database rows but not the files.
    - Every manual backup then fails, and `rotate_automatic_backups` fails on every launch.
    - Re-attaching the identical file returns the existing row (`duplicate: true`) and throws away the freshly verified
      temp file. The missing blob stays missing.
  - **Fix:**
    - In `commit_blob`'s duplicate branch, rename the verified temp file into `attachment_blob_path` when that path is
      missing or the wrong size.
    - In `embed_portable_payload`, skip rows whose file is absent and report the skip count, at least for automatic
      backups.
  - **Test:**
    - Delete one blob, then confirm automatic backup still succeeds and reports one skip.
    - Re-ingest the same bytes, then confirm the blob file exists again.

- [x] **4. The MLX image worker crashes on a late Cancel**
  - **Status:** Done 2026-10-29: Linux source pins re-pinned; the frozen Mac 2512 bundle and Linux DGX image still need rebuilding.
  - **Where:** `local-image-worker/mlx_worker.py`:
    - `:225-229`: a cancel that does not match the active request is queued.
    - `:217-218`: an unknown command raises `ValueError`.
    - `:92-99`: `interrupt_main` is called after the lock is released.
    - `:491-496`: `main()` has no handler.
  - **Problem:**
    - A cancel that arrives just after `finish()` goes onto the command queue. The run loop raises
      `ValueError("unsupported command")` and the worker exits, while the host still believes it is loaded.
    - Separately, `interrupt_main()` can fire after `finish()` has already run. The `KeyboardInterrupt` then lands in
      `self._commands.get()` or in the middle of a frame write.
  - **Fix:** In `Worker.run`, add `elif command_type == "cancel": continue`. Catch `KeyboardInterrupt` around the loop
    body and continue. The cleaner option is to stop queueing stale cancels in `_read_commands` and drop them there.
  - **Test (in `local-image-worker/test_mlx_worker.py`):**
    - A cancel sent after a completed generate leaves the worker alive.
    - A following generate succeeds.

- [x] **5. The MOSS image and Nano speech workers inherit the full app environment**
  - **Status:** Done 2026-10-29 in `src-tauri/src/worker_env.rs`; real-hardware MOSS MLX/CUDA re-proof still owed.
  - **Where:** `src-tauri/src/local_image_worker/moss/runtime.rs:153-187` and
    `src-tauri/src/speech/nano/process.rs:34-46`.
  - **Problem:** These spawns only remove `PYTHONPATH` and `PYTHONHOME`. Variables like `ANTHROPIC_API_KEY`,
    `HF_TOKEN` and proxy credentials from the launching shell reach third-party ML code (torch, diffusers with remote
    code, ONNX, a Swift binary). Their only network guard is a Python audit hook, and the Swift binary has none.
  - **Fix:** Add `.env_clear()` and re-add only what is needed:
    - PATH and HOME.
    - The CUDA or loader variables the runtime needs.
    - SYSTEMROOT on Windows.
    - The `HF_HUB_OFFLINE`, `TRANSFORMERS_OFFLINE` and `HF_HUB_DISABLE_TELEMETRY` flags already set.

    Follow the existing pattern in `local_image_worker/transport.rs:198` and `python_process_transport.rs:33`.
  - **Test:** Spawn a fixture worker with a secret variable set in the parent and confirm the child does not see it. On
    real hardware, re-run the MOSS MLX and CUDA proofs to confirm nothing needed was stripped.

## Should fix

- [ ] **6. A hostile or buggy OpenAI-compatible server can make memory grow without limit**
  - **Where:**
    - `src-tauri/src/inference/openai/protocol.rs:368-373` (`OpenAiToolCallAccumulator::extend`).
    - `src-tauri/src/inference/sse.rs:13-16` and `:42-56` (`SseDecoder`).
    - `src-tauri/src/inference/ollama/protocol.rs:429-446` (NDJSON decoder).
    - Streamed content and reasoning: `openai.rs:242-249`, `ollama.rs:194-200`, `omlx.rs:220`, `anthropic.rs:166`.
  - **Problem:**
    - Tool-call `name` and `arguments` grow with no cap. Anthropic caps arguments with `MAX_STREAMED_ARGUMENT_BYTES`
      (64 KiB) in `anthropic/protocol/response.rs:16,196`.
    - The SSE and NDJSON buffers grow with no cap when no frame boundary arrives, and are rescanned from the start on
      every chunk (quadratic work).
    - Per-round content has no total cap.
    - The idle timeout resets on every chunk, so it never fires while data keeps coming.
  - **Fix:**
    - Move the 64 KiB argument cap to a shared constant and enforce it in `extend`. Also cap `tool_name` at 128.
    - Return `ProviderError::malformed` when a decoder buffer exceeds about 1 MiB with no boundary.
    - Start the boundary scan at `old_len.saturating_sub(3)`.
    - Add a per-round content cap of about 4 MiB.
  - **Test:**
    - Oversized streamed arguments return a malformed error.
    - A never-terminated SSE frame larger than the cap returns a malformed error.

- [ ] **7. Error and discovery response bodies are unbounded, and provider error text is forwarded verbatim**
  - **Where:**
    - Error bodies: `openai.rs:290`, `anthropic.rs:282`, `ollama.rs:358`, `omlx.rs:362`.
    - Discovery bodies: `openai.rs:219`, `anthropic.rs:210`, `ollama.rs:128,150`.
    - Message forwarding: `openai.rs:295-299` and the equivalents in the other adapters.
  - **Problem:** A large body is read fully into memory. `omlx.rs:174` already caps discovery, so the rule is applied
    unevenly. The provider's `error.message` reaches the WebView whole, and some providers echo part of the API key in
    it.
  - **Fix:** Read through a capped reader: reuse `web_fetch`'s `read_bounded_body`, with about 64 KiB for errors and
    about 1 MiB for discovery. Truncate the forwarded provider message to about 300 characters.
  - **Test:** An oversized error body yields a bounded, truncated `ProviderError`.

- [ ] **8. Overwriting an export can lose both the old and the new file**
  - **Where:** `src-tauri/src/storage/portable_export.rs:44` and `:53-56` (`write_to`).
  - **Problem:**
    - The ZIP path removes the old file before renaming. If the rename fails, the cleanup deletes the staging file too,
      so both copies are lost.
    - The plain path's `fs::write` truncates the target in place.
  - **Fix:**
    - Delete lines 53-56; `fs::rename` replaces the target on both unix and Windows.
    - For plain documents, write `.bottie-export-<uuid>.tmp` in the same directory and rename it.
  - **Test:** An existing export target is replaced, and a forced write failure leaves the old file intact.

- [ ] **9. The Linux Python sandbox likely leaves each run's `main.py` in /tmp (not yet confirmed on Linux)**
  - **Where:** `python-runner/src/lib.rs:198-203` (`execute_with_host_boundary`) and
    `python-runner/src/linux_containment.rs:229-236` and `:266-272` (Landlock rights).
  - **Problem:** The `TempDir` is dropped after `restrict_self`. The ruleset handles REMOVE_FILE and REMOVE_DIR but
    grants only READ rights on the workspace, so the drop's delete fails silently. Each run leaves model-written code
    (which may include user data) on disk.
  - **Fix:** Have the parent (`LinuxContainedPythonRunner` / `execute_private_pipe_process`) create the workspace and
    pass it in, then delete it after `child.wait()`.
  - **Test (Linux-only, ignored elsewhere):** After one contained execution, the workspace directory no longer exists.
    First confirm the bug on Linux.

- [ ] **10. MOSS CUDA packages are installed without hash checks**
  - **Where:** `scripts/moss-image/install.py:110-124` and `:142-147`, and
    `src-tauri/src/local_image_worker/moss/runtime.rs:91-95` and `:134-151`.
  - **Problem:**
    - `pip install torch==X` and `-r requirements-cuda.txt` pin versions, not hashes.
    - The CUDA config's `artifacts` list is empty, so native verification never covers venv `site-packages`. Model files
      are hashed on every run, while the code beside them is not.
  - **Fix:**
    - Generate a hash-locked requirements file and install with `--require-hashes`.
    - Optionally record the venv `site-packages` files as artifacts, as the MLX path does.
  - **Test:** A requirements line with a wrong hash makes the install fail.

- [ ] **11. `npm test` fails because vitest collects `.claude/worktrees/**`**
  - **Where:** the `test` script in `package.json`.
  - **Problem:** Stale worktree copies produce 22 failed files. The real suite is green.
  - **Fix:** Add `--exclude ".claude/**"` to the `test` script. Also check the `performance:frontend` and `python:bundle:test`
    scripts.
  - **Test:** `npm test` passes on this machine with the worktrees present.

- [ ] **12. Restore, reindex and restore-latest can hang behind the embedding-model download**
  - **Where:** `src-tauri/src/semantic_indexer.rs:238-246` (`pause`) and `:296-328` (`FastEmbedder::load` inside an
    iteration), and `src-tauri/src/storage_commands.rs:155` and `:203-207`. The same unbounded wait is in
    `attachment_processor.rs:66-74`.
  - **Problem:** `pause()` blocks on `acknowledged.recv()` with no timeout while `storage_management` is held. A
    first-run or stalled model download blocks the restore, and backup and attachment ingest queue behind it.
  - **Fix:** Use `recv_timeout` in `pause()` and return a retryable "busy, try again shortly" error to the caller.
  - **Test:** A worker that never acknowledges makes `pause()` return a timeout error within the bound.

- [ ] **13. Killed automatic backups leave full-size staging copies forever**
  - **Where:** `src-tauri/src/storage/backup.rs:101-113` and `:263-277` (`managed_backups` ignores them).
  - **Problem:** If the app is killed mid-backup, the `.bottie-auto-staging-*` files are never swept, and each one holds
    the database plus all blobs. The same applies to `.bottie-restore-staging-*`, `.bottie-attachment-restore-*` and
    `.bottie-attachment-rollback-*` in app data.
  - **Fix:** At the start of `rotate_automatic_backups_at`, delete `.bottie-auto-staging-*` files older than one hour.
    Add the same sweep for the restore and attachment staging prefixes at startup, before workers start, next to the
    attachment GC.
  - **Test:** An old staging file is removed by rotation, and a fresh one (less than an hour old) is kept.

- [ ] **14. Clicking links in answers, citations and the license disclosure probably does nothing (needs a runtime
  check)**
  - **Where:** `src/lib/markdown.ts:71`, `src/lib/ContextPanel.svelte:240` and
    `src/lib/QwenResearchLicenseDisclosure.svelte:6`. `src-tauri` has no handler: no opener plugin, `on_new_window` or
    `on_navigation`.
  - **Problem:** `target="_blank"` links have nothing to open them in a Tauri webview.
  - **Fix:** First click a link in the running app to confirm. Then add `tauri-plugin-opener` with a scoped
    `opener:allow-open-url` for http, https and mailto, or an `on_new_window` handler that opens http(s) URLs only.
    Keep the existing http/https/mailto allowlists in `markdown.ts` (`validateLink`) and `web-provenance.ts`
    (`safeWebLocation`).
  - **Test:** Manual click-through on macOS, plus a capability check that only these schemes are allowed.

- [ ] **15. Opening two conversations quickly can show the wrong one**
  - **Where:** `src/routes/conversation-state.svelte.ts:99-108` (`open`), called from
    `src/routes/page-state.svelte.ts:370-379`.
  - **Problem:** `open()` has no latest-request guard, while `search()` does. If A's `loadConversation` resolves after
    B's, A is shown while B is selected.
  - **Fix:** Use the same monotonic sequence guard `search()` uses, and discard stale results.
    `selectConversationBranch` probably needs the same guard.
  - **Test:** In `conversation-state` tests, resolve A after B and confirm B stays applied.

- [ ] **16. Small UI timing races**
  - **Where:** `src/routes/page-state.svelte.ts:1010-1019` (`startNewChat`) and
    `src/routes/speech-state.svelte.ts:98-112` (`refresh`), with `stop()` at `:71-81`.
  - **Problem:**
    - `generationRun += 1` happens after `await speech.stop()` and `await history.startNew()`, so a late image event can
      land in the new blank chat.
    - A `refresh()` poll that was in flight when Stop was pressed overwrites idle with `speaking`, and nothing corrects
      it.
  - **Fix:**
    - Move the `generationRun` bump and state resets above the awaits.
    - Add a `pollEpoch` counter, incremented in `stop()` and `startPolling()`. `refresh()` ignores results from an older
      epoch.
  - **Test:** One unit test per race, using deferred promises.

## Nice to have

- [ ] **17. The native restore guard ignores image runs and cancelled-but-unfinished chat runs**
  - **Where:**
    - Restore guards: `src-tauri/src/storage_commands.rs:156`, `:194`, `:226` and `:248`.
    - Cancellation and run registration: `src-tauri/src/run_cancellation.rs:28`, `src-tauri/src/generation.rs:56` and
      `:319-325`.
  - **Problem:**
    - The guard checks only `state.runs`.
    - `cancel_chat` removes the run before its final `finish_provider_run` write.
    - A new run is invisible between `start_provider_run` and `runs.insert`.

    The toolbar disables Restore while `isGenerating`, so only a direct command call or a tight Stop-then-Restore can
    hit this.
  - **Fix:**
    - Also check `state.image_runs.is_active().await`, as `lib.rs:258` already does.
    - Leave the run in `runs` until the task finishes its final write. `cancel_chat` should only signal cancellation.

- [ ] **18. Generated-image downloads have no private-address check**
  - **Where:** `src-tauri/src/image_generation/download.rs:92-106`.
  - **Problem:** A URL from the provider's response is fetched after only https and userinfo checks, with the system
    proxy applied.
  - **Fix:**
    - Reuse `web_fetch`'s `resolve_addresses` and `is_public_ip`, with `resolve_to_addrs`.
    - Set `no_proxy`.

- [ ] **19. Batch JSON export holds every conversation in memory**
  - **Where:** `src-tauri/src/storage/export.rs:79-95` (`prepare_batch_json_export`) and `bundle_export`.
  - **Fix:** Stream each conversation into the ZIP writer instead of building one `String`. This is low priority for a
    single-user app.

- [ ] **20. Clippy: a test fixture is loaded as a module twice**
  - **Where:** `src-tauri/src/local_image_worker/managed_install/` includes
    `../../local_image_worker_model_download_tests/fixtures.rs` a second time.
  - **Fix:** Import the existing module with `use`; do not re-`mod` it.
  - **Optional:** Run `cargo clippy --fix` for the other style warnings (about 50, none of them bugs).

- [ ] **21. The SSE decoder does not handle bare-CR line endings**
  - **Where:** `src-tauri/src/inference/sse.rs:42-70`.
  - **Problem:** Frames ending only in `\r\r` never complete.
  - **Fix:** Normalize `\r\n` and `\r` to `\n` before looking for a boundary. Do this with item 6.

- [ ] **22. The SSE decoder strips too much whitespace from `data:` values**
  - **Where:** `src-tauri/src/inference/sse.rs:42-70`.
  - **Problem:** `trim_start` removes all leading whitespace; the spec removes one space.
  - **Fix:** Use `strip_prefix(' ')`.

## Minor notes (optional)

- `web_fetch`'s IPv6 block list omits `64:ff9b:1::/48` (local-use NAT64). It is only reachable through a NAT64
  gateway.
- The Python WASI epoch deadline (`python-runner/src/lib.rs:375-387`) cannot interrupt a blocking host call, such as a
  guest `time.sleep(1e9)`. The 45 s outer kill in `python_process_transport.rs:21,73` bounds it, so the result is
  `HelperFailed` instead of `TimedOut`. Consider mapping that case to `TimedOut`.
- The model cache has no lock across processes, so the dev binary `local_image_model_fetch` and the app could both
  write it at once. Promotion re-hashes every file, so corruption cannot be promoted. This only matters if
  `ManagedModelInstaller` gets wired into the app.
- The local file `bottie-linux-signing-private.asc` in the repo root is mode 644. It is gitignored and untracked; run
  `chmod 600` on it.

## Not audited yet

A follow-up audit should cover:
- **Storage:** the search, memory, branching, retention and lifecycle modules, and the migration SQL.
- **Native tools:** `credentials*.rs`, `tool_contract/**` argument validation, `generation*.rs` cancellation paths,
  `web_fetch/extraction.rs`, and the Ollama, oMLX and Anthropic tool-call decoding.
- **Workers:** microphone and system speech, the diffusers worker scripts, the Swift, Windows AppContainer and macOS XPC
  code, and the release and packaging scripts in `scripts/`.
