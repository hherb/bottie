# Linux NVIDIA Qwen Image proof

Last reviewed: 2026-09-17

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

## Provisional UCC-removal experiment

The retained NGC PyTorch build links `libucc.so.1` directly: masking `/opt/hpcx/ucc` makes a bare `import torch` fail.
Masking all of `/opt/hpcx` additionally removes `libmpi.so.40`. Removing UCC from this exact runtime is therefore not a
viable packaging shortcut.

A bounded alternative replaced only NGC's PyTorch build with the conventional ARM64 CUDA 13.0 PyTorch 2.10.0 wheel
from the [official wheel index](https://download.pytorch.org/whl/cu130/torch/). The 529,404,606-byte wheel has SHA-256
`4fc8f67637f4c92b989a07d80ffe755e79a3510ca02ebf23ce66396fb277c88d`. The derived experimental image was
`sha256:2f559d21bb696d6d56ddc0943569571d1905f4a7287d9d3e6c9905a07b0e676b`, 21,250,037,999 bytes in the local
Docker store.

With `/opt/hpcx/ucc` replaced by an empty read-only mount, that image completed model load, byte-deterministic cold and
warm generation, active-step cancellation, network denial, and clean shutdown. A live process-map snapshot contained
no UCC path or `libucc` library. Cold and warm decoded RGB matched at
`a3ec2972f88027b969e087a39d4d1b738438b61150ccd4d9dec2842ca1a9ad70`; manual review found the same coherent scene
as the retained baseline. Load took 354.353 seconds, cold generation 14.661 seconds, warm generation 12.323 seconds,
and cancellation 100 ms.

This result is deliberately provisional. The experimental image still contains the NGC UCC installation behind the
mask, and its run predates the separate `diffusers-0.40.0-pytorch-2.10.0-cu130-proof-1` worker identity. No measurement
record from it is checked in. The wheel also reports CUDA architectures through 12.0 while the named GB10 is compute
capability 12.1, despite the exercised BF16 path completing successfully. A fresh run must use the distinct profile,
then a clean image built without UCC must produce a complete runtime closure before this route can affect packaging.

### Clean runtime result

A subsequent isolated build started from Ubuntu 24.04 ARM64 at
`sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90`, installed Python 3.12 in a
virtual environment, and resolved the verified PyTorch wheel plus the pinned direct Diffusers requirements. The exact
image is `sha256:5039ad07130ce8d29f12325a54e02bf95e90112e114d745e5883434180e3bdad`, 5,780,237,265 bytes,
ARM64/Linux. It contains no HPC-X installation or `libucc` runtime library. PyTorch includes UCC-related C++ headers,
so this is a UCC-runtime exclusion claim, not an assertion that no filename or source text mentions UCC.

The clean image passed the complete offline proof with the corrected runtime identity. Load took 350.350 seconds; cold
and warm generation took 13.855 and 12.292 seconds; cancellation completed in 111 ms. Cold and warm encoded PNGs were
identical at SHA-256 `cb3e252f7df6748daf191bb25cb6fc41168c8c5a1f541c7b559ffc2df720eb3d`, and their decoded RGB
matched the replacement control. The already reviewed byte-identical image is coherent and prompt-matching. The live
process maps contained no `libucc`, network denial passed, and the worker shut down cleanly.

The resolved environment contains 63 Python distributions and 112 Debian packages. Their versions were captured from
the exact image, but their source-wheel/deb byte hashes and licences are not yet frozen or reviewed. The build used
mutable Ubuntu and Python indexes after the immutable base, so the recipe is not checked in as reproducible evidence.
[`local-image-linux-ucc-free-measurements.json`](local-image-linux-ucc-free-measurements.json) preserves the unmodified
path-free harness result. Linux remains unavailable until exact inputs, repeated traces, closure, and review exist.

The repository now has a proof-only offline input-lock gate for the next clean build. It resolves the source image
reference through the Docker daemon, requires the exact clean image ID and Linux/ARM64 target, and collects its complete
installed Python and Debian identities by that immutable ID with networking disabled. Lock generation then requires
exactly 63 wheels and 112 Debian archives in separate symlink-free directories. It rejects Python source archives,
non-ARM platform wheels, foreign Debian architectures, missing or extra files, embedded wheel-identity drift, Debian
control-metadata drift, and byte/hash drift. Every wheel must have one bounded `WHEEL` metadata member beside
`METADATA`, and its expanded compatibility tags must exactly equal the filename tags, so a renamed foreign-platform
archive cannot satisfy the ARM64 gate. The generated manifest retains only portable filenames, identities, exact
sizes, SHA-256 hashes, and a canonical lock digest.

This gate does not download packages, prove index immutability, rebuild an image, infer licence expressions, or make the
clean image distributable. No complete lock has been produced yet because the exact wheel and Debian archives have not
been assembled and compared with the retained clean image.

## Reproduction boundary

`Dockerfile.diffusers-proof` is intentionally a proof recipe. Building it needs network access to obtain the exact
pinned Python distributions; runtime proof execution does not. `prove_diffusers_worker.py` requires an already verified
absolute model directory, an empty writable output directory, and an absolute measurement destination. It applies the
network, filesystem, capability, user, deadline, deterministic-output, decoded-PNG, memory-sampling, cancellation, and
shutdown checks itself.

`Dockerfile.diffusers-pytorch-proof` pins the same NGC base and checksum-pins the conventional wheel solely to
reproduce the removal experiment. Its separate entrypoint reports the alternative runtime identity. The proof harness
accepts that identity only with `--runtime-profile pytorch-2.10-cu130 --ablate-ucc`; ablation mounts an empty directory
over the complete UCC installation and rejects any live mapping containing the original path or `libucc` elsewhere.
The recipe is not a clean or redistributable worker image because the masked UCC bytes remain in an underlying layer.

Do not convert this evidence directly into a support claim. The native package catalog now represents this exact
Diffusers worker, runtime, model revision, Linux ARM64 target, DGX Spark evidence profile, and this evidence document as
a closed candidate. It deliberately has no accepted product profile or importable executable, so selection fails
closed before worker import, readiness, or execution. The catalog also marks its deterministic bundle inspector as
candidate preparation only. A later slice must produce and independently review the exact executable, complete bundle
inventory, third-party licence evidence, and distribution boundary before considering Linux hardware probing or
product availability; it must not infer either from this Docker proof or from a structurally valid candidate record.
