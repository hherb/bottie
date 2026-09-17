# Bottie handover

Last verified: 2026-09-17

## Start here

Start from `codex/linux-image-worker-ucc-ablation`. Read `ROADMAP.md` Milestone 8.4,
`docs/local-image-linux-nvidia-proof.md`, `docs/local-image-linux-worker-bundle.md`, and
`local-image-worker/prove_diffusers_worker.py`.

## Current state

- The retained NGC proof and its hash-pinned worker input are unchanged. Its complete traced closure still has 84
  components and 106 licence-only blockers. The unresolved identity is
  `native:hpcx-ucc@1.5.0+ec95a0a96fc7220e1627157439c508cafc82274e`; the exact target archive contains no UCC
  licence/source bytes and its library differs from the retained image by eight bytes.
- Masking `/opt/hpcx/ucc` on the retained image fails because NGC PyTorch directly links `libucc.so.1`. Masking all of
  `/opt/hpcx` additionally removes required `libmpi.so.40`.
- A provisional derived image replaced NGC PyTorch with the official conventional ARM64 PyTorch 2.10.0 CUDA 13.0
  wheel, SHA-256 `4fc8f67637f4c92b989a07d80ffe755e79a3510ca02ebf23ce66396fb277c88d`. With the UCC
  installation masked, the full offline private-protocol proof passed and live process maps contained no UCC library.
  The decoded cold/warm RGB SHA-256 was
  `a3ec2972f88027b969e087a39d4d1b738438b61150ccd4d9dec2842ca1a9ad70`; manual review found the same coherent
  scene as the baseline.
- That result is not accepted evidence: the image still contains masked UCC bytes, the run predates the corrected
  runtime identity, and the wheel reports CUDA architectures only through 12.0 on the compute-capability-12.1 GB10.
  No replacement measurement, closure, licence review, bundle, profile, or product gate is checked in.
- The new proof-only route has its own worker identity and checksum-pinned Docker recipe. Selecting
  `pytorch-2.10-cu130` requires `--ablate-ucc`; the harness verifies the empty read-only UCC mask after handshake and
  rejects any live mapping containing either the HPC-X UCC path or `libucc` elsewhere. The original proof worker's
  exact SHA-256 remains `772ed436de27afc01c043202a7815097e9d2249bd1368b2efa53e1d28d54a67c`.
- Upload of the four non-secret proof/build inputs to the named DGX was denied because this session has no explicit
  confirmation that the host is trusted for repository-source transfer. Do not work around that boundary.
- `licenseReviewed`, `assemblyEligible`, and `distributionReviewed` remain false. Linux local image generation remains
  unavailable. Unrelated logo-kit, screenshot, and Linux public-key files remain untouched.

## Validation

- All 71 local image-worker Python tests pass.
- Prettier, Svelte diagnostics, all 407 active frontend/script tests (3 skipped), and the production build pass.
- Cargo formatting and locked checking pass. The identical locked Rust suite passes outside the sandbox after the
  expected loopback-bind denial: 649 library tests (37 ignored), 1 updater-evidence test, 6 local-image adapter tests,
  10 worker-transport tests, and doc tests.
- `git diff --check` passes.

## Next slice

After the user explicitly approves sending the named non-secret repository proof/build files to their DGX host:

1. build `Dockerfile.diffusers-pytorch-proof`, rerun the full proof with
   `--runtime-profile pytorch-2.10-cu130 --ablate-ucc`, and retain only correctly identified path-free measurements;
2. require deterministic cold/warm pixels, visual review, network denial, cancellation, clean shutdown, an empty UCC
   mount, and zero `libucc` mappings; record the exact image ID, size, wheel metadata, and architecture warning;
3. if that control passes, build a fresh exact ARM64 Ubuntu/Python runtime that never contains HPC-X or UCC, freeze its
   complete dependency inputs, repeat the proof twice, and generate a new traced runtime closure from those exact bytes.

Do not treat a masked image as distributable, update the original NGC closure, infer licence expressions, create a
review manifest, accept a Linux profile, add an executable or hardware probe, wire app execution/UI, publish a bundle,
or perform signing, release, Store, updater, or other provider activity.
