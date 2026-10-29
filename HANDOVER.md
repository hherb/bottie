# Bottie handover

Last verified: 2026-10-29 (FIX_BOTTIE items 1-5 on `fix/bottie-audit-1-5`).

## Start here

1. Check the PR list first. As of 2026-10-30, PRs #212 (MOSS/Qwen 2.1 backends) and #213 (audit items 1-5) are
   merged into `main`.
2. **Next item:** `FIX_BOTTIE.md` items 6 onward ("Should fix"), in order. Re-locate code before editing; line numbers
   refer to the audit commit `dae91bf`.
3. Owed hardware re-checks (cannot run in a normal session):
   - Re-run the MOSS MLX (Mac) and CUDA (DGX) proofs to confirm item 5's minimal worker environment strips nothing a
     runtime needs (`docs/moss-image-local.md`).
   - Item 4 changed `mlx_worker.py`, but both shipped forms of it are frozen builds of the old bytes. Until each is
     rebuilt and re-pinned, the fix reaches no user:
     - the Mac managed Qwen-Image-2512 worker, a PyInstaller bundle pinned by executable/bundle hash in
       `model_package.rs` (`docs/local-image-model-package.md`);
     - the Linux clean-runtime image (`sha256:740816cb…`) and its closure record. Their source pins now name the new
       bytes, so rerun both DGX rebuilds and closures before relying on that evidence.

## Audit fixes done (2026-10-29)

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

## Validation (2026-10-29)

- Rust: `cargo test --locked` passes 719 library tests (41 ignored) plus the updater, adapter (6), and transport (10)
  suites. `cargo fmt` is clean.
- Python: all 121 `local-image-worker` tests pass.
- Frontend: 433 tests pass (3 skipped) with `.claude/**` excluded. Only `website/tests/rendered-html.test.mjs` fails,
  because it needs a built `website/dist`.
- Prettier reports 24 pre-existing unformatted files (docs JSON, website config, `FIX_BOTTIE.md`); none were touched
  here.

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
