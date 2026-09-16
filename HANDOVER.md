# Bottie handover

Last verified: 2026-09-16

## Start here

`main` includes merged PR #187 at `1d79d6b`. Branch `codex/qwen-image-linux-nvidia-proof` completes the next Milestone
8.4 slice: a pinned Diffusers `QwenImagePipeline` feasibility proof through Bottie's private worker protocol on a named
NVIDIA DGX Spark. Read `ROADMAP.md` Milestone 8.4, `docs/local-image-linux-nvidia-proof.md`, and
`local-image-worker/diffusers_worker.py`.

## Current state

- The exact `Qwen/Qwen-Image-2512` revision `25468b98e3276ca6700de15c6628e51b7de54a26` was anonymously downloaded and
  locally verified as 30 files and 57,704,595,735 bytes. The complete per-file SHA-256 manifest is checked in; its own
  digest is `db98eb84cca7a34853526a3d2cf7ef2ec63cf5ffc2adcdf82f3ea92b3c93c656`.
- A proof-only ARM64 container pins NGC PyTorch 25.11 by digest, Diffusers 0.40.0, and every added Python distribution.
  Its exact derived image ID is `sha256:cded2049f9dbff513406052a191af2588476056d795468c4aacb7814aca5666c`.
- The shared Python worker now accepts an explicit backend identity without weakening MLX validation. The Diffusers
  adapter loads only local bfloat16 weights onto CUDA and uses the same closed
  hello/load/generate/progress/cancel/result protocol. Network attempts have a fixed path-free diagnostic.
- The final read-only, non-root, capability-free, network-denied DGX run passed exact model loading, two same-seed
  512×512 generations, decoded-PNG validation, deterministic byte and pixel hashes, denoising-boundary cancellation in
  100 ms, and bounded clean shutdown. Manual review found a coherent, prompt-matching PNG without visible corruption.
- Linux NVIDIA remains unavailable in the product. The existing Apple M3 Max profile is still the only accepted local
  availability route. The proof container is neither an application payload nor approved redistribution evidence.

## Validation and limits

The focused Python suite passes all 14 tests. Prettier, Svelte diagnostics, all 407 active frontend/script tests (3
skipped), the production build, `cargo fmt --check`, and `cargo check` pass. The host-local Rust run passes all 646
active library tests (37 ignored), updater evidence, all 16 private-worker integration tests, and doc tests.

The proof measured a 368.430 s model load, 13.663 s cold generation, 14.397 s warm generation, 17,583,603,712-byte
worker RSS high-water, and a conservative 63,296,946,176-byte host-availability drop. DGX Spark uses coherent unified
memory and exposes no separate aggregate VRAM total; its per-process counter was unavailable during sampling. CPU
offload was disabled. An unrelated resident inference process was not stopped. Exact model/runtime files and generated
proof outputs remain only in the dedicated DGX proof workspace; no credential or provider request was used.

Unrelated untracked logo-kit, screenshot, and Linux public-key files remain untouched.

## Next slice

Refactor the native selected local-image package contract so backend/runtime selection is explicit and can represent
both the existing accepted Apple MLX package and this exact unavailable-by-default Linux Diffusers candidate. Start with
pure Rust catalog/selection types and tests: preserve every Apple manifest, acknowledgement, acquisition, availability,
and execution behavior byte-for-byte; bind the Linux candidate to the exact runtime/model identities and evidence
document; and fail closed when no accepted platform profile selects it.

Do not add the Linux candidate to native availability yet. Do not add a hardware probe, Docker dependency, worker
import/download, automatic model download, runtime execution, UI/IPC field, or support claim in that slice. Do not add
WSL-as-Windows evidence, cloud fallback, local editing, guessed 2.0 weights, source-byte/path IPC, signing, release,
Store, or distribution work.
