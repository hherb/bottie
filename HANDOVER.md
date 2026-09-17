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
- The corrected replacement control is
  `sha256:033071250053770a99190dfaaf87a05f6ba377f3b5a821c5ba4a8e649cf5c6bf`, 21,250,038,967 bytes. It passed
  the full offline proof under the distinct identity with zero UCC mappings and reproduced the provisional pixels.
- The new proof-only route has its own worker identity and checksum-pinned Docker recipe. Selecting
  `pytorch-2.10-cu130` requires `--ablate-ucc`; the harness verifies the empty read-only UCC mask after handshake and
  rejects any live mapping containing either the HPC-X UCC path or `libucc` elsewhere. The original proof worker's
  exact SHA-256 remains `772ed436de27afc01c043202a7815097e9d2249bd1368b2efa53e1d28d54a67c`.
- A clean Ubuntu-based ARM64 image now passes the same proof. Its exact identity is
  `sha256:5039ad07130ce8d29f12325a54e02bf95e90112e114d745e5883434180e3bdad`; it is 5,780,237,265 bytes and has
  no HPC-X installation or `libucc` runtime. PyTorch still includes UCC-related C++ headers, so do not claim absence of
  all UCC-named source bytes. Cold/warm PNGs are identical to the reviewed replacement output, live mappings contain no
  `libucc`, and network denial, cancellation, and shutdown pass.
- The clean image resolved 63 Python distributions and 112 Debian packages. Exact versions were observed from the
  image, but the build used mutable package indexes and their source-wheel/deb hashes and licences are not frozen. No
  repeated trace, closure record, review manifest, bundle, accepted profile, or executable exists for this image.
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

Freeze the clean image's exact dependency inputs without broadening product scope:

1. produce a complete sorted Python lock for all 63 distributions with exact ARM64 wheel filenames, sizes, and SHA-256
   hashes, including the already verified PyTorch wheel; reject source builds and index drift;
2. bind the 112 exact Debian package versions to immutable `.deb` bytes and hashes or an equivalently immutable Ubuntu
   snapshot, then rebuild twice and require the same installed inventories and image filesystem evidence;
3. repeat the full proof and Python/process-map trace twice, require byte-identical path-free records and zero `libucc`
   mappings, then generate a new closure without importing assumptions from the NGC record.

Do not treat either proved image as distributable, update the original NGC closure, infer licence expressions, create a
review manifest, accept a Linux profile, add an executable or hardware probe, wire app execution/UI, publish a bundle,
or perform signing, release, Store, updater, or other provider activity.
