# Bottie handover

Last verified: 2026-10-18 for MOSS backend integration checks; live MOSS MLX qualification is in progress.

## Start here

The owner requested MOSS-Image generation using Apple MLX or otherwise NVIDIA CUDA on 2026-10-18. The optional
backend is implemented on `codex/moss-image-backend`: explicit setup, pinned Base/SFT snapshots, private one-request
workers, layout prompts, existing image UI/storage, MOSS provenance, and exact-model retries. The Swift bridge builds
on the current Mac after installing the missing official Metal toolchain. Model acquisition and real MLX proof remain
in progress; do not claim CUDA or native Tauri UI qualification. See `docs/moss-image-local.md` for setup and bounds.
The completed Nano speech integration remains intact. Retain the independent frozen Linux image-runtime continuation
below; the MOSS request does not approve or reopen its distribution gates.

Start from the draft PR for `codex/linux-clean-runtime-license-sources` after it merges. Read `ROADMAP.md` Milestone
8.4 and the clean-runtime sections of `docs/local-image-linux-nvidia-proof.md` and
`docs/local-image-linux-worker-bundle.md`.

## Current state

- Two exact offline Linux/ARM64 rebuilds still agree across 63 Python identities, 112 Debian identities, and 27,035
  normalized regular files. The accepted rebuilt image is
  `sha256:740816cb8f348aa26e3d32f73b15b86b7f5a7228cff7b24d6910ee7aa0ee12a4`.
- The clean-runtime profile now accepts only the exact SentencePiece 0.2.2 and tokenizers 0.23.2 authoritative source
  archives. It requires both archives, exact component identities, archive/member sizes and hashes, and rejects partial
  input, licence-review manifests, the legacy NGC native-source set, image drift, lock drift, trace drift, and source
  drift.
- Collection is read-only, non-root, capability-free, and offline. A bounded in-process little-endian AArch64 ELF
  reader closes `NEEDED` and `SONAME` edges without adding `readelf` or changing the frozen runtime.
- The two independent collections agree on 161 files totalling 4,039,257,278 bytes, including 114 ELF files and 62
  components. `runtimeFilesSha256` is `f0f3e2dabe8777609f479ef81b3542e966f312a528183afda3f07368d43424d4`.
- `docs/local-image-linux-clean-runtime-closure-review.json` is the 23,223-byte path-free record with SHA-256
  `ebbca8034178339e6ba6007ad244ace868157a67f715dcd0628926444bd806fb`. `closureComplete` is true.
- No missing licence-byte blocker remains. Exactly 66 licence-only blockers remain: Jinja2, safetensors, tokenizers,
  and Triton have undeclared expressions, and all 62 component expressions are unreviewed. `licenseReviewed`,
  `assemblyEligible`, and `distributionReviewed` remain false.
- No licence-review manifest, bundle assembly, accepted Linux profile, hardware gate, app execution/UI, signing,
  release, Store, updater, provider, or legal action exists.

## Validation

- All 118 local image-worker Python tests pass, including exact source selection/completeness, profile/lock/trace
  rejection, canonical package identities, in-process AArch64 ELF parsing, legacy NGC behavior, and agreement.
- Python byte-compilation and Black checks pass; every changed implementation is below 500 lines.
- Prettier, Svelte diagnostics, all 407 active frontend/script tests (3 skipped), and the production build pass. The
  default Vitest glob also found the unrelated `.claude` worktree; the clean rerun explicitly excluded `.claude/**`.
- Cargo formatting and locked checking pass. The locked Rust suite passes outside the sandbox after expected
  loopback-bind denials: 649 library tests (37 ignored), 1 updater-evidence test, 6 local-image adapter tests, 10
  worker-transport tests, and doc tests.
- Both final DGX collections agree and reproduce the checked-in closure record SHA-256. JSON parsing and
  `git diff --check` pass.

## Recent maintenance

MOSS-Image integration checks on 2026-10-18 pass: 433 frontend/script tests (three existing skips), 12 Python setup/
adapter tests, Svelte diagnostics, Prettier, Black, byte compilation, frontend build, Cargo formatting/locked checking,
and the serial host native suite: 712 library tests (41 ignored), one updater fixture, six adapter fixtures, ten
transport fixtures, and doc tests. The Swift/MLX bridge builds with its pinned dependency graph, and a real Metal
array probe passes from the copied app-data runtime. Full-model acquisition and live layout/storage/cancellation/
recovery qualification are still in progress at this checkpoint. CUDA hardware and native Tauri clicks are not
claimed. MOSS durable model IDs include the immutable weight revision so old retries cannot silently use new weights.

