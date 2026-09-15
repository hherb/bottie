# Bottie handover

Last verified: 2026-09-16

## Start here

`main` includes merged PR #186 at `fb31182`. Branch `codex/qwen-image-live-fixture-support-matrix` completes the two
remaining Milestone 8 slices that did not require new hardware or a billed provider request. Read `ROADMAP.md`
Milestone 8, `src-tauri/src/image_generation/live_tests.rs`, and the Qwen Image section in `README.md`.

## Current state

- The exact hosted `qwen-image-2.0-2026-03-03` adapter has an ignored, opt-in live fixture restricted to one 512×512
  low-risk output, a throwaway Singapore Model Studio workspace, and a separately supplied throwaway key. Ordinary
  tests cannot run it or spend provider credit.
- `README.md` and the user manual publish the evidence-backed local support matrix. Only Apple M3 Max with 128 GiB
  unified memory is accepted for the exact Qwen-Image-2512 MLX-Gen route. Every other Apple, NVIDIA, AMD, Intel, and
  lower-memory profile remains unavailable without fallback.
- Qwen's official GitHub repository, Hugging Face inventory, and ModelScope registry were checked on 2026-09-16. They
  still expose the hosted 2.0 launch and older open models, but no exact Qwen-Image-2.0 weights. No local-2.0 product
  change was made.

## Validation and limits

Prettier, Svelte diagnostics (0 errors and 0 warnings), all 407 active frontend/script tests (3 skipped), the production
build, `cargo fmt --check`, and `cargo check` pass. The host-local Rust run passes all 646 active library tests (37
ignored), updater evidence, all 16 private-worker integration tests, and doc tests. The new live fixture compiled and
was confirmed ignored; it was not executed because no throwaway key/workspace or billed provider call was authorized.
The user manual's Images section and support table were reviewed at desktop width and 540×800. No credentials, provider
request, model bytes, source assets, or private paths left the device. Unrelated untracked logo-kit, screenshot, and
Linux public-key files remain untouched.

## Next slice

On an explicitly available, named Linux NVIDIA host, prove one pinned PyTorch + Diffusers `QwenImagePipeline` worker
through Bottie's existing private protocol before adding any Linux availability profile. Freeze the GPU, driver, CUDA,
Python/package, model revision, complete files/hashes, and licenses; measure VRAM and host memory, cold and warm runs,
deterministic seed/output, decoded PNG correctness, network denial, and cancellation latency. Keep Linux NVIDIA
unavailable if any evidence is absent or outside existing bounds.

Do not dispatch a runner, download model/runtime bytes, accept terms, use credentials, or publish a Linux support claim
without separate authorization. Do not add WSL-as-Windows evidence, cloud fallback, local editing, guessed 2.0 weights,
source-byte/path IPC, signing, release, Store, or distribution work.
