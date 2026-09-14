# Bottie handover

Last verified: 2026-09-14

## Start here

`main` includes PR #171 at `fbb13f5`. Branch `codex/local-image-model-downloader` completes the next Milestone 8.3
foundations: exact model source planning, resumable native download into the transactional cache, and source-bound
restart safety. Read `ROADMAP.md` Milestone 8.3 and `src-tauri/src/local_image_worker/` before continuing.

## Completed slice

- `ModelSourcePlan` accepts one native-reviewed repository root only as canonical HTTPS, then binds every manifest file
  in exact order to the same immutable source revision, portable path, and bounded strong ETag. Test-only HTTP accepts
  literal loopback hosts; credentials, queries, fragments, percent-encoded roots, weak validators, and revision drift
  fail closed.
- `ModelDownloader` cannot open cache or network work until the matching `ModelAcquisition` has entered its explicit
  downloading phase. It keeps redirects disabled and enforces fixed connection, per-file, package-time, per-file-byte,
  and package-byte ceilings.
- Full responses require exact `200`, ETag, and `Content-Length` metadata. Resumes additionally require exact `206`,
  `Content-Range`, `If-Range`, offset, total size, and remaining length; ignored/wrong ranges and duplicate or changed
  validators are rejected before response bytes are retained.
- Partial files now use a one-pass cache writer: the retained prefix is re-hashed once, chunks cannot exceed the exact
  file size, and progress becomes observable only after file and directory sync. Transport interruption, timeout, and
  cancellation retain a synced resumable prefix; unsafe response or integrity failure discards only the exact staging
  transaction.
- Staging persists only an opaque SHA-256 binding over repository root, revision, paths, and validators. Source-plan
  drift discards an old partial before issuing a new full request, while final activation still requires every exact
  manifest size and SHA-256. Successful promotion advances the approved acquisition through verification to `Ready`.
- Exact hosted `qwen-image-2.0-2026-03-03` remains distinct from local open-weight `Qwen/Qwen-Image-2512`, and this
  subsystem remains separate from Bottie's user-approved Python tool runtime.

## Validation

- Focused acquisition/cache/downloader suites: 26 passed, including full and ranged transfer, restart resume,
  ignored/wrong/duplicate range metadata, validator and source-plan drift, redirects, truncation/overflow, byte limits,
  explicit approval, sync-before-progress, cancellation, timeout, cache cleanup, promotion, and activation.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and isolated-target
  `cargo check --manifest-path src-tauri/Cargo.toml` pass. The ordinary-target Cargo check also passes.
- The host-local full Rust suite passes 583 library tests with 36 intentionally ignored, one updater-evidence test, and
  all 10 private-process integration tests. `npm run format:check`, `npm run check`, `npm test` (360 passed, 3 skipped),
  and `npm run build` pass.
- All-target Clippy reaches only existing unrelated warnings under `-D warnings`; it reports no changed-file finding.
- No real model/runtime bytes, selected MLX-Gen package, external network download, provider request, billable action,
  runtime execution, hardware probe, local generation, Svelte IPC/UI, or user-approved Python execution was exercised.

## Next slice

Freeze one actually supported Apple-silicon MLX-Gen package only after reviewing official immutable revisions and
collecting exact model/runtime paths, sizes, SHA-256 digests, strong validators, license material, and measured disk,
peak-memory, output, and cancellation evidence on the target hardware. Encode that evidence as Bottie's first selected
`ModelPackageManifest` plus `ModelSourcePlan`, with regression fixtures that require every reviewed value before the
existing approval/downloader/cache/worker-load path can become available.

Do not infer a package tier from model names, use a moving branch/tag, hard-code unmeasured weights, download
multi-gigabyte artifacts without explicit approval, execute a runtime, add UI availability, or claim hardware support
before that evidence exists. Presentation should follow the selected package contract and expose only path-free status;
there is still no silent cloud fallback and no worker network-isolation claim without a real runtime-specific process
proof.

Do not merge, dispatch workflows, sign, release, publish, distribute, or perform Store work without separate
authorization. Preserve unrelated untracked logo-kit, screenshot, and Linux public-key files.
