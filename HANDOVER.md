# Bottie handover

Last verified: 2026-09-14

## Start here

`main` includes merged PR #172 at `e6c28c1`. Branch `codex/local-image-package-evidence` continues Milestone 8.3 with
the first immutable MLX-Gen model candidate and the Hugging Face delivery contract needed to acquire it. Read
`docs/local-image-model-package.md`, `ROADMAP.md` Milestones 8.3-8.4, and `src-tauri/src/local_image_worker/`.

## Completed slice

- The reviewed candidate is `AbstractFramework/qwen-image-2512-4bit` revision
  `423f1f5bf708c6e11eb78881ef9738422cea0814`: 18 exact files totaling 17,442,350,812 bytes, Apache-2.0, derived from
  `Qwen/Qwen-Image-2512`, and bound to MLX-Gen 0.18.2 commit `fca64a283737c68b67a7bfd88d93f7aa9101a95c`.
- Base-model and package identities are distinct in the manifest and path-free acquisition status. The candidate cannot
  produce an active manifest/source plan until exact worker bytes, target hardware/profile, output digest and visual
  review, measured peak memory, and sub-three-second active cancellation evidence all agree.
- Hugging Face sources permit one manually validated 302/307 envelope while automatic redirects remain disabled. The
  resolver requires exact commit, linked ETag, optional linked size, and a tightly allowlisted HTTPS destination; a
  second redirect, metadata drift, unsafe host, malformed range, or final-length mismatch fails before bytes are kept.
- Resume staging now binds the delivery policy in addition to root, revision, paths, and validators. Full and ranged
  two-hop fixtures cover exact success, drift, unsafe hosts, second redirects, and direct-source separation.

## Validation

- Focused package-evidence tests: 3 passed.
- Focused direct/Hugging Face downloader tests: 16 passed using host-local loopback.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and
  `cargo check --manifest-path src-tauri/Cargo.toml` pass.
- The full Rust library suite passes 592 tests with 36 intentionally ignored; the updater evidence test and all 10
  private-process integration tests pass. One combined-suite transport teardown attempt failed transiently, then passed
  both alone and with all 10 transport tests on exact rerun.
- `npm run format:check`, `npm run check`, `npm test` (360 passed, 3 skipped), and `npm run build` pass.
- No model weights, MLX-Gen environment, worker runtime, provider request, or generated output was downloaded or run.

## Next slice

After explicit approval for a 17,442,350,812-byte model download and runtime execution, build an isolated worker from
the pinned MLX-Gen commit, hash its exact executable/runtime bundle, acquire the frozen q4 package through the existing
approval/downloader/cache path, and run the fixed Apple M3 Max 128 GB, 512x512, 15-step proof. Record whole-process peak
memory, decoded PNG hash plus visual review, and active-step cancellation latency. Only accepted evidence may turn the
candidate into the selected manifest/source plan.

Do not add UI availability, infer support from macOS/model names, weaken the evidence gate, reuse Bottie's approved
Python-tool runtime, download the model or execute MLX-Gen without explicit approval, claim worker network isolation
without a runtime-specific proof, or silently fall back to cloud.

Do not merge, dispatch workflows, sign, release, publish, distribute, or perform Store work without separate
authorization. Preserve unrelated untracked logo-kit, screenshot, and Linux public-key files.