MOSS-TTS-Nano native integration completed on 2026-10-18. An explicitly provisioned app-data runtime adds MOSS Nano · Ava
and MOSS Nano · Adam to the existing opaque voice/preference contract. A dedicated Rust actor owns a reusable offline
CPU worker, strict bounded PCM validation, a two-second native queue, CPAL device-rate conversion, device-predicted
drain, and independent cancellation. Stop discards queued audio and kills/reaps the worker before capture admission;
speech starts now share the capture-registration mutex. OS speech remains independent, including when its engine is
unavailable. Explicit Python 3.12 setup reuses compatible environments and verified model files; paths and PCM remain
native. The contained `run_python` executor is unchanged. The host proof passed real native English/German playback,
warm reuse, stable preference restoration, interruption, and fresh-worker recovery. The initial sandboxed playback
check failed and its identical host retry passed. Browser Settings inspection is fixture evidence, not native Tauri
click automation. Exact setup, bounds, and remaining platform/distribution qualification are in
`docs/moss-nano-integration.md`. No generated speech recording is retained by the app.
Validation: 428 frontend/script tests (three skipped), 18 Python setup/protocol tests, Svelte diagnostics, formatting,
the production build, Cargo formatting/locked checking, and the serial host Rust suite pass. Rust results: 700 library
tests (40 ignored), one updater fixture, six adapter fixtures, ten transport fixtures, and doc tests. The isolated
sandbox keychain failure passed in the identical host retry. The saved-selection startup race has regression coverage.
The development-signed contained-Python DMG is `bottie_0.9.0_nano_b3fa63e_development_aarch64.dmg`, from source commit
`b3fa63e1cb5c3f64b98daa314be7d71650549b7e`; SHA-256
`8e150481670952ddfc9ec5ac4c6b670f9ef3388a5521a5e68e2985d65f758025`. Final DMG verification, mounted/staged app-byte
agreement, strict main/nested signature verification, and final mounted-app contained-Python smoke checks pass. It is
an unnotarized development testing package. Nano's external model/runtime are configured on this Mac, not bundled.
The sandbox DMG bundler failed; the identical host build passed. Artifact/inspection/proof/checksum/tester instructions
are under `src-tauri/target/release/bundle/dmg/`.

MOSS-TTS-Nano feasibility work completed on 2026-10-17 in `scripts/moss-nano-spike/`. Twenty immutable code/model/codec
assets (763,254,539 bytes) are explicitly acquired and SHA-256 verified before offline CPU inference. The low-level
ONNX core avoids the convenience wrapper's Torch/TorchAudio imports. The trial uses bounded private-stdio PCM,
finite/aligned audio validation, token-limited text chunks, hard generation ceilings, and process termination/reaping.
All six synthetic English/German short/numeric/long cases completed on macOS arm64 with four CPU threads. First PCM
was 116–159 ms, RTF 0.249–0.372, fresh-process readiness 3.61 seconds, and peak process RSS 1.67 GiB. Active cancellation
reaped the worker in 42.9 ms; a fresh worker reproduced the initial English WAV hash. All 14 boundary tests,
byte-compilation, Black formatting, asset verification, and diff checks pass. Exact evidence and limitations are in
`docs/moss-nano-evaluation.md` and `docs/moss-nano-measurements.json`. At that checkpoint human audition and application
integration were still pending; the 2026-10-18 record above supersedes those next steps. The original trial does not
claim packaged-runtime distribution or acoustic qualification.

