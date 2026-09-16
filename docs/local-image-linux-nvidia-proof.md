# Linux NVIDIA Qwen Image proof

Last reviewed: 2026-09-16

This document freezes the first native Linux NVIDIA execution evidence for Bottie's private image-worker protocol. It
is a feasibility proof, not a Linux product-availability claim. Bottie still has no accepted redistributable Linux
worker bundle, Linux hardware probe, app-owned runtime import, or Linux availability profile. The separate
[`local-image-linux-worker-bundle.md`](local-image-linux-worker-bundle.md) contract can prepare deterministic review
material for exact produced bytes, but no such candidate has been accepted.

## Exact target

The named target was an NVIDIA DGX Spark with one GB10 GPU, compute capability 12.1, 128 GB coherent unified memory,
and an ARM64 host. The proof recorded Ubuntu 24.04.4, Linux `6.17.0-1029-nvidia`, NVIDIA driver `580.173.02`, and CUDA
13.0. DGX Spark does not expose a separate aggregate VRAM total through `nvidia-smi`; NVIDIA documents this as expected
for its unified-memory architecture. The proof therefore sampled Linux `MemAvailable`, the worker process high-water
RSS, and NVIDIA's per-process counter without treating an unavailable counter as zero use.

CPU offload was disabled. The exact pipeline used `device_map="cuda"` and `torch.bfloat16`; it did not silently select
CPU offload or another device. An unrelated resident `llama-server` process was left running, so the host-availability
measurements are deliberately conservative rather than a clean-machine capacity claim.

## Runtime and model pins

- Base image: `nvcr.io/nvidia/pytorch:25.11-py3` at
  `sha256:417cbf33f87b5378849df37983552cd1f8bc8b62fe1ceabe004de816a55dff21`.
- Derived proof image: `sha256:cded2049f9dbff513406052a191af2588476056d795468c4aacb7814aca5666c`,
  19,693,091,337 bytes in the local Docker store.
- Base runtime: Python 3.12.3 and PyTorch `2.10.0a0+b558c986e8.nv25.11`.
- Added exact packages: Accelerate 1.15.0, Diffusers 0.40.0, Hugging Face Hub 1.31.0, safetensors 0.8.0,
  SentencePiece 0.2.2, tokenizers 0.23.2, Transformers 5.17.0, and Typer 0.27.2. The complete direct pin set is in
  `local-image-worker/requirements-diffusers-proof.txt`.
- Worker identity: `diffusers-0.40.0-ngc-25.11-proof-1`; runtime identity:
  `diffusers@0.40.0+ngc-25.11-arm64`.
- Model: `Qwen/Qwen-Image-2512`, immutable revision
  `25468b98e3276ca6700de15c6628e51b7de54a26`, 30 files, 57,704,595,735 bytes.
- Complete sorted per-file hashes are checked in as `docs/qwen-image-2512-diffusers-model.sha256`; that 30-line
  manifest's own SHA-256 is `db98eb84cca7a34853526a3d2cf7ef2ec63cf5ffc2adcdf82f3ea92b3c93c656`. Each model file was
  hashed locally before the proof; the count and total bytes matched the exact public revision.

The model declares Apache-2.0. The added Python distributions declare Apache/Apache-2.0, BSD-3-Clause, or MIT in their
installed metadata, except safetensors and tokenizers, whose installed metadata did not expose a license field. The
NVIDIA base image already existed on the target and remains governed by NVIDIA's container/product terms. This evidence
does not approve redistribution of the base image, Python environment, or a derived worker bundle; a separate complete
license and distribution review is still required before product packaging.

| Added distribution | Installed license metadata |
| --- | --- |
| Accelerate 1.15.0 | Apache |
| annotated-doc 0.0.5 | MIT |
| Click 8.5.0 | BSD-3-Clause |
| Diffusers 0.40.0 | Apache 2.0 License |
| hf-xet 1.6.0 | Apache-2.0 |
| Hugging Face Hub 1.31.0 | Apache-2.0 |
| safetensors 0.8.0 | Undeclared |
| SentencePiece 0.2.2 | Apache-2.0 |
| tokenizers 0.23.2 | Undeclared |
| Transformers 5.17.0 | Apache 2.0 License |
| Typer 0.27.2 | MIT |

## Measured protocol run

The harness used one 512×512 output, 15 denoising steps, seed 42, and the prompt “A violet glass robot tending a tiny
greenhouse, detailed botanical illustration.” It loaded only the verified local model tree, ran inside a read-only
container with all capabilities dropped, no-new-privileges, a non-root UID, and Docker `--network none`, then exercised
the same length-prefixed private protocol used by Bottie's MLX worker.

| Evidence | Result |
| --- | ---: |
| Model load | 368.430 s |
| Cold generation | 13.663 s |
| Warm generation | 14.397 s |
| Cancellation after denoising step 2 | 100 ms |
| Worker process RSS high-water | 17,583,603,712 bytes |
| Host available memory at start | 104,280,002,560 bytes |
| Minimum host available memory | 40,983,056,384 bytes |
| Conservative availability drop | 63,296,946,176 bytes |
| NVIDIA per-process unified-memory counter | Unavailable during sampling |
| Cold and warm encoded PNG | Identical; 474,620 bytes |
| Cold and warm decoded RGB | Identical |
| External TCP probe | Denied by the network namespace |
| Clean shutdown | Passed within the 30 s harness deadline |

The encoded PNG SHA-256 is `52aaa947ee40493f4f85405b383c499f11cf9ad88886ba919abc4a0e16b380b1`.
The decoded RGB SHA-256 is `fe4afa9c3ba700c15f57650e4090d885075809c17f69bf2559226f8f7224947d`.

The final PNG decoded as RGB at exactly 512×512. Manual review found a coherent violet glass robot tending plants in a
greenhouse, matching the prompt without visible corruption. The cancelled run retained no output. The worker's Python
audit hook also fails network attempts with the fixed path-free diagnostic `network-attempt-denied`. The unmodified
machine-readable harness result is checked in as `docs/local-image-linux-nvidia-measurements.json`; its
`visualReviewed` field remains false because the manual review happened after the harness wrote the record.

## Reproduction boundary

`Dockerfile.diffusers-proof` is intentionally a proof recipe. Building it needs network access to obtain the exact
pinned Python distributions; runtime proof execution does not. `prove_diffusers_worker.py` requires an already verified
absolute model directory, an empty writable output directory, and an absolute measurement destination. It applies the
network, filesystem, capability, user, deadline, deterministic-output, decoded-PNG, memory-sampling, cancellation, and
shutdown checks itself.

Do not convert this evidence directly into a support claim. The native package catalog now represents this exact
Diffusers worker, runtime, model revision, Linux ARM64 target, DGX Spark evidence profile, and this evidence document as
a closed candidate. It deliberately has no accepted product profile or importable executable, so selection fails
closed before worker import, readiness, or execution. The catalog also marks its deterministic bundle inspector as
candidate preparation only. A later slice must produce and independently review the exact executable, complete bundle
inventory, third-party licence evidence, and distribution boundary before considering Linux hardware probing or
product availability; it must not infer either from this Docker proof or from a structurally valid candidate record.