Composer maintenance completed on 2026-10-13: accessible Chat/Image tabs replace the Image toggle. Chat tools and
collapsible voice input appear only in Chat; Image has independent aspect/size controls and collapsed local model
details. Prompt drafts survive tab switches and both clear on New conversation. Arrow keys/Home/End move tab focus
and selection; mode switching is disabled during submission, generation, or active capture.
Cloud and exact Local 2.1 requests use three size tiers and five exact aspect ratios. Local defaults to 512×512 and
always returns one image. The older 2512 worker keeps its fixed 512×512 limit. Dimensions and model identity are
snapshotted before asynchronous prompt persistence and retained in retry metadata. The request validator and server
adapter share a closed native dimension catalog; frontend tests compare its complete set to the offered controls.
The live 768×512 native fixture passed generation, normalization, durable dimensions/model provenance, and cancellation
in 138.53 seconds. Browser interaction checks covered desktop/narrow layout, controls, draft retention, and keyboard
tabs; no full native Tauri UI click automation is claimed. See `docs/qwen-image-2.1-local.md` and the user manual.
Validation: formatting, Svelte checks (zero errors/warnings), 425 frontend tests (3 skipped), and the production build
pass. The locked serial host suite passes 690 library tests (39 opt-in fixtures ignored), the updater fixture,
6 worker-adapter fixtures, 10 worker-transport fixtures, and doc tests. The separate live size/aspect proof passed.
The refreshed contained-Python testing DMG is `bottie_0.9.0_image-tabs_71721b3_aarch64.dmg`, built from source commit
`71721b339e929d07083d772c3ededb1673d25ff2`; SHA-256
`a924fd02cb68574e428d428be4800e0707c9ab502e1f34c868109b378e62bc0e`. Disk-image verification, strict main/nested
signatures, and the final mounted-app Python smoke proof pass. It remains an unnotarized development testing package.

Testing package and optional-installer foundation completed on 2026-10-13. The contained-Python arm64 0.9.0 DMG was
built from pre-installer commit `57d82da96ff7f9652570f2fef1ed68726a800182`, retained with that revision in its filename,
and verified through the mounted app. Disk-image verification and Python private-pipe/sandbox/cancellation/cleanup
checks passed. The main app was subsequently development-signed and repacked; final mounted-byte main/nested signature
verification and Python smoke checks passed. It is unnotarized, without production Developer ID signing. Final file:
`bottie_0.9.0_qwen21_57d82da_development_aarch64.dmg`; SHA-256
`8bdbbd6f461d4932471666d4a9ccbd926f004bbb2a6c84d0c8958fcd86849cfe`. Artifact, checksum, exact-byte inspection, and
tester instructions are under `src-tauri/target/release/bundle/dmg/`.
The native optional 2.1 model-file installer now pins the existing exercised pack at
`fbda4caa0b4b1e17b5a29633e8deb600386f1eaf`: 18 files, 17,647,912,015 bytes. All installed files match the pinned
size/hash inventory. Consent binds the exact terms digest/revision/identity/bytes before I/O; downloads resume,
cancel, verify/promote atomically, and reverify/reuse completed cache files. No duplicate model download occurred.
This is a native model-acquisition layer, not a new app action or managed server installation. Runtime qualification,
installation/lifecycle, mutation serialization, and UI integration remain the next slice. Newer upstream `main`
changes a weight shard and is not silently adopted. Details and runtime candidate pins are in
`docs/qwen-image-2.1-local.md`; Roadmap 8.6 tracks the remaining work.
Validation: six focused acquisition tests pass; the locked serial host suite passes 688 library tests (39 opt-in
tests ignored), the updater fixture, 6 worker-adapter fixtures, 10 worker-transport fixtures, and doc tests. Cargo
formatting/checking, 418 frontend tests (3 skipped), Svelte diagnostics, formatting, and the production build pass.

Local image maintenance completed on 2026-10-13: Bottie now prefers the installed
`ddalcu/Qwen-Image-2.1-MLX-Serve-8bit` pack through MLX Serve's loopback image API at port 11234. It reuses the existing
oMLX model-directory files in place; oMLX 0.7.1.dev1 itself only lists the pack as an unloaded text model and exposes no
image output endpoint. No oMLX configuration was changed. Starts bind the displayed package before asynchronous
persistence, retries retain the original package/seed, and the managed 2512 worker stays separate. Inline response
bytes remain native, bounded, redirect/proxy-free, and normalized into the shared private PNG lifecycle.
The live native fixture passed 512×512 generation, durable completion with exact Local provenance, and cancellation;
the MLX Serve log confirmed disconnected-client generation stopped. Research-licensed managed downloads require
explicit revision-bound consent. This 2.1 route reuses an externally installed server; a Bottie-managed optional 2.1
installer's runtime/app integration remains future work. See `docs/qwen-image-2.1-local.md` and Roadmap 8.6.
Validation: 418 frontend tests pass (3 skipped), Svelte diagnostics and the production build pass, and the final host
suite passes 682 library tests (39 opt-in tests ignored), the updater fixture, 6 worker-adapter fixtures, 10 worker
transport fixtures, and doc tests. Cargo formatting/checking pass. The current composer was visually inspected at
1280×720 and 620×800. The separate opt-in local 2.1 native proof also passed. No signed installer was built.

Branding maintenance completed on 2026-09-30: `assets/logo_v2/bottie_icon_512.png` is the approved app/general logo
master; the locked icon pipeline now derives both product favicons and the website header/footer logo from it.
`npm run package:macos:dmg` builds the
local unsigned macOS installer with contained Python by default; the explicit `:no-python` variant opts out. The
inspected arm64 DMG is neither distribution-signed nor notarized.

Credential and Localmail maintenance on 2026-10-01: macOS now stores credentials together in one keychain item.
Status queries request public attributes only, session authentication accepts Touch ID or a login password, and
accessible legacy entries migrate with keychain dialogs disabled. Older entries requiring separate approval remain
intact; re-save their API keys in Settings. Public retirement metadata prevents replaced/deleted legacy entries from
returning. Localmail setup now accepts administrator-issued `lmk_` API keys, still sent with the server's Bearer
scheme, and rejects expiring login-token drafts. No live prompt-count or Localmail-server acceptance is claimed.
The Settings follow-up includes Localmail key changes in Save and reconnect, keeps drafts on failed writes, and
uses draft credentials without eagerly reading locked legacy keys for search/cloud/image tests. Removal commits
legacy retirement before best-effort cleanup, so a legacy ACL cannot stop removal or resurrect its key. Regression
checks cover replacement/removal across restart and actual Settings clicks with fake vault commands.

Generation maintenance on 2026-10-02: recent oMLX failures hit the old four-round/eight-call tool budgets while the
server returned valid tool requests with roughly 7k context tokens. Generation limits are now saved in Settings,
defaulting to 12 rounds, 24 calls, and 8,192 output tokens per model request. Rust validates the saved values and
snapshots them before run provenance; all mapped provider loops use those snapshots. Older settings gain defaults.
The independent five-minute tool-work deadline, 120-second stream-idle timeout, and output-byte limits remain.
Validation: 413 frontend tests and 671 active native library tests pass; the explicit loopback fixture also completes
12 and 13 configured tool rounds. Settings clicks verify save/reopen, cancellation, and invalid-value rejection;
native persistence checks verify restart and legacy defaults. No live private conversation was replayed.

Localmail PDF maintenance on 2026-10-02: the attachment reader rejected the server's valid `offset`, `limit`, `total`,
and `next_offset` fields because it expected a closed text-only response. It now accepts paged responses and older
text-only responses, requests the first 12,288 characters, validates page metadata, and marks unread text truncated.
Malformed server responses now have a specific fixed decode-failure message without exposing native details.
Synthetic regression tests reproduce the rejection using the actual Localmail response schema. No private PDF was
retrieved; server-side text extraction must already have completed.
All 38 focused Localmail tests pass. The full host suite passes 673 tests but the existing shell-based Python
cancellation fixture times out waiting for its start marker, also when run alone. The actual development-signed DMG
passes packaged Python execution, App Sandbox containment, cancellation, and client-exit termination checks.

## Next product slice

Complete independent expression review for the exact clean closure without assembling a bundle:

1. obtain a complete independent review record for all 62 sorted component identities, including reviewed non-placeholder
   expressions for Jinja2, safetensors, tokenizers, and Triton; do not infer expressions from package metadata;
2. TDD a clean-profile-only two-trace review intake bound to the exact image, lock, trace contexts, source archives,
   licence/notice bytes, and per-component review evidence without weakening the retained NGC review route;
3. rerun both closures and retain output only if every expression and review byte agrees, no licence blocker remains,
   `licenseReviewed` and `assemblyEligible` are true, and `distributionReviewed` remains false.

Stop if a complete independent review record is unavailable. Do not author a review conclusion, infer an expression,
assemble or publish bytes, accept a Linux profile, add hardware probing/app execution/UI, or perform signing, release,
Store, updater, provider, or legal-terms activity.
